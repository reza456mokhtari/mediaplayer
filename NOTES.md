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

## 2026-11-09: Slice 2 (libmpv)

### Decisions

- **D8** libmpv is loaded at runtime from trusted folders only. [ADR 0003](docs/adr/0003-libmpv-runtime-loading.md)
  is now accepted and matches the code (option table, `keep-open=no`, System32
  fallback, logging of the loaded DLL).
- **D9** Video: libmpv's OpenGL render API draws into our FBO, and an egui paint
  callback blits it into egui's framebuffer ([ADR 0004](docs/adr/0004-video-output.md)).
  The plan said "egui texture"; the blit avoids re-registering a texture on every
  resize.
- **D10** Scalers are `catmull_rom` with `correct-downscaling=no`, which avoids a
  libmpv bug that blanks the picture (investigation below, ADR 0004).
- **D11** New dependencies: libloading 0.9 (ISC), glow 0.17 (MIT OR Apache-2.0 OR
  Zlib; the version eframe uses) and rfd 0.18 (MIT; `xdg-portal` only, so Linux
  builds need no GTK headers). The build still needs no libmpv and no system packages.
- **D12** `mediaplayer-core` gained `DurationChanged` and `VideoChanged` events, so
  the UI can show the duration and size the video area. It still has no dependencies.

### Security review: libmpv and the Open dialog

| Surface | Handling | Covered by |
|---|---|---|
| DLL loading (Windows) | Absolute path next to the exe, `LoadLibraryExW` with `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR \| LOAD_LIBRARY_SEARCH_SYSTEM32`, then System32 only; never the current directory or `PATH`. The loaded path is logged | `library` tests (candidate names), MSVC clippy. Not run on Windows yet |
| Library version | Refused unless the client API major version is 2 | `library` tests |
| mpv configuration | `config`, `load-scripts`, `ytdl` and `access-references` off, all required; no IPC server or script options anywhere in the table | `engine` tests |
| Nested opens | `access-references=no` makes FFmpeg's nested opens fail (checked in mpv 0.37's `demux_lavf.c`: `block_io_open`), so a local HLS or playlist file can't reach the network | Source review |
| Paths into libmpv | Validated absolute paths only, as one `loadfile` argument (never parsed from a string); raw bytes on Unix, UTF-8 on Windows; NUL bytes rejected | `engine` tests, smoke tests |
| FFI | `unsafe` only in `crates/mpv`, each block with a `SAFETY:` comment (clippy `undocumented_unsafe_blocks`). Callbacks only wake the UI. The render context is freed before the core, with the GL context current | clippy, review |
| Open dialog | Native dialog (rfd; the desktop portal on Linux); the chosen file is validated like any other | UI tests for the button and Ctrl+O |
| Logging | libmpv warnings and errors go to `log` (stderr, local only), and mpv messages can include the file being played | n/a |

### Investigation: black video

- **Symptom:** libmpv loaded and delivered frames, but under Xvfb with Mesa
  llvmpipe the video area stayed black for a whole run in most runs. The other
  runs were fine throughout.
- **Ruled out** (batches of 4 parallel runs; black runs in brackets): baseline
  (3/4, repeatedly), Mesa shader cache off (3/4), `LP_NUM_THREADS=0` (4/4),
  `vd-lavc-dr=no` (1/4), `hwdec=no` (2/4). Rendering into the default
  framebuffer instead of our FBO was black in black runs too (4/4), so the FBO
  wasn't the cause. An earlier note blaming the FBO came from one run that
  happened to be fine.
- mpv's `debug` logs were line-for-line identical in black and colour runs.
- **apitrace:** the GL calls and arguments were identical through the first frame.
  The one difference was the contents of an 8 KiB upload: the 2×256 RGBA16F weight
  table of the 6-tap chroma scaler. Its padding (taps 6 and 7 of each row) held
  NaN in the black run and other garbage in the colour run. In the black run, the
  rows the chroma pass samples (63/64 and 191/192) had NaN in the padding.
- **Cause:** a libmpv bug. `reinit_scaler` allocates the table uninitialised and
  `mp_compute_lut` fills only the real taps; mpv master still does this. Linear
  filtering mixes the padding texel in with weight 0, `0 × NaN = NaN` reaches the
  weights that are used, and the whole frame turns black. Details in ADR 0004.
- **Fix:** `catmull_rom` for `scale`, `cscale` and `dscale` (4 taps, no padding),
  plus `correct-downscaling=no`. The temporary diagnostic code was removed.

### Verification: slice 2

