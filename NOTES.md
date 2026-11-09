# Engineering notes (ECC log)

Short, dated entries: decisions, security review, verification evidence, and
what to improve before the next slice. Longer decisions live in [docs/adr/](docs/adr/).

## 2026-11-09: Slices 0 and 1

### Decisions

- **D1** eframe/egui 0.37 with the glow renderer, default features off ([ADR 0001](docs/adr/0001-ui-stack-egui-glow.md)).
- **D2** Event-driven `PlayerEngine` plus a core `Player` state machine; `LoadId` drops stale events ([ADR 0002](docs/adr/0002-engine-boundary-and-player-state-machine.md)).
- **D3** libmpv loaded at runtime from trusted folders only (proposed for slice 2, [ADR 0003](docs/adr/0003-libmpv-runtime-loading.md)).
- **D4** Minimal dependencies. `mediaplayer-core` has none; the app uses
  eframe, log and env_logger (no default features); egui_kittest is test-only.
  No thiserror, tempfile or clap. The build needs no system packages
  (Wayland, GL and fontconfig are loaded at runtime).
- **D5** Lints: `unsafe_code = "deny"` workspace-wide (not `forbid`, so slice 2's
  FFI module can use a scoped allow), clippy `all` plus `unwrap_used`. CI runs
  clippy with `-D warnings`.
- **D6** Clean-room UI. The layout hierarchy comes from general knowledge of
  desktop players; colours and icons are original and drawn in code. The app is
  called `mediaplayer`, and both Settings and the README state it is independent.
- **D7** No REA performed; nothing needed it.

### Security review: file and path handling

| Surface | Handling | Covered by |
|---|---|---|
| Command-line files | `args_os` (non-UTF-8 safe); unknown `-x` options rejected; `--` ends options | `cli` tests |
| Drag and drop | First dropped path only, then the same validation as the CLI | Manual |
| Path validation | `std::path::absolute` (no `\\?\` prefixes); `fs::metadata` follows symlinks; only regular files pass, so folders, devices and FIFOs can't block a backend; failures become user-facing messages | `media` and `player` tests |
| Time-of-check/time-of-use | A file can vanish after validation, so engines must report `Failed`; Play after Stop/End re-validates | `player` tests |
| Engine input | Engines only see absolute, validated paths, so names can't be read as mpv protocols or options later | ADR 0003 |
| Processes, network, DLLs | None in slices 0–1 | n/a |
| Logging | No paths or user data logged yet | n/a |
| Panics | No `unwrap`/`expect` outside tests (clippy `unwrap_used`); `Duration::from_secs_f64` only gets clamped, finite, non-negative values | clippy |

Carried forward: DLL search hardening and mpv option hardening (slice 2, ADR 0003),
size limits on M3U input and keeping remote playlists from referencing local files
(slice 3), parameterised SQLite only (slice 4), a URL scheme allowlist (slice 6).

### Verification: slice 0 (shell)

- `cargo build --workspace --all-targets`: pass (Linux host).
- `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- UI test `shell_shows_navigation_and_an_empty_now_playing_page`: pass. kittest's
  accessibility check also ran on every `Harness::run`: every input widget has an
  accessible name.
- Screenshots under Xvfb with Mesa llvmpipe: dark empty state, Settings in the
  light theme, Music library placeholder.
- Windows: `cargo build --release --target x86_64-pc-windows-gnu` passed and produced
  `mediaplayer.exe` (PE32+ GUI, x86-64, 8.4 MB). `cargo check --target
  x86_64-pc-windows-msvc --workspace --all-targets` passed. **Not run here:** a
  native MSVC link and launching on Windows; the `windows-latest` CI job covers
  the build.
- Screenshot review found four defects, all fixed and re-captured: full-width
  status chip, misaligned error bar, black corner behind the rounded content layer,
  boxy slider handles.

### Verification: slice 1 (engine and state machine)

- **TDD red:** the 18 `Player` tests and the integration test were written
  against an API-only stub. `cargo test -p mediaplayer-core`: lib 20 passed /
  16 failed, integration 0/1. The two `Player` tests that passed ("starts idle",
  "idle ignores transport") are ones a no-op stub satisfies.
- **TDD green:** after the implementation, `cargo test --workspace` gives 50
  passed, 0 failed (core: 36 unit + 1 integration; app: 6 CLI + 7 UI wiring).
- UI wiring tests drive the real `MediaPlayerApp` with `NullEngine` and
  `ManualClock`:
  - buttons: Play, Pause, Mute, Stop
  - keys: Space, ←/→, Shift+→, ↓, M, S
  - the elapsed label follows the clock
  - a missing file shows the error bar, and Dismiss clears it
- Screenshots: simulated playback at 0:05 of 3:30 with the Pause button showing;
  the missing-file error bar; playback keeps running through a theme switch and
  page changes.

### Improve before slice 2

- Slider thumbs are grey circles; a Windows 11 style thumb with an accent centre
  needs a custom seek bar (slice 7, or sooner if it's cheap).
- Radio buttons render as filled grey wells; restyle them as outlined/accent in slice 6.
- On Windows, load Segoe UI Variable from the system when it's present (slice 7).
- Persist theme and volume (slice 6).
- Have `Player::tick` report whether state changed, so idle frames can skip work (performance pass).

## 2026-11-09: Published to GitHub

- Pushed `hoplite/kranioi-ff9b6477`. The repository was empty, so GitHub made
  this branch the default. Renaming it to `main` (repository Settings → General →
  Default branch) matches CI's `push` trigger.
- The first push didn't register the CI workflow: the Actions API listed 0
  workflows and `workflow_dispatch` returned 404. Presumably the branch wasn't
  the default yet when GitHub processed that push. Pushing the default branch a
  second time registers it, and the first Windows and Linux run is started by hand.
- CI jobs now time out after 45 minutes instead of GitHub's default of 6 hours.
