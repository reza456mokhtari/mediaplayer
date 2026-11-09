//! UI-agnostic core of mediaplayer.
//!
//! - [`Player`]: the playback state machine every frontend drives with [`PlayerCommand`]s.
//! - [`PlayerEngine`]: the backend boundary; `mediaplayer-mpv` implements it with libmpv.
//! - [`NullEngine`]: a simulated backend for development, tests, and when libmpv is missing.

pub mod engine;
pub mod media;
pub mod null_engine;
pub mod player;
pub mod time;

pub use engine::{EngineError, EngineEvent, EngineInfo, LoadId, PlayerEngine, VideoSize};
pub use media::{MediaItem, OpenError, validate_local_file};
pub use null_engine::{Clock, DEFAULT_SIMULATED_DURATION, ManualClock, NullEngine, SystemClock};
pub use player::{DEFAULT_VOLUME, PlaybackState, Player, PlayerCommand, PlayerError};
pub use time::format_timestamp;