- **TDD red:** `scalers_use_kernels_without_lut_padding` failed ("option scale is
  not set"). **Green** after adding the options.
- `cargo fmt --all --check`: pass. `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test --workspace`: 82 passed, 0 failed, 4 ignored (core 43 + 1
  integration, app 6 + 11 UI wiring, mpv 21).
- `cargo test -p mediaplayer-mpv -- --ignored` against Ubuntu's libmpv 0.37: 4
  passed. These cover libmpv accepting the scaler options; play, pause, seek and
  finish; an undecodable file reporting an error; and a quick second open.
- Windows: `cargo check` and `cargo clippy --workspace --all-targets -- -D warnings`
  for `x86_64-pc-windows-msvc` pass, covering the `LoadLibraryExW` and UTF-8 path
  code. `cargo build --release --target x86_64-pc-windows-gnu` produces a PE32+ GUI
  `mediaplayer.exe`.
- **Video**, Xvfb + llvmpipe, 4 parallel runs at a time:
  - Before the fix, most runs were black.
  - After it, 16 of 16 runs showed the picture: 8 MP4 (H.264 720p, AAC) and 8 MKV
    (VP9 640×480, Opus). The centre of the video area measured 0.507–0.509 mean
    brightness, against 0.000 for a black run.
  - Screenshots show 16:9 letterboxed and 4:3 pillarboxed, both upright, and the
    test pattern's burned-in clock matches the seek bar.
- **Controls**, real key presses (xdotool) against libmpv:
  - Space pauses: two screenshots 2 s apart are pixel-identical. Space again resumes.
  - Shift+→ seeks from 0:04 to 0:36. ↓×3 lowers the volume. M shows the muted icon.
  - No libmpv command failures in the logs.
- **MP3:** the audio-only Now Playing view appears, the clock advances, and
  libmpv reports the 0:30 duration.

**Not verified here:**

- Audible output. The sandbox has no audio device, so libmpv used its null output (`audio-fallback-to-null`).
- Windows at runtime: loading `libmpv-2.dll`, playback and hardware decoding. CI builds and tests on `windows-latest`, without libmpv.
- Real GPUs. Only Mesa llvmpipe was available.

### Pre-merge review

- An independent review of `crates/mpv` found a bug: replacing a video with
  another of the same size could leave the player audio-only for the whole file.
  - Cause: libmpv coalesces property changes (client.h), so it may never report
    `dwidth`/`dheight` for the new file, while the player forgets the picture on
    every open.
  - Confirmed with a ctypes probe against libmpv 0.37. Reloading the same file
    after 0.6 s without polling sent no `dwidth`/`dheight` change, but it did send
    `VIDEO_RECONFIG`, and `dwidth` read as a double returned 1280.
  - Fix: the engine reads `dwidth`/`dheight` on every `VIDEO_RECONFIG`, and the
    event mapper reports the size afresh for each file.
  - TDD: two mapper tests failed (`left: None`), then passed.
- Reopening the same 720p file in the app through Ctrl+O (zenity under Xvfb):
  - With the fix, the picture returned in both valid runs.
  - The one valid pre-fix run showed it too: in the app, the UI usually polls
    between the two files, so the race is rare there. The unit tests and the probe
    are the deterministic evidence.
- Closing the window during playback now has a test; earlier runs were always
  killed, which skips `on_exit`. MP4 on Now Playing, MP4 in the transport-bar
  thumbnail, MP3, and closing after a failed Ctrl+O all exit with status 0 in
  0.1–0.3 s, with no renderer-leak warning.
- On Linux without a desktop portal or zenity, Ctrl+O fails quietly (it's logged)
  and the UI stays responsive. The README now says so. rfd loads libdbus at
  runtime, so the build still needs no system packages.
- `cargo test --workspace`: 84 passed, 0 failed, 4 ignored. The 4 ignored libmpv
  tests pass when run. Clippy `-D warnings` passes for Linux and for
  `x86_64-pc-windows-msvc`.

### Improve before slice 3

- Keep libmpv's render loop serviced whenever a video file is active. Until the
  video size is known it isn't, so libmpv waits 200 ms and drops the first frame.
- libmpv's hardware-decoding and audio probing failures (CUDA, VA-API, ALSA, JACK)
  arrive as ERROR logs even when playback works. Demote or summarise known probe noise.
- `set_volume` unmutes on any change while muted, including lowering the volume,
  but the test is called "raising the volume unmutes". Decide which is intended.
- Report the uninitialised scaler padding upstream (mpv `video/out/gpu/video.c`),
  then version-gate the workaround.
- Run the ignored libmpv tests on Windows CI against a pinned, checksummed `libmpv-2.dll`.
- `MpvEngine::create_video_output`, `VideoOutput::paint` and `VideoOutput::destroy`
  are safe functions with OpenGL requirements the compiler can't check:
  - the creating context must be current;
  - `get_proc_address` must return real GL functions.

  Breaking them is undefined behaviour (render.h). The app meets them. Marking them
  `unsafe` would push `unsafe` into the app, so instead move the egui paint
  callback and the context handling into `crates/mpv`.
- The libmpv callback wrappers don't catch panics, so a panicking closure aborts
  the process. That's defined behaviour, but abrupt. Add `catch_unwind` along with
  the change above.
