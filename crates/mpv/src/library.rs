//! Finds libmpv at runtime and resolves the functions mediaplayer uses.

use std::error::Error;
use std::fmt;
use std::path::Path;
#[cfg(any(windows, target_vendor = "apple", test))]
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::ffi::Api;

/// The client API major version this crate is written against (mpv 0.35 and later).
const SUPPORTED_MAJOR: u32 = 2;

/// A loaded libmpv. It stays loaded for the rest of the process.
#[derive(Clone, Copy)]
pub struct Libmpv {
    pub(crate) api: &'static Api,
    version: (u32, u32),
}

impl fmt::Debug for Libmpv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Libmpv")
            .field("client_api_version", &self.version)
            .finish_non_exhaustive()
    }
}

impl Libmpv {
    /// Loads libmpv from this platform's standard location. The result is cached.
    ///
    /// On Windows that means `libmpv-2.dll` (or the older `mpv-2.dll`) next to the
    /// executable, then System32. The current directory and `PATH` are never searched.
    pub fn load() -> Result<Self, LoadError> {
        static LOADED: OnceLock<Result<Libmpv, LoadError>> = OnceLock::new();
        LOADED
            .get_or_init(|| Self::from_library(open_default()?))
            .clone()
    }

    /// Loads libmpv from an explicit file, for tests and diagnostics. The app only
    /// uses [`Self::load`].
    pub fn load_from(path: &Path) -> Result<Self, LoadError> {
        Self::from_library(open_path(path)?)
    }

    /// `(major, minor)` client API version of the loaded library.
    pub fn client_api_version(&self) -> (u32, u32) {
        self.version
    }

    fn from_library(library: libloading::Library) -> Result<Self, LoadError> {
        // Never unloaded: libmpv's own threads can outlive mpv_terminate_destroy.
        let library: &'static libloading::Library = Box::leak(Box::new(library));
        let api: &'static Api = Box::leak(Box::new(resolve(library)?));
        // SAFETY: mpv_client_api_version has no preconditions.
        let raw = widen(unsafe { (api.client_api_version)() });
        let version = ((raw >> 16) as u32, (raw & 0xffff) as u32);
        if version.0 != SUPPORTED_MAJOR {
            return Err(LoadError::Incompatible {
                major: version.0,
                minor: version.1,
            });
        }
        Ok(Self { api, version })
    }
}

/// Why libmpv couldn't be used. `Display` tells the user what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// No libmpv where we look; `location` names those places.
    NotFound { location: String },
    /// A libmpv file exists, but the OS refused to load it.
    Unloadable { library: String, reason: String },
    /// The library lacks a function we need.
    MissingSymbol(&'static str),
    /// The library speaks a client API major version we don't support.
    Incompatible { major: u32, minor: u32 },
}

impl LoadError {
    /// A short description without paths, for logs.
    pub fn summary(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not found",
            Self::Unloadable { .. } => "found but not loadable",
            Self::MissingSymbol(_) => "missing a required function",
            Self::Incompatible { .. } => "unsupported client API version",
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { location } if cfg!(windows) => write!(
                f,
                "libmpv-2.dll was not found next to mediaplayer.exe (in {location}). Download \
                 the mpv-dev archive (mpv-dev-x86_64-….7z) from \
                 https://github.com/shinchiro/mpv-winbuild-cmake/releases, copy libmpv-2.dll \
                 from it into that folder, and restart mediaplayer."
            ),
            Self::NotFound { location } if cfg!(target_vendor = "apple") => write!(
                f,
                "libmpv.2.dylib was not found (looked in {location}). Install it with \
                 `brew install mpv` and restart mediaplayer."
            ),
            Self::NotFound { .. } => f.write_str(
                "libmpv.so.2 was not found. Install libmpv (Debian/Ubuntu: \
                 `sudo apt install libmpv2`) and restart mediaplayer.",
            ),
            Self::Unloadable { library, reason } if cfg!(windows) => write!(
                f,
                "{library} was found but couldn't be loaded ({reason}). Use the 64-bit \
                 x86_64 mpv-dev build, and make sure a graphics driver is installed: libmpv \
                 needs the driver's vulkan-1.dll."
            ),
            Self::Unloadable { library, reason } => {
                write!(f, "{library} couldn't be loaded ({reason}).")
            }
            Self::MissingSymbol(name) => {
                write!(f, "libmpv is missing {name}(); install mpv 0.35 or newer.")
            }
            Self::Incompatible { major, minor } => write!(
                f,
                "libmpv client API {major}.{minor} isn't supported; mediaplayer needs 2.x \
                 (mpv 0.35 or newer)."
            ),
        }
    }
}

