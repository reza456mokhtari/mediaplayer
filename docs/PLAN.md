# Plan

How this project is built: small vertical slices, each closed by the ECC loop.

1. **Plan** the slice (this file + an ADR when a decision is hard to reverse).
2. **Security review** anything touching files, paths, processes, DLLs or the network.
3. **TDD** for core state (player commands now, playlist ops in slice 3): tests first, watch them fail, then implement.
4. **Verification loop**: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, a Windows build, and screenshots of the running app for UI changes.
5. **Record** decisions and verification results in [`NOTES.md`](../NOTES.md) and [`docs/adr/`](adr/).
6. **Improve** what the slice exposed before starting the next one.

## Architecture

```
crates/
  core/   mediaplayer-core   no UI, no dependencies
    engine.rs        PlayerEngine trait, EngineEvent, LoadId
    player.rs        Player: PlayerCommand -> PlaybackState state machine
    null_engine.rs   NullEngine (simulated playback) + Clock / ManualClock
    media.rs         validate_local_file, MediaItem
    time.rs          format_timestamp
  app/    mediaplayer-app    egui shell, binary `mediaplayer`
    app.rs           panels, routing, input plumbing (drops, hotkeys, commands)
    theme.rs         design tokens -> egui Style, dark + light
    hotkeys.rs       shortcut table
    cli.rs           argument parsing
    ui/              nav, now_playing, transport, pages, info_bar, widgets, icons
planned:
  mpv/              slice 2: libmpv engine (runtime-loaded, OpenGL render API)
  library/          slice 4: folder scanner + SQLite index
```

One frame:

```
keys / drops / clicks ──► PlayerCommand ──► Player::dispatch ──► PlayerEngine (commands)
PlayerEngine::poll_events ──► Player::tick ──► Player state ──► UI draws
```

Rules:

- `core` never depends on egui; every player rule is testable without a window.
- The UI never talks to an engine directly; it sends `PlayerCommand`s and reads `Player` state.
- Engines only receive absolute paths to regular files that passed `validate_local_file`.
- Nothing panics on user input. Failures become a `PlayerError` that the UI shows and the user can dismiss.

## Slices

| Slice | Scope | Status |
|---|---|---|
| 0 | Workspace, egui shell window, navigation placeholders, theme tokens, empty Now Playing, transport chrome | Done |
| 1 | `PlayerEngine` trait, simulated `NullEngine`, `Player` state machine (TDD), transport and keyboard wired to state | Done |
| 2 | libmpv engine: runtime DLL loading, OpenGL render API into an egui texture, open-file dialog, real play/pause/seek/volume | Next |
| 3 | Play queue: add/remove/reorder, open folder, M3U save/load, previous/next, repeat | |
| 4 | Library scan on a background thread, SQLite index, durations, search | |
| 5 | Fullscreen video, external `.srt` and embedded subtitle/audio tracks, playback speed, A-B loop | |
| 6 | Network streams, persisted settings, configurable hotkeys, README licenses and limits | |
| 7 | Windows 11 style Now Playing polish, performance pass | |

## Slice 2 outline (libmpv)

See [ADR 0003](adr/0003-libmpv-runtime-loading.md) for the decision and its security notes.

- New crate `crates/mpv`, the only crate allowed `unsafe` (scoped `#[allow(unsafe_code)]`, each block with a `SAFETY:` comment).
- Load `libmpv-2.dll` / `libmpv.so.2` / `libmpv.2.dylib` at runtime with `libloading`. On Windows, load only from the executable's folder or System32 (`LoadLibraryExW` search flags), never from the working directory.
- If loading fails, start with `NullEngine` and show why in the error bar and in Settings.
- Map mpv to `EngineEvent`: `MPV_EVENT_FILE_LOADED` → `Loaded`, observed `time-pos` → `Position`, `MPV_EVENT_END_FILE` with reason EOF → `Ended`, with reason error → `Failed`. Use the playlist entry id as `LoadId`.
- Render video with `MPV_RENDER_API_TYPE_OPENGL` into an FBO, show it as an egui texture, and request repaints from mpv's update callback.
- Add an open-file dialog (`rfd`) and `Ctrl+O`.
- Tests: the existing `Player` suite stays engine-agnostic. Add an mpv smoke test that's skipped when the library is missing, plus a manual checklist for mp4/mkv/mp3.

## Acceptance tracking

| Criterion | Slice | Status |
|---|---|---|
| `cargo build` works on Windows | 0 | Cross-compiled (`x86_64-pc-windows-gnu`) and `cargo check` for MSVC in the sandbox; native MSVC build runs in CI |
| Open local mp4/mkv/mp3 and play | 2 | Simulated playback only (NullEngine) |
| Play/pause/seek/volume from UI and keyboard | 1 → 2 | Wired and tested against the simulated engine |
| Playlist add/remove/reorder | 3 | |
| Library scan with durations | 4 | |
| Fullscreen video | 5 | |
| External SRT subtitles | 5 | |
| Speed control | 5 | |
| No crash on missing file; error shown | 1 | Done (unit + UI tests, screenshot) |
| README: run instructions, backend deps, license notes | 0 → 6 | First version |
| No "official Microsoft/VLC" claim | all | README and Settings say independent and not affiliated |
| ECC verification notes per slice | all | `NOTES.md` |
