# ADR 0002: Event-driven engine boundary and a core player state machine

- Status: Accepted (slice 1)
- Date: 2026-11-09

## Context

Playback backends are asynchronous. libmpv accepts `loadfile` immediately,
reports success or failure later, emits position updates continuously, and can
deliver late events about the previous file after a new one was requested. The UI
must stay responsive, never crash on bad input, and be testable without a window
or a real decoder.

## Decision

- `mediaplayer-core` owns a `Player` state machine:
  `Idle → Loading → Playing ⇄ Paused → Stopped / Ended`, driven by
  `PlayerCommand`s. Commands that make no sense in the current state are ignored.
  Failures become a dismissible `PlayerError`; nothing panics.
- `PlayerEngine` is a small trait of fire-and-forget commands (`load`, `play`,
  `pause`, `stop`, `seek`, `set_volume`, `set_muted`) plus `poll_events()`,
  called once per frame. The play/pause state persists across loads, like mpv's
  `pause` property.
- `load` returns a `LoadId`. `Loaded`, `Ended` and `Failed` events carry it, so
  the player drops events about a file it no longer plays.
- The player validates paths (`validate_local_file`) before any engine sees them:
  absolute, existing, regular file. Engines still report their own failures,
  since files can disappear afterwards.
- The player updates its state optimistically on commands (pause shows at once)
  and reconciles from engine events.
- `NullEngine` simulates playback with an injectable `Clock`. It powers the UI
  before libmpv lands, and with `ManualClock` it makes tests deterministic.

## Consequences

- The whole command/state matrix is unit-tested against a recording fake engine
  (`player.rs` tests). The UI tests drive the real shell with `NullEngine`.
- Every engine has to translate its native events into four `EngineEvent`s. For
  mpv the mapping is listed in `docs/PLAN.md` (slice 2).
- Track lists, speed, A-B loop and subtitles (slice 5) will extend the trait.
  All implementations live in this repo, so changing it is cheap.
