# ADR 0003: Load libmpv at runtime, from trusted folders only

- Status: Accepted (implemented in slice 2)
- Date: 2026-11-09

## Context

libmpv is the preferred playback backend: one DLL covers practically every
container and codec through FFmpeg, it does hardware decoding, and it has an
OpenGL render API (see ADR 0001). Linking it at build time would make
`cargo build` fail on any machine without libmpv's import library and headers,
and the app would fail to start without the DLL.

## Decision

- Load the library at runtime with `libloading` (0.9): `libmpv-2.dll` on
  Windows, `libmpv.so.2` on Linux, `libmpv.2.dylib` on macOS. Resolve only the
  client and render API functions we use. Refuse to continue unless
  `mpv_client_api_version()` reports major version 2.
- **Windows DLL search hardening:** load `libmpv-2.dll` (or the older name
  `mpv-2.dll`) by absolute path from the executable's folder with
  `LoadLibraryExW` and `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`,
  so the current directory and `PATH` are never searched (DLL planting). If
  neither file is there, try `libmpv-2.dll` from System32 only. Log which file
  was loaded.
- If loading fails, the app still starts with `NullEngine` and says why (error
  bar and Settings), plus where to put the DLL.
- **mpv hardening** (`OPTIONS` in `crates/mpv/src/engine.rs`). These are
  required, and startup fails if libmpv rejects one: `config=no` (ignore a user's
  mpv.conf), `load-scripts=no` (no Lua/JS scripts), `ytdl=no` (never spawn
  yt-dlp), `access-references=no` (don't follow playlists, EDL files, archives
  or ordered chapters inside a file), `idle=yes` and `vo=libmpv`. These are
  best-effort, because not every mpv version has them: `input-default-bindings=no`,
  `input-vo-keyboard=no`, `osc=no`, built-in scripts off, `keep-open=no` (the
  player needs `END_FILE` when a file finishes), `hwdec=auto-safe`,
  `audio-fallback-to-null=yes` and the scaler options from ADR 0004. No IPC
  server is ever configured.
- Only absolute local paths reach `loadfile` (ADR 0002), passed as one argument
  of a command array, never parsed from a string. That keeps local names from
  being read as mpv protocols such as `edl://` or `av://`. Together with
  `access-references=no`, nothing reaches the network; slice 6 adds URLs behind
  an explicit allowlist.

## Consequences

- `cargo build` works on a clean Windows machine. libmpv is a runtime drop-in:
  copy one DLL next to `mediaplayer.exe`. No installer or package manager needed.
- `unsafe` is confined to the `mpv` crate behind a safe wrapper, with a scoped
  `#[allow(unsafe_code)]` (the workspace denies it elsewhere).
- **Licensing:** mpv is GPLv2+ by default, and LGPLv2.1+ only when built without
  GPL-only files. Linked libraries such as a GPL FFmpeg affect the result.
  Shipping a release bundle that includes a GPL libmpv build means distributing
  that bundle under GPL terms. Leaving the DLL out and documenting where to get it
  avoids bundling, but the owner still needs to choose this project's own licence
  with that in mind. (This is a summary, not legal advice.)

## Alternatives considered

- **Link-time linking** (`libmpv-sys`-style bindings): simpler code, but the
  build needs libmpv dev files and the app can't start or explain itself when
  the DLL is missing.
- **Bundling the DLL in the repo:** licence and update burden; GPL binary
  in-tree.
- **GStreamer:** larger runtime with separate plugin installs; rejected in
  ADR 0001.
