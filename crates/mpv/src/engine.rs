//! [`MpvEngine`]: the [`PlayerEngine`] backed by libmpv.

use std::error::Error;
use std::ffi::{CStr, CString};
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use mediaplayer_core::{EngineError, EngineEvent, EngineInfo, LoadId, PlayerEngine};

use crate::events::{self, EventMapper, RawEvent, property};
use crate::ffi;
use crate::handle::{Handle, Status};
use crate::video::{GetProcAddress, VideoOutput};
use crate::{Libmpv, Waker};

/// Caps the work done per frame if libmpv floods us with events.
const MAX_EVENTS_PER_POLL: usize = 1000;

/// An option applied before `mpv_initialize`.
pub(crate) struct MpvOption {
    pub name: &'static CStr,
    pub value: &'static CStr,
    /// Startup fails if libmpv rejects a required option; others may not exist in
    /// every mpv version.
    pub required: bool,
}

const fn option(name: &'static CStr, value: &'static CStr, required: bool) -> MpvOption {
    MpvOption {
        name,
        value,
        required,
    }
}

/// Hardened, embedding-friendly settings. See docs/adr/0003-libmpv-runtime-loading.md.
pub(crate) const OPTIONS: &[MpvOption] = &[
    // Never read the user's mpv.conf/input.conf or scripts, and never run yt-dlp.
    option(c"config", c"no", true),
    option(c"load-scripts", c"no", true),
    option(c"ytdl", c"no", true),
    // Don't follow references inside files (playlists, EDL, archives, ordered
    // chapters): only the local file the user picked is opened.
    option(c"access-references", c"no", true),
    option(c"idle", c"yes", true),
    option(c"vo", c"libmpv", true),
    option(c"terminal", c"no", false),
    option(c"input-default-bindings", c"no", false),
    option(c"input-vo-keyboard", c"no", false),
    option(c"osc", c"no", false),
    option(c"osd-level", c"0", false),
    // Built-in scripts; the set differs between mpv versions.
    option(c"load-stats-overlay", c"no", false),
    option(c"load-osd-console", c"no", false),
    option(c"load-console", c"no", false),
    option(c"load-auto-profiles", c"no", false),
    option(c"load-select", c"no", false),
    option(c"load-positioning", c"no", false),
    option(c"load-commands", c"no", false),
    option(c"load-context-menu", c"no", false),
    // The player relies on END_FILE at the end of a file.
    option(c"keep-open", c"no", false),
    option(c"hwdec", c"auto-safe", false),
    // VideoOutput renders without blocking for the display time; mpv recommends
    // pairing that with a zero timing offset.
    option(c"video-timing-offset", c"0", false),
    // libmpv doesn't initialise the padding in its scaler weight LUTs. With 6- or
    // 10-tap kernels (its default lanczos, or hermite at some downscale ratios), a
    // NaN there can turn the whole picture black. Radius-2 kernels use 4 taps (no
    // padding), and correct-downscaling=no stops dscale from widening them.
    // See docs/adr/0004-video-output.md.
    option(c"scale", c"catmull_rom", false),
    option(c"cscale", c"catmull_rom", false),
    option(c"dscale", c"catmull_rom", false),
    option(c"correct-downscaling", c"no", false),
    // Without an audio device (VMs, CI), play silently instead of failing.
    option(c"audio-fallback-to-null", c"yes", false),
];

/// libmpv refused to start or to create a renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpvError(String);

impl fmt::Display for MpvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MpvError {}

/// Plays local files through libmpv.
pub struct MpvEngine {
    handle: Arc<Handle>,
    mapper: EventMapper,
    name: String,
    shut_down: bool,
}

