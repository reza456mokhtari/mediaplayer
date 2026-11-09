//! The boundary between the player state machine and a playback backend.

use std::fmt;
use std::path::Path;
use std::time::Duration;

/// Identifies one [`PlayerEngine::load`] request, so late events about a
/// previously loaded file can be told apart from events about the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadId(pub u64);

/// Static description of a backend, for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineInfo {
    /// Human-readable backend name.
    pub name: String,
    /// `true` when the backend produces no audio or video.
    pub simulated: bool,
}

/// Display size of the current video, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSize {
    pub width: u32,
    pub height: u32,
}

/// Something the backend reports asynchronously.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    /// The file opened. Playback runs unless the engine is paused.
    Loaded {
        id: LoadId,
        duration: Option<Duration>,
    },
    /// Current playback position.
    Position(Duration),
    /// The current file's length changed after it loaded (estimates get refined).
    DurationChanged(Option<Duration>),
    /// The current file gained, lost or resized its picture; `None` means audio only.
    VideoChanged(Option<VideoSize>),
    /// Playback reached the end of the file.
    Ended { id: LoadId },
    /// The file could not be opened or decoded.
    Failed { id: LoadId, message: String },
}

/// A request the backend refused outright.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError(pub String);

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EngineError {}

/// A playback backend.
///
/// Commands are fire-and-forget; outcomes arrive through [`Self::poll_events`].
/// Implementations must not panic on any input.
pub trait PlayerEngine {
    fn info(&self) -> EngineInfo;

    /// Starts loading `path`, replacing the current file.
    ///
    /// [`crate::Player`] only passes absolute paths that pointed at a regular
    /// file when checked. `Err` means the request was refused; failures while
    /// opening are reported later as [`EngineEvent::Failed`].
    fn load(&mut self, path: &Path) -> Result<LoadId, EngineError>;

    /// Resumes playback. Like mpv's `pause` property, the play/pause state
    /// persists across loads.
    fn play(&mut self);

    fn pause(&mut self);

    /// Unloads the current file.
    fn stop(&mut self);

    fn seek(&mut self, position: Duration);

    /// Linear volume in `0.0..=1.0`.
    fn set_volume(&mut self, volume: f32);

    fn set_muted(&mut self, muted: bool);

    /// Returns the events produced since the previous call. Called once per frame.
    fn poll_events(&mut self) -> Vec<EngineEvent>;
}
