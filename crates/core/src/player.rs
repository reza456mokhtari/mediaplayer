//! The playback state machine every frontend drives.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::engine::{EngineEvent, EngineInfo, LoadId, PlayerEngine, VideoSize};
use crate::media::{MediaItem, OpenError, validate_local_file};

/// Volume a new player starts with.
pub const DEFAULT_VOLUME: f32 = 0.8;

/// Where the player is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Nothing loaded.
    Idle,
    /// A file was handed to the engine and is opening.
    Loading,
    Playing,
    Paused,
    /// Stopped by the user. The file stays selected; Play starts it over.
    Stopped,
    /// Reached the end of the file. Play starts it over.
    Ended,
}

/// Everything a frontend can ask the player to do.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    Open(PathBuf),
    Play,
    Pause,
    TogglePlayPause,
    Stop,
    SeekTo(Duration),
    SeekForward(Duration),
    SeekBackward(Duration),
    /// Linear volume, clamped to `0.0..=1.0`; non-finite values are ignored.
    SetVolume(f32),
    /// Adds a signed delta to the volume.
    ChangeVolume(f32),
    ToggleMute,
    /// Clears the error shown to the user.
    DismissError,
}

/// A user-facing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerError {
    /// The file was rejected before it reached the engine.
    Open(OpenError),
    /// The engine could not load or play the file.
    Backend { path: PathBuf, message: String },
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(err) => write!(f, "{err}"),
            Self::Backend { path, message } => {
                write!(f, "Couldn't play {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for PlayerError {}

/// Playback state machine.
///
/// [`Self::dispatch`] applies commands right away (optimistically) and
/// [`Self::tick`] folds in what the engine reports. Commands that make no sense
/// in the current state are ignored, and failures surface through
/// [`Self::error`] instead of panics, so a frontend can forward any input as-is.
pub struct Player {
    engine: Box<dyn PlayerEngine>,
    state: PlaybackState,
    media: Option<MediaItem>,
    load_id: Option<LoadId>,
    resume_after_load: bool,
    position: Duration,
    duration: Option<Duration>,
    video: Option<VideoSize>,
    volume: f32,
    muted: bool,
    error: Option<PlayerError>,
}

impl Player {
    pub fn new(mut engine: Box<dyn PlayerEngine>) -> Self {
        engine.set_volume(DEFAULT_VOLUME);
        engine.set_muted(false);
        Self {
            engine,
            state: PlaybackState::Idle,
            media: None,
            load_id: None,
            resume_after_load: true,
            position: Duration::ZERO,
            duration: None,
            video: None,
            volume: DEFAULT_VOLUME,
            muted: false,
            error: None,
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn media(&self) -> Option<&MediaItem> {
        self.media.as_ref()
    }

    pub fn position(&self) -> Duration {
        self.position
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    /// Size of the current picture; `None` for audio-only media or when nothing plays.
    pub fn video(&self) -> Option<VideoSize> {
        self.video
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn error(&self) -> Option<&PlayerError> {
        self.error.as_ref()
    }

    pub fn engine_info(&self) -> EngineInfo {
        self.engine.info()
    }

    pub fn is_seekable(&self) -> bool {
        matches!(self.state, PlaybackState::Playing | PlaybackState::Paused)
            && self.duration.is_some()
    }

    pub fn dispatch(&mut self, command: PlayerCommand) {
        match command {
            PlayerCommand::Open(path) => self.open(&path),
            PlayerCommand::Play => self.play(),
            PlayerCommand::Pause => self.pause(),
            PlayerCommand::TogglePlayPause => {
                let running = self.state == PlaybackState::Playing
                    || (self.state == PlaybackState::Loading && self.resume_after_load);
                if running {
                    self.pause();
                } else {
                    self.play();
                }
            }
            PlayerCommand::Stop => self.stop(),
            PlayerCommand::SeekTo(position) => self.seek(position),
            PlayerCommand::SeekForward(by) => self.seek(self.position.saturating_add(by)),
            PlayerCommand::SeekBackward(by) => self.seek(self.position.saturating_sub(by)),
            PlayerCommand::SetVolume(volume) => self.set_volume(volume),
            PlayerCommand::ChangeVolume(delta) => self.set_volume(self.volume + delta),
            PlayerCommand::ToggleMute => {
                self.muted = !self.muted;
                self.engine.set_muted(self.muted);
            }
            PlayerCommand::DismissError => self.error = None,
        }
    }

    /// Applies the engine's pending events. Call once per frame.
    pub fn tick(&mut self) {
        for event in self.engine.poll_events() {
            self.apply(event);
        }
    }

    fn open(&mut self, path: &Path) {
        match validate_local_file(path) {
            Ok(path) => self.load(MediaItem::new(path)),
            // Rejected before the engine is involved, so current playback continues.
            Err(err) => self.error = Some(PlayerError::Open(err)),
        }
    }

    fn load(&mut self, item: MediaItem) {
        match self.engine.load(item.path()) {
            Ok(id) => {
                self.engine.play();
                self.state = PlaybackState::Loading;
                self.load_id = Some(id);
                self.resume_after_load = true;
                self.media = Some(item);
                self.position = Duration::ZERO;
                self.duration = None;
                self.video = None;
                self.error = None;
            }
            Err(err) => {
                self.error = Some(PlayerError::Backend {
                    path: item.path().to_path_buf(),
                    message: err.0,
                });
                self.unload();
            }
        }
    }

    fn unload(&mut self) {
        self.state = PlaybackState::Idle;
        self.media = None;
        self.load_id = None;
        self.position = Duration::ZERO;
        self.duration = None;
        self.video = None;
    }

    fn play(&mut self) {
        match self.state {
            PlaybackState::Paused => {
                self.engine.play();
                self.state = PlaybackState::Playing;
            }
            PlaybackState::Loading => {
                self.engine.play();
                self.resume_after_load = true;
            }
            PlaybackState::Stopped | PlaybackState::Ended => {
                // Reopen through validation: the file may have moved since.
                if let Some(path) = self.media.as_ref().map(|item| item.path().to_path_buf()) {
                    self.open(&path);
                }
            }
            PlaybackState::Idle | PlaybackState::Playing => {}
        }
    }

    fn pause(&mut self) {
        match self.state {
            PlaybackState::Playing => {
                self.engine.pause();
                self.state = PlaybackState::Paused;
            }
            PlaybackState::Loading => {
                self.engine.pause();
                self.resume_after_load = false;
            }
            _ => {}
        }
    }

    fn stop(&mut self) {
        if matches!(
            self.state,
            PlaybackState::Loading
                | PlaybackState::Playing
                | PlaybackState::Paused
                | PlaybackState::Ended
        ) {
            self.engine.stop();
            self.state = PlaybackState::Stopped;
            self.load_id = None;
            self.position = Duration::ZERO;
            self.video = None;
        }
    }

    fn seek(&mut self, position: Duration) {
        if self.is_seekable() {
            let position = self.clamp_to_duration(position);
            self.engine.seek(position);
            self.position = position;
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if !volume.is_finite() {
            return;
        }
        self.volume = volume.clamp(0.0, 1.0);
        self.engine.set_volume(self.volume);
        if self.muted && self.volume > 0.0 {
            self.muted = false;
            self.engine.set_muted(false);
        }
    }

    fn clamp_to_duration(&self, position: Duration) -> Duration {
        self.duration
            .map_or(position, |duration| position.min(duration))
    }

    fn apply(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Loaded { id, duration } => {
                if self.state == PlaybackState::Loading && self.load_id == Some(id) {
                    self.duration = duration;
                    self.state = if self.resume_after_load {
                        PlaybackState::Playing
                    } else {
                        PlaybackState::Paused
                    };
                }
            }
            EngineEvent::Position(position) => {
                if matches!(self.state, PlaybackState::Playing | PlaybackState::Paused) {
                    self.position = self.clamp_to_duration(position);
                }
            }
            // Before `Loaded` these may still describe the previous file.
            EngineEvent::DurationChanged(duration) => {
                if matches!(self.state, PlaybackState::Playing | PlaybackState::Paused) {
                    self.duration = duration;
                    self.position = self.clamp_to_duration(self.position);
                }
            }
            // Engines may report the picture before `Loaded`, so keep it while loading;
            // stale sizes are cleared by the engine's own `None` for the old file.
            EngineEvent::VideoChanged(video) => {
                if matches!(
                    self.state,
                    PlaybackState::Loading | PlaybackState::Playing | PlaybackState::Paused
                ) {
                    self.video = video;
                }
            }
            EngineEvent::Ended { id } => {
                if self.load_id == Some(id)
                    && matches!(self.state, PlaybackState::Playing | PlaybackState::Paused)
                {
                    self.state = PlaybackState::Ended;
                    self.video = None;
                    if let Some(duration) = self.duration {
                        self.position = duration;
                    }
                }
            }
            EngineEvent::Failed { id, message } => {
                if self.load_id == Some(id) {
                    let path = self
                        .media
                        .as_ref()
                        .map(|item| item.path().to_path_buf())
                        .unwrap_or_default();
                    self.error = Some(PlayerError::Backend { path, message });
                    self.unload();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::engine::EngineError;
    use EngineEvent::{DurationChanged, Ended, Failed, Loaded, Position, VideoChanged};
    use PlayerCommand::*;

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Load(PathBuf),
        Play,
        Pause,
        Stop,
        Seek(Duration),
        Volume(f32),
        Muted(bool),
    }

    /// Shared between a test and the boxed engine owned by the player.
    #[derive(Default)]
    struct Script {
        calls: Vec<Call>,
        events: Vec<EngineEvent>,
        loads: u64,
        reject_next_load: Option<String>,
    }

    struct FakeEngine(Rc<RefCell<Script>>);

    impl FakeEngine {
        fn record(&self, call: Call) {
            self.0.borrow_mut().calls.push(call);
        }
    }

    impl PlayerEngine for FakeEngine {
        fn info(&self) -> EngineInfo {
            EngineInfo {
                name: "fake".to_owned(),
                simulated: true,
            }
        }

        fn load(&mut self, path: &Path) -> Result<LoadId, EngineError> {
            self.record(Call::Load(path.to_path_buf()));
            let mut script = self.0.borrow_mut();
            if let Some(message) = script.reject_next_load.take() {
                return Err(EngineError(message));
            }
            script.loads += 1;
            Ok(LoadId(script.loads))
        }

        fn play(&mut self) {
            self.record(Call::Play);
        }

        fn pause(&mut self) {
            self.record(Call::Pause);
        }

        fn stop(&mut self) {
            self.record(Call::Stop);
        }

        fn seek(&mut self, position: Duration) {
            self.record(Call::Seek(position));
        }

        fn set_volume(&mut self, volume: f32) {
            self.record(Call::Volume(volume));
        }

        fn set_muted(&mut self, muted: bool) {
            self.record(Call::Muted(muted));
        }

        fn poll_events(&mut self) -> Vec<EngineEvent> {
            std::mem::take(&mut self.0.borrow_mut().events)
        }
    }

    struct Fixture {
        player: Player,
        script: Rc<RefCell<Script>>,
    }

    impl Fixture {
        fn new() -> Self {
            let script = Rc::new(RefCell::new(Script::default()));
            let player = Player::new(Box::new(FakeEngine(Rc::clone(&script))));
            script.borrow_mut().calls.clear();
            Self { player, script }
        }

        /// A player that opened [`file`] and finished loading a 60 s track.
        fn playing() -> Self {
            let mut fixture = Self::new();
            fixture.send(Open(file()));
            fixture.emit(Loaded {
                id: LoadId(1),
                duration: Some(secs(60)),
            });
            fixture.calls();
            fixture
        }

        fn send(&mut self, command: PlayerCommand) {
            self.player.dispatch(command);
        }

        fn emit(&mut self, event: EngineEvent) {
            self.script.borrow_mut().events.push(event);
            self.player.tick();
        }

        /// Engine calls since the previous check.
        fn calls(&self) -> Vec<Call> {
            std::mem::take(&mut self.script.borrow_mut().calls)
        }

        fn reject_next_load(&self, message: &str) {
            self.script.borrow_mut().reject_next_load = Some(message.to_owned());
        }

        fn state(&self) -> PlaybackState {
            self.player.state()
        }
    }

    fn file() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
    }

    fn other_file() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("lib.rs")
    }

    fn missing_file() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("missing.mp4")
    }

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn starts_idle_and_syncs_volume_to_the_engine() {
        let script = Rc::new(RefCell::new(Script::default()));
        let player = Player::new(Box::new(FakeEngine(Rc::clone(&script))));
        assert_eq!(player.state(), PlaybackState::Idle);
        assert!(player.media().is_none());
        assert_eq!(player.volume(), DEFAULT_VOLUME);
        assert!(!player.is_muted());
        assert_eq!(
            script.borrow().calls,
            [Call::Volume(DEFAULT_VOLUME), Call::Muted(false)]
        );
    }

    #[test]
    fn open_loads_the_file_and_requests_playback() {
        let mut f = Fixture::new();
        f.send(Open(file()));
        assert_eq!(f.state(), PlaybackState::Loading);
        assert_eq!(f.calls(), [Call::Load(file()), Call::Play]);
        assert_eq!(f.player.media().map(MediaItem::title), Some("Cargo"));
    }

    #[test]
    fn loaded_event_starts_playing_with_a_duration() {
        let mut f = Fixture::new();
        f.send(Open(file()));
        f.emit(Loaded {
            id: LoadId(1),
            duration: Some(secs(60)),
        });
        assert_eq!(f.state(), PlaybackState::Playing);
        assert_eq!(f.player.duration(), Some(secs(60)));
        assert!(f.player.is_seekable());
    }

    #[test]
    fn missing_file_reports_an_error_without_touching_the_engine() {
        let mut f = Fixture::new();
        f.send(Open(missing_file()));
        assert_eq!(f.state(), PlaybackState::Idle);
        assert_eq!(
            f.player.error(),
            Some(&PlayerError::Open(OpenError::NotFound(missing_file())))
        );
        assert!(f.calls().is_empty());
    }

    #[test]
    fn failed_open_keeps_the_current_playback() {
        let mut f = Fixture::playing();
        f.send(Open(missing_file()));
        assert_eq!(f.state(), PlaybackState::Playing);
        assert_eq!(
            f.player.media().map(MediaItem::path),
            Some(file().as_path())
        );
        assert!(f.player.error().is_some());
        f.send(DismissError);
        assert!(f.player.error().is_none());
    }

    #[test]
    fn directories_are_rejected() {
        let mut f = Fixture::new();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        f.send(Open(dir.clone()));
        assert_eq!(
            f.player.error(),
            Some(&PlayerError::Open(OpenError::NotAFile(dir)))
        );
    }

    #[test]
    fn transport_commands_are_ignored_while_idle() {
        let mut f = Fixture::new();
        for command in [
            Play,
            Pause,
            TogglePlayPause,
            Stop,
            SeekTo(secs(5)),
            SeekForward(secs(5)),
        ] {
            f.send(command);
        }
        assert_eq!(f.state(), PlaybackState::Idle);
        assert!(f.calls().is_empty());
    }

    #[test]
    fn toggle_pauses_and_resumes() {
        let mut f = Fixture::playing();
        f.send(TogglePlayPause);
        assert_eq!(f.state(), PlaybackState::Paused);
        f.send(TogglePlayPause);
        assert_eq!(f.state(), PlaybackState::Playing);
        assert_eq!(f.calls(), [Call::Pause, Call::Play]);
    }

    #[test]
    fn pausing_while_loading_opens_paused() {
        let mut f = Fixture::new();
        f.send(Open(file()));
        f.send(TogglePlayPause);
        assert_eq!(f.state(), PlaybackState::Loading);
        f.emit(Loaded {
            id: LoadId(1),
            duration: Some(secs(60)),
        });
        assert_eq!(f.state(), PlaybackState::Paused);
        assert_eq!(f.calls(), [Call::Load(file()), Call::Play, Call::Pause]);
    }

    #[test]
    fn stop_rewinds_and_play_reloads_the_same_file() {
        let mut f = Fixture::playing();
        f.emit(Position(secs(20)));
        f.send(Stop);
        assert_eq!(f.state(), PlaybackState::Stopped);
        assert_eq!(f.player.position(), Duration::ZERO);
        f.send(Play);
        assert_eq!(f.state(), PlaybackState::Loading);
        assert_eq!(f.calls(), [Call::Stop, Call::Load(file()), Call::Play]);
    }

    #[test]
    fn seeking_clamps_to_the_track() {
        let mut f = Fixture::playing();
        f.send(SeekTo(secs(90)));
        assert_eq!(f.player.position(), secs(60));
        f.send(SeekBackward(secs(100)));
        assert_eq!(f.player.position(), Duration::ZERO);
        f.send(SeekForward(secs(5)));
        assert_eq!(f.player.position(), secs(5));
        assert_eq!(
            f.calls(),
            [
                Call::Seek(secs(60)),
                Call::Seek(Duration::ZERO),
                Call::Seek(secs(5))
            ]
        );
    }

    #[test]
    fn position_events_only_apply_while_active() {
        let mut f = Fixture::playing();
        f.emit(Position(secs(12)));
        assert_eq!(f.player.position(), secs(12));
        f.send(Stop);
        f.emit(Position(secs(13)));
        assert_eq!(f.player.position(), Duration::ZERO);
    }

    #[test]
    fn end_of_media_then_play_starts_over() {
        let mut f = Fixture::playing();
        f.emit(Ended { id: LoadId(1) });
        assert_eq!(f.state(), PlaybackState::Ended);
        assert_eq!(f.player.position(), secs(60));
        f.send(TogglePlayPause);
        assert_eq!(f.state(), PlaybackState::Loading);
        assert_eq!(f.calls(), [Call::Load(file()), Call::Play]);
    }

    #[test]
    fn late_events_about_a_previous_file_are_ignored() {
        let mut f = Fixture::playing();
        f.send(Open(other_file()));
        f.emit(Ended { id: LoadId(1) });
        f.emit(Failed {
            id: LoadId(1),
            message: "stale".to_owned(),
        });
        f.emit(Loaded {
            id: LoadId(1),
            duration: Some(secs(1)),
        });
        assert_eq!(f.state(), PlaybackState::Loading);
        assert!(f.player.error().is_none());
        f.emit(Loaded {
            id: LoadId(2),
            duration: Some(secs(30)),
        });
        assert_eq!(f.state(), PlaybackState::Playing);
        assert_eq!(f.player.duration(), Some(secs(30)));
    }

    #[test]
    fn backend_failure_reports_an_error_and_unloads() {
        let mut f = Fixture::new();
        f.send(Open(file()));
        f.emit(Failed {
            id: LoadId(1),
            message: "unsupported codec".to_owned(),
        });
        assert_eq!(f.state(), PlaybackState::Idle);
        assert!(f.player.media().is_none());
        assert_eq!(
            f.player.error(),
            Some(&PlayerError::Backend {
                path: file(),
                message: "unsupported codec".to_owned()
            })
        );
    }

    #[test]
    fn a_refused_load_reports_an_error() {
        let mut f = Fixture::new();
        f.reject_next_load("engine shut down");
        f.send(Open(file()));
        assert_eq!(f.state(), PlaybackState::Idle);
        assert_eq!(
            f.player.error(),
            Some(&PlayerError::Backend {
                path: file(),
                message: "engine shut down".to_owned()
            })
        );
    }

    #[test]
    fn volume_is_clamped_and_non_finite_values_are_ignored() {
        let mut f = Fixture::new();
        f.send(SetVolume(1.7));
        assert_eq!(f.player.volume(), 1.0);
        f.send(ChangeVolume(-0.25));
        assert_eq!(f.player.volume(), 0.75);
        f.send(SetVolume(f32::NAN));
        assert_eq!(f.player.volume(), 0.75);
        f.send(SetVolume(-3.0));
        assert_eq!(f.player.volume(), 0.0);
        assert_eq!(
            f.calls(),
            [Call::Volume(1.0), Call::Volume(0.75), Call::Volume(0.0)]
        );
    }

    #[test]
    fn mute_toggles_and_raising_the_volume_unmutes() {
        let mut f = Fixture::new();
        f.send(ToggleMute);
        assert!(f.player.is_muted());
        f.send(SetVolume(0.5));
        assert!(!f.player.is_muted());
        assert_eq!(
            f.calls(),
            [Call::Muted(true), Call::Volume(0.5), Call::Muted(false)]
        );
    }

    const HD: VideoSize = VideoSize {
        width: 1920,
        height: 1080,
    };

    #[test]
    fn video_size_follows_the_engine_while_a_file_is_active() {
        let mut f = Fixture::playing();
        f.emit(VideoChanged(Some(HD)));
        assert_eq!(f.player.video(), Some(HD));
        f.send(Pause);
        f.emit(VideoChanged(None));
        assert_eq!(f.player.video(), None);
    }

    #[test]
    fn video_reported_while_loading_is_kept() {
        let mut f = Fixture::new();
        f.send(Open(file()));
        f.emit(VideoChanged(Some(HD)));
        f.emit(Loaded {
            id: LoadId(1),
            duration: Some(secs(60)),
        });
        assert_eq!(f.player.video(), Some(HD));
    }

    #[test]
    fn video_is_cleared_by_stop_end_and_a_new_load() {
        let mut stopped = Fixture::playing();
        stopped.emit(VideoChanged(Some(HD)));
        stopped.send(Stop);
        assert_eq!(stopped.player.video(), None);

        let mut ended = Fixture::playing();
        ended.emit(VideoChanged(Some(HD)));
        ended.emit(Ended { id: LoadId(1) });
        assert_eq!(ended.player.video(), None);

        let mut reloaded = Fixture::playing();
        reloaded.emit(VideoChanged(Some(HD)));
        reloaded.send(Open(other_file()));
        assert_eq!(reloaded.player.video(), None);
    }

    #[test]
    fn video_events_without_an_active_file_are_ignored() {
        let mut idle = Fixture::new();
        idle.emit(VideoChanged(Some(HD)));
        assert_eq!(idle.player.video(), None);

        let mut stopped = Fixture::playing();
        stopped.send(Stop);
        stopped.emit(VideoChanged(Some(HD)));
        assert_eq!(stopped.player.video(), None);
    }

    #[test]
    fn duration_updates_after_loading_are_applied() {
        let mut f = Fixture::new();
        f.send(Open(file()));
        f.emit(Loaded {
            id: LoadId(1),
            duration: None,
        });
        assert!(!f.player.is_seekable());
        f.emit(DurationChanged(Some(secs(90))));
        assert_eq!(f.player.duration(), Some(secs(90)));
        assert!(f.player.is_seekable());
    }

    #[test]
    fn duration_reported_while_loading_is_ignored() {
        let mut f = Fixture::playing();
        f.send(Open(other_file()));
        // Still describes the previous file.
        f.emit(DurationChanged(Some(secs(5))));
        assert_eq!(f.player.duration(), None);
    }

    #[test]
    fn a_shorter_duration_clamps_the_position() {
        let mut f = Fixture::playing();
        f.emit(Position(secs(40)));
        f.emit(DurationChanged(Some(secs(30))));
        assert_eq!(f.player.position(), secs(30));
    }
}