impl MpvEngine {
    /// Starts a libmpv core. `wakeup` runs on libmpv's threads whenever events are
    /// waiting for [`PlayerEngine::poll_events`].
    pub fn new(libmpv: Libmpv, wakeup: Waker) -> Result<Self, MpvError> {
        let mut handle = Handle::create(libmpv.api).map_err(MpvError)?;
        for option in OPTIONS {
            if let Err(status) = handle.set_option(option.name, option.value) {
                let message = format!(
                    "option {}={} was rejected: {}",
                    option.name.to_string_lossy(),
                    option.value.to_string_lossy(),
                    handle.error_string(status)
                );
                if option.required {
                    return Err(MpvError(message));
                }
                log::debug!("libmpv: {message}");
            }
        }
        if let Err(status) = handle.request_log_messages(c"warn") {
            log::debug!(
                "libmpv log forwarding unavailable: {}",
                handle.error_string(status)
            );
        }
        handle.initialize().map_err(|status| {
            MpvError(format!(
                "mpv_initialize failed: {}",
                handle.error_string(status)
            ))
        })?;
        for (id, name, format) in [
            (property::TIME_POS, c"time-pos", ffi::FORMAT_DOUBLE),
            (property::DURATION, c"duration", ffi::FORMAT_DOUBLE),
            (property::WIDTH, c"dwidth", ffi::FORMAT_INT64),
            (property::HEIGHT, c"dheight", ffi::FORMAT_INT64),
        ] {
            handle.observe(id, name, format).map_err(|status| {
                MpvError(format!(
                    "can't observe {}: {}",
                    name.to_string_lossy(),
                    handle.error_string(status)
                ))
            })?;
        }
        handle.set_wakeup(wakeup);

        let version = handle
            .get_string(c"mpv-version")
            .unwrap_or_else(|| "mpv".to_owned());
        let (major, minor) = libmpv.client_api_version();
        log::info!("libmpv ready: {version}, client API {major}.{minor}");
        Ok(Self {
            handle: Arc::new(handle),
            mapper: EventMapper::default(),
            name: format!("libmpv ({version})"),
            shut_down: false,
        })
    }

    /// Creates the video output that draws into the UI's OpenGL context.
    ///
    /// Call it with that context current (eframe's app-creation closure), before
    /// any file loads; without it video tracks are skipped and only audio plays.
    pub fn create_video_output(
        &self,
        get_proc_address: Arc<GetProcAddress>,
        frame_ready: Waker,
    ) -> Result<VideoOutput, MpvError> {
        VideoOutput::new(Arc::clone(&self.handle), get_proc_address, frame_ready).map_err(MpvError)
    }

    fn handle_event(&mut self, event: RawEvent, out: &mut Vec<EngineEvent>) {
        match event {
            RawEvent::StartFile { entry } => self.mapper.start_file(entry),
            RawEvent::FileLoaded => {
                let duration = self.handle.get_double(c"duration");
                out.extend(self.mapper.file_loaded(duration));
            }
            RawEvent::EndFile {
                entry,
                reason,
                error,
            } => {
                let handle = &self.handle;
                let message = || events::describe_error(error, || handle.error_string(error));
                out.extend(self.mapper.end_file(entry, reason, message));
            }
            RawEvent::Property { id, value } => out.extend(self.mapper.property(id, value)),
            RawEvent::Log {
                level,
                prefix,
                text,
            } => {
                let level = if matches!(level.as_str(), "fatal" | "error") {
                    log::Level::Error
                } else {
                    log::Level::Warn
                };
                log::log!(level, "libmpv [{prefix}] {}", text.trim_end());
            }
            RawEvent::Shutdown => {
                self.shut_down = true;
                log::error!("libmpv shut down unexpectedly");
            }
            RawEvent::Other => {}
        }
    }

    fn log_failure(&self, what: &str, result: Result<(), Status>) {
        if let Err(status) = result {
            log::warn!(
                "libmpv: {what} failed: {}",
                self.handle.error_string(status)
            );
        }
    }
}

impl PlayerEngine for MpvEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            name: self.name.clone(),
            simulated: false,
        }
    }

    fn load(&mut self, path: &Path) -> Result<LoadId, EngineError> {
        let target = path_arg(path)?;
        let entry = self
            .handle
            .command_returning(&[c"loadfile", &target, c"replace"], "playlist_entry_id")
            .map_err(|status| EngineError(self.handle.error_string(status)))?
            .ok_or_else(|| EngineError("libmpv didn't report a playlist entry".to_owned()))?;
        Ok(LoadId(entry.unsigned_abs()))
    }

    fn play(&mut self) {
        self.log_failure("play", self.handle.set_flag(c"pause", false));
    }

    fn pause(&mut self) {
        self.log_failure("pause", self.handle.set_flag(c"pause", true));
    }

    fn stop(&mut self) {
        self.log_failure("stop", self.handle.command(&[c"stop"]));
    }

    fn seek(&mut self, position: Duration) {
        let target = seconds_arg(position);
        self.log_failure(
            "seek",
            self.handle.command(&[c"seek", &target, c"absolute"]),
        );
    }

    fn set_volume(&mut self, volume: f32) {
        // libmpv's volume is 0–100 (with its own perceptual curve).
        let percent = f64::from(volume.clamp(0.0, 1.0)) * 100.0;
        self.log_failure("volume", self.handle.set_double(c"volume", percent));
    }

    fn set_muted(&mut self, muted: bool) {
        self.log_failure("mute", self.handle.set_flag(c"mute", muted));
    }

    fn poll_events(&mut self) -> Vec<EngineEvent> {
        let mut out = Vec::new();
        for _ in 0..MAX_EVENTS_PER_POLL {
            // After SHUTDOWN, libmpv returns SHUTDOWN forever.
            if self.shut_down {
                break;
            }
            match self.handle.next_event() {
                Some(event) => self.handle_event(event, &mut out),
                None => break,
            }
        }
        out
    }
}