impl Error for LoadError {}

fn resolve(library: &'static libloading::Library) -> Result<Api, LoadError> {
    macro_rules! symbol {
        ($name:literal) => {
            // SAFETY: the receiving field's type matches this function's C prototype
            // in client API 2.x (see `ffi`), and the library is never unloaded.
            match unsafe { library.get(concat!($name, "\0").as_bytes()) } {
                Ok(symbol) => *symbol,
                Err(_) => return Err(LoadError::MissingSymbol($name)),
            }
        };
    }
    Ok(Api {
        client_api_version: symbol!("mpv_client_api_version"),
        error_string: symbol!("mpv_error_string"),
        free: symbol!("mpv_free"),
        create: symbol!("mpv_create"),
        initialize: symbol!("mpv_initialize"),
        terminate_destroy: symbol!("mpv_terminate_destroy"),
        set_option_string: symbol!("mpv_set_option_string"),
        command: symbol!("mpv_command"),
        command_ret: symbol!("mpv_command_ret"),
        free_node_contents: symbol!("mpv_free_node_contents"),
        set_property: symbol!("mpv_set_property"),
        get_property: symbol!("mpv_get_property"),
        get_property_string: symbol!("mpv_get_property_string"),
        observe_property: symbol!("mpv_observe_property"),
        request_log_messages: symbol!("mpv_request_log_messages"),
        wait_event: symbol!("mpv_wait_event"),
        set_wakeup_callback: symbol!("mpv_set_wakeup_callback"),
        render_context_create: symbol!("mpv_render_context_create"),
        render_context_set_update_callback: symbol!("mpv_render_context_set_update_callback"),
        render_context_update: symbol!("mpv_render_context_update"),
        render_context_render: symbol!("mpv_render_context_render"),
        render_context_free: symbol!("mpv_render_context_free"),
    })
}

/// File names accepted next to the executable on Windows: current builds ship
/// `libmpv-2.dll`, older ones `mpv-2.dll`. Always absolute, never bare names.
#[cfg(any(windows, test))]
fn windows_candidates(exe_dir: &Path) -> [PathBuf; 2] {
    [exe_dir.join("libmpv-2.dll"), exe_dir.join("mpv-2.dll")]
}

#[cfg(windows)]
fn open_default() -> Result<libloading::Library, LoadError> {
    use libloading::os::windows::{LOAD_LIBRARY_SEARCH_SYSTEM32, Library};

    let exe_dir = exe_dir()?;
    for path in windows_candidates(&exe_dir) {
        if path.is_file() {
            return open_path(&path);
        }
    }
    // SAFETY: loading runs the DLL's initialisers. This flag limits the search, for
    // libmpv and its dependencies, to System32.
    unsafe { Library::load_with_flags("libmpv-2.dll", LOAD_LIBRARY_SEARCH_SYSTEM32) }
        .map(|library| {
            log::info!("loaded libmpv-2.dll from System32");
            library.into()
        })
        .map_err(|_| LoadError::NotFound {
            location: exe_dir.display().to_string(),
        })
}

