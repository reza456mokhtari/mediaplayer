//! A backend that decodes nothing and simulates the passage of time.
//!
//! It lets the UI and the state machine run before a real backend exists, and
//! with [`ManualClock`] it makes playback tests deterministic.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::engine::{EngineError, EngineEvent, EngineInfo, LoadId, PlayerEngine};

/// Length reported for every file opened with [`NullEngine::new`].
pub const DEFAULT_SIMULATED_DURATION: Duration = Duration::from_secs(3 * 60 + 30);

/// A monotonic time source.
pub trait Clock {
    /// Time elapsed since an arbitrary fixed origin.
    fn now(&self) -> Duration;
}

/// Real time, measured from construction.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock(Instant);

impl SystemClock {
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// A clock that only moves when told to. Clones share the same time.
#[derive(Debug, Clone, Default)]
pub struct ManualClock(Rc<Cell<Duration>>);

impl ManualClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance(&self, by: Duration) {
        self.0.set(self.0.get() + by);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}

/// Simulated backend: every file "plays" in silence for a fixed duration.
pub struct NullEngine {
    clock: Box<dyn Clock>,
    media_duration: Duration,
    current: Option<LoadId>,
    loads: u64,
    playing: bool,
    ended: bool,
    /// Position at clock time `anchor`; while running, the live position
    /// is extrapolated from it.
    anchor_position: Duration,
    anchor: Duration,
    events: Vec<EngineEvent>,
}

impl NullEngine {
    pub fn new() -> Self {
        Self::with_clock(Box::new(SystemClock::new()), DEFAULT_SIMULATED_DURATION)
    }

    pub fn with_clock(clock: Box<dyn Clock>, media_duration: Duration) -> Self {
        let anchor = clock.now();
        Self {
            clock,
            media_duration,
            current: None,
            loads: 0,
            playing: false,
            ended: false,
            anchor_position: Duration::ZERO,
            anchor,
            events: Vec::new(),
        }
    }

    fn running(&self) -> bool {
        self.current.is_some() && self.playing && !self.ended
    }

    fn position(&self) -> Duration {
        if self.running() {
            let elapsed = self.clock.now().saturating_sub(self.anchor);
            (self.anchor_position + elapsed).min(self.media_duration)
        } else {
            self.anchor_position
        }
    }

    fn set_position(&mut self, position: Duration) {
        self.anchor_position = position.min(self.media_duration);
        self.anchor = self.clock.now();
    }
}

impl Default for NullEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerEngine for NullEngine {
    fn info(&self) -> EngineInfo {
        EngineInfo {
            name: "Null backend (simulated)".to_owned(),
            simulated: true,
        }
    }

    fn load(&mut self, _path: &Path) -> Result<LoadId, EngineError> {
        self.loads += 1;
        let id = LoadId(self.loads);
        self.current = Some(id);
        self.ended = false;
        self.set_position(Duration::ZERO);
        self.events.push(EngineEvent::Loaded {
            id,
            duration: Some(self.media_duration),
        });
        Ok(id)
    }

    fn play(&mut self) {
        if !self.playing {
            let position = self.position();
            self.set_position(position);
            self.playing = true;
        }
    }

    fn pause(&mut self) {
        if self.playing {
            let position = self.position();
            self.set_position(position);
            self.playing = false;
        }
    }

    fn stop(&mut self) {
        self.current = None;
        self.ended = false;
        self.set_position(Duration::ZERO);
    }

    fn seek(&mut self, position: Duration) {
        if self.current.is_some() {
            self.ended = false;
            self.set_position(position);
        }
    }

    fn set_volume(&mut self, _volume: f32) {}

    fn set_muted(&mut self, _muted: bool) {}

    fn poll_events(&mut self) -> Vec<EngineEvent> {
        if let Some(id) = self.current
            && self.running()
        {
            let position = self.position();
            self.events.push(EngineEvent::Position(position));
            if position >= self.media_duration {
                self.set_position(position);
                self.ended = true;
                self.events.push(EngineEvent::Ended { id });
            }
        }
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EngineEvent::{Ended, Loaded, Position};

    const LENGTH: Duration = Duration::from_secs(60);

    fn engine() -> (NullEngine, ManualClock) {
        let clock = ManualClock::new();
        (
            NullEngine::with_clock(Box::new(clock.clone()), LENGTH),
            clock,
        )
    }

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn song() -> &'static Path {
        Path::new("/media/song.mp3")
    }

    #[test]
    fn load_reports_the_simulated_duration() {
        let (mut engine, _) = engine();
        let id = engine.load(song()).unwrap();
        assert_eq!(
            engine.poll_events(),
            [Loaded {
                id,
                duration: Some(LENGTH)
            }]
        );
    }

    #[test]
    fn each_load_gets_a_new_id() {
        let (mut engine, _) = engine();
        let first = engine.load(song()).unwrap();
        let second = engine.load(song()).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn position_advances_only_while_playing() {
        let (mut engine, clock) = engine();
        engine.load(song()).unwrap();
        engine.play();
        engine.poll_events();
        clock.advance(secs(10));
        assert_eq!(engine.poll_events(), [Position(secs(10))]);
        engine.pause();
        clock.advance(secs(5));
        assert!(engine.poll_events().is_empty());
        engine.play();
        clock.advance(secs(2));
        assert_eq!(engine.poll_events(), [Position(secs(12))]);
    }

    #[test]
    fn seek_moves_the_clock() {
        let (mut engine, clock) = engine();
        engine.load(song()).unwrap();
        engine.play();
        engine.poll_events();
        engine.seek(secs(30));
        clock.advance(secs(1));
        assert_eq!(engine.poll_events(), [Position(secs(31))]);
    }

    #[test]
    fn reaching_the_end_reports_ended_once() {
        let (mut engine, clock) = engine();
        let id = engine.load(song()).unwrap();
        engine.play();
        engine.poll_events();
        clock.advance(secs(75));
        assert_eq!(engine.poll_events(), [Position(LENGTH), Ended { id }]);
        clock.advance(secs(5));
        assert!(engine.poll_events().is_empty());
    }

    #[test]
    fn seeking_past_the_end_clamps_and_ends() {
        let (mut engine, _) = engine();
        let id = engine.load(song()).unwrap();
        engine.play();
        engine.poll_events();
        engine.seek(secs(600));
        assert_eq!(engine.poll_events(), [Position(LENGTH), Ended { id }]);
    }

    #[test]
    fn stop_unloads() {
        let (mut engine, clock) = engine();
        engine.load(song()).unwrap();
        engine.play();
        engine.poll_events();
        engine.stop();
        clock.advance(secs(5));
        assert!(engine.poll_events().is_empty());
    }

    #[test]
    fn play_state_persists_across_loads() {
        let (mut engine, clock) = engine();
        let first = engine.load(song()).unwrap();
        clock.advance(secs(5));
        // Not playing yet, so no position updates.
        assert_eq!(
            engine.poll_events(),
            [Loaded {
                id: first,
                duration: Some(LENGTH)
            }]
        );
        engine.play();
        let second = engine.load(song()).unwrap();
        assert_eq!(
            engine.poll_events(),
            [
                Loaded {
                    id: second,
                    duration: Some(LENGTH)
                },
                Position(Duration::ZERO)
            ]
        );
    }
}