/// The path as libmpv expects it: raw bytes on Unix, UTF-8 elsewhere.
fn path_arg(path: &Path) -> Result<CString, EngineError> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(not(unix))]
    let bytes = path
        .to_str()
        .ok_or_else(|| EngineError("the file name isn't valid Unicode".to_owned()))?
        .as_bytes()
        .to_vec();
    CString::new(bytes).map_err(|_| EngineError("the file name contains a NUL byte".to_owned()))
}

/// `seek` takes seconds; milliseconds are as precise as the UI gets.
fn seconds_arg(position: Duration) -> CString {
    CString::new(format!("{:.3}", position.as_secs_f64())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(name: &str) -> &'static MpvOption {
        OPTIONS
            .iter()
            .find(|option| option.name.to_bytes() == name.as_bytes())
            .unwrap_or_else(|| panic!("option {name} is not set"))
    }

    #[test]
    fn hardening_options_are_required() {
        for (name, value) in [
            ("config", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("access-references", "no"),
            ("vo", "libmpv"),
        ] {
            let option = find(name);
            assert_eq!(option.value.to_bytes(), value.as_bytes(), "{name}");
            assert!(option.required, "{name} must be required");
        }
    }

    const SCALER_OPTIONS: [(&str, &str); 4] = [
        ("scale", "catmull_rom"),
        ("cscale", "catmull_rom"),
        ("dscale", "catmull_rom"),
        ("correct-downscaling", "no"),
    ];

    #[test]
    fn scalers_use_kernels_without_lut_padding() {
        for (name, value) in SCALER_OPTIONS {
            assert_eq!(find(name).value.to_bytes(), value.as_bytes(), "{name}");
        }
    }

    /// The scaler options are optional, so a typo would only show up as black
    /// video on some drivers; the real library validates them here.
    #[test]
    #[ignore = "needs libmpv"]
    fn libmpv_accepts_the_scaler_options() {
        let libmpv = match std::env::var_os("MPV_LIB") {
            Some(path) => Libmpv::load_from(Path::new(&path)),
            None => Libmpv::load(),
        }
        .unwrap_or_else(|err| panic!("{err}"));
        let handle = Handle::create(libmpv.api).unwrap_or_else(|err| panic!("{err}"));
        for (name, _) in SCALER_OPTIONS {
            let option = find(name);
            if let Err(status) = handle.set_option(option.name, option.value) {
                panic!("libmpv rejected {name}: {}", handle.error_string(status));
            }
        }
    }

    #[test]
    fn no_ipc_or_script_options_are_ever_set() {
        for forbidden in ["input-ipc-server", "script", "scripts", "ytdl-path"] {
            assert!(
                OPTIONS
                    .iter()
                    .all(|option| option.name.to_bytes() != forbidden.as_bytes()),
                "{forbidden}"
            );
        }
    }

    #[test]
    fn seek_targets_are_seconds_with_millisecond_precision() {
        assert_eq!(
            seconds_arg(Duration::from_millis(83_456)).to_bytes(),
            b"83.456"
        );
    }

    #[test]
    fn paths_pass_through_unchanged() {
        let path = Path::new("/music/Ünïcode – name.mp3");
        assert_eq!(
            path_arg(path).unwrap().as_bytes(),
            "/music/Ünïcode – name.mp3".as_bytes()
        );
    }

    #[cfg(unix)]
    #[test]
    fn paths_with_nul_bytes_are_refused() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(OsStr::from_bytes(b"/tmp/a\0b.mp3"));
        assert!(path_arg(path).is_err());
    }
}