#[cfg(windows)]
fn open_path(path: &Path) -> Result<libloading::Library, LoadError> {
    use libloading::os::windows::{
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, Library,
    };

    let unloadable = |reason: String| LoadError::Unloadable {
        library: path.display().to_string(),
        reason,
    };
    // DLL_LOAD_DIR needs an absolute path.
    let path = std::path::absolute(path).map_err(|err| unloadable(err.to_string()))?;
    // SAFETY: loading runs the DLL's initialisers. We load this exact file, and its
    // dependencies only from its own folder or System32: never the current
    // directory or PATH (DLL planting).
    unsafe {
        Library::load_with_flags(
            &path,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    }
    .map(|library| {
        log::info!("loaded libmpv from {}", path.display());
        library.into()
    })
    .map_err(|err| unloadable(describe(&err)))
}

#[cfg(all(unix, not(target_vendor = "apple")))]
fn open_default() -> Result<libloading::Library, LoadError> {
    const NAME: &str = "libmpv.so.2";
    // SAFETY: loading runs the library's initialisers; the dynamic linker resolves
    // the versioned soname from the system library paths.
    unsafe { libloading::Library::new(NAME) }.map_err(|err| {
        let reason = describe(&err);
        if reason.contains("No such file") {
            LoadError::NotFound {
                location: "the system library path".to_owned(),
            }
        } else {
            LoadError::Unloadable {
                library: NAME.to_owned(),
                reason,
            }
        }
    })
}

#[cfg(target_vendor = "apple")]
fn open_default() -> Result<libloading::Library, LoadError> {
    let mut candidates: Vec<PathBuf> = exe_dir()
        .ok()
        .map(|dir| dir.join("libmpv.2.dylib"))
        .into_iter()
        .collect();
    // Homebrew's prefixes aren't on the default dlopen path.
    candidates.extend(
        [
            "/opt/homebrew/lib/libmpv.2.dylib",
            "/usr/local/lib/libmpv.2.dylib",
        ]
        .map(PathBuf::from),
    );
    match candidates.iter().find(|path| path.is_file()) {
        Some(path) => open_path(path),
        None => Err(LoadError::NotFound {
            location: "the app folder, /opt/homebrew/lib and /usr/local/lib".to_owned(),
        }),
    }
}

#[cfg(not(any(windows, unix)))]
fn open_default() -> Result<libloading::Library, LoadError> {
    Err(LoadError::NotFound {
        location: "this platform".to_owned(),
    })
}

#[cfg(not(windows))]
fn open_path(path: &Path) -> Result<libloading::Library, LoadError> {
    // SAFETY: loading runs the initialisers of the exact file the caller chose.
    unsafe { libloading::Library::new(path) }.map_err(|err| LoadError::Unloadable {
        library: path.display().to_string(),
        reason: describe(&err),
    })
}

#[cfg(any(windows, target_vendor = "apple"))]
fn exe_dir() -> Result<PathBuf, LoadError> {
    let unloadable = |reason: String| LoadError::Unloadable {
        library: "libmpv".to_owned(),
        reason,
    };
    let exe = std::env::current_exe()
        .map_err(|err| unloadable(format!("can't locate the executable: {err}")))?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| unloadable("the executable has no parent folder".to_owned()))
}

/// `c_ulong` is u32 on Windows and u64 elsewhere.
#[allow(clippy::useless_conversion)]
fn widen(value: std::ffi::c_ulong) -> u64 {
    u64::from(value)
}

/// The error and its causes, e.g. "LoadLibraryExW failed: The specified module could not be found."
fn describe(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_only_looks_next_to_the_executable() {
        let dir = Path::new("/apps/mediaplayer");
        let candidates = windows_candidates(dir);
        assert!(candidates.iter().all(|path| path.parent() == Some(dir)));
        assert_eq!(candidates[0], dir.join("libmpv-2.dll"));
    }

    #[test]
    fn not_found_says_what_to_install() {
        let text = LoadError::NotFound {
            location: "C:\\Apps".to_owned(),
        }
        .to_string();
        if cfg!(windows) {
            assert!(text.contains("libmpv-2.dll was not found next to mediaplayer.exe"));
            assert!(text.contains("mpv-winbuild-cmake/releases"));
            assert!(text.contains("C:\\Apps"));
        } else if cfg!(target_vendor = "apple") {
            assert!(text.contains("brew install mpv"));
        } else {
            assert!(text.contains("sudo apt install libmpv2"));
        }
    }

    #[test]
    fn version_mismatch_names_the_supported_version() {
        let text = LoadError::Incompatible {
            major: 1,
            minor: 109,
        }
        .to_string();
        assert!(text.contains("1.109") && text.contains("2.x"));
    }

    #[test]
    fn summaries_carry_no_paths() {
        let err = LoadError::Unloadable {
            library: "/home/someone/libmpv.so.2".to_owned(),
            reason: "bad ELF".to_owned(),
        };
        assert!(!err.summary().contains("someone"));
    }
}
