# mediaplayer

An independent, open-source desktop media player for Windows, written in Rust
with [egui](https://github.com/emilk/egui). Playback is designed around
[libmpv](https://mpv.io/), which arrives in slice 2.

> **Not affiliated with Microsoft or VideoLAN.** mediaplayer is not Windows Media
> Player, the Windows 11 Media Player app, or VLC, and none of their makers
> endorse it. The layout follows common desktop-player conventions (navigation
> pane, Now Playing view, bottom transport bar). All code, icons and colours in
> this repository are original.

## Status

| Slice | What works |
|---|---|
| 0 | Shell window, navigation with placeholder pages, light/dark design tokens, Now Playing empty state, transport bar |
| 1 | Player state machine and `PlayerEngine` trait with a **simulated backend**; transport buttons and keyboard drive real state |

Playback is **simulated** for now. Opening a file checks it and runs a 3:30
clock, but nothing is decoded (no sound or picture) until the libmpv backend
lands in slice 2. The roadmap is in [docs/PLAN.md](docs/PLAN.md).

## Build and run

### Windows

1. Install Rust 1.95 or newer with [rustup](https://rustup.rs). The default
   `x86_64-pc-windows-msvc` toolchain needs the Visual Studio C++ build tools,
   and rustup's installer offers to set them up. The `x86_64-pc-windows-gnu`
   toolchain ships its own linker instead.
2. From the repository root:

```powershell
cargo run --release -p mediaplayer-app                         # start empty
cargo run --release -p mediaplayer-app -- "C:\Music\song.mp3"  # open a file on start
cargo test --workspace                                         # all tests, no window needed
```

The binary is `target\release\mediaplayer.exe`. No system packages are needed:
every dependency is a Rust crate that cargo fetches.

### Linux (development)

Same commands. At runtime it needs OpenGL drivers (Mesa); nothing extra at build
time. X11 is tested. Wayland support is compiled in but untested.

### Using it

- Drag a file onto the window, or pass it on the command line. Options are listed
  by `mediaplayer --help`.
- Set `RUST_LOG=info` to see logs. On Windows only debug builds have a console, so
  `--help` output and logs show up there and not in release builds.

### Keyboard shortcuts

| Key | Action |
|---|---|
| Space | Play / pause |
| ← / → | Seek 5 s |
| Shift + ← / → | Seek 30 s |
| ↑ / ↓ | Volume ±5 % |
| M | Mute / unmute |
| S | Stop |

## Playback backend: libmpv (slice 2)

mediaplayer will load libmpv **at runtime**. Building never needs libmpv
headers or import libraries, and when the library is missing the app still
starts (on the simulated backend) and says what's wrong. The design and its
security hardening are in [ADR 0003](docs/adr/0003-libmpv-runtime-loading.md).

### Windows install notes

There's no installer and no package manager: libmpv is one DLL that sits next to
the executable.

1. Open the [shinchiro/mpv-winbuild-cmake releases](https://github.com/shinchiro/mpv-winbuild-cmake/releases)
   (community builds, updated often; the latest when this was written was `20261109`).
2. Download the **`mpv-dev-…`** archive for your CPU, for example
   `mpv-dev-x86_64-20261109-git-8bc468896e.7z`:
   - `x86_64`: any 64-bit Intel/AMD CPU (the safe choice)
   - `x86_64-v3`: newer CPUs with AVX2 (faster, but won't run on older CPUs)
   - `aarch64`: Windows on Arm (needs an ARM64 build of mediaplayer too)

   The archives without `dev` in the name contain the mpv player, not the library.
3. Extract it with [7-Zip](https://www.7-zip.org/) and copy `libmpv-2.dll` next to
   `mediaplayer.exe` (`target\release\` or `target\debug\` when developing).
4. Start mediaplayer. Settings → Playback backend shows which backend is active.

For security, mediaplayer will only load libmpv from its own folder or System32,
never from the current directory or `PATH`.

### Linux and macOS

- Debian/Ubuntu: `sudo apt install libmpv2` (provides `libmpv.so.2`)
- macOS with Homebrew: `brew install mpv` (provides `libmpv.2.dylib`)

### What about GStreamer?

It isn't used. It was considered as the backend and rejected because it needs a
large runtime plus separate codec plugin sets (see [ADR 0001](docs/adr/0001-ui-stack-egui-glow.md)).

## Licenses

- **This project:** no licence has been chosen yet. Until a `LICENSE` file is
  added, all rights are reserved by the owner. The choice should take the libmpv
  note below into account.
- **Rust dependencies:** egui, eframe, egui_kittest, env_logger and log are
  MIT OR Apache-2.0; winit is Apache-2.0. `cargo tree` lists everything.
- **Fonts:** egui's bundled fonts: Ubuntu-Light (Ubuntu Font Licence 1.0), Hack
  (MIT), Noto Emoji (SIL OFL 1.1) and emoji-icon-font (MIT). Glyphs missing from
  them are borrowed at runtime from fonts installed on the system; none are bundled.
- **Icons and artwork:** drawn in code (`crates/app/src/ui/icons.rs`). There are
  no third-party or proprietary assets.
- **libmpv (not bundled):** mpv is GPLv2+ by default, and LGPLv2.1+ only when
  built without its GPL-only parts. Linked libraries such as a GPL FFmpeg can also
  affect the final licence. Treat prebuilt Windows DLLs, shinchiro's included, as
  GPL unless the build says otherwise. If you distribute mediaplayer *together
  with* a GPL libmpv DLL, that distribution must meet the GPL's terms (source
  availability, licence notices). See mpv's
  [Copyright file](https://github.com/mpv-player/mpv/blob/master/Copyright).
  This is a summary, not legal advice.

## Current limits

- No real decoding yet (simulated backend). Slice 2.
- No file picker yet: use drag and drop or the command line. Slice 2.
- One file at a time; a multi-file drop opens the first file. Play queue in slice 3.
- egui doesn't shape complex scripts, so Persian and Arabic file names show
  their letters, but unjoined and not right-to-left.
- The OpenGL renderer needs OpenGL 3 drivers. Some VMs and remote sessions without
  GPU drivers can't open the window ([ADR 0001](docs/adr/0001-ui-stack-egui-glow.md)).
- Settings such as the theme aren't saved between runs yet. Slice 6.

## Project docs

- [docs/PLAN.md](docs/PLAN.md): architecture, module map, slices, acceptance tracking
- [docs/adr/](docs/adr/): architecture decisions
- [NOTES.md](NOTES.md): ECC log with decisions, security review and verification notes per slice
