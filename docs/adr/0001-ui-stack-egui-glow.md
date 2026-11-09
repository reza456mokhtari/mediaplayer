# ADR 0001: UI stack is eframe/egui 0.37 with the glow (OpenGL) renderer

- Status: Accepted (slice 0)
- Date: 2026-11-09

## Context

The app needs a native Windows UI written in Rust, with video from libmpv drawn
*inside* the UI so controls can overlay it (the Windows 11 style fullscreen view
in slice 5). libmpv's render API draws into OpenGL framebuffers, or into CPU
memory (`MPV_RENDER_API_TYPE_SW`), which is too slow for HD video.

## Decision

- Use `eframe` 0.37 / `egui` 0.37 with `default-features = false` and the
  `glow` renderer (OpenGL via glutin), not the default `wgpu` renderer.
- Enabled eframe features: `accesskit` (Narrator / screen readers, and the
  accessibility tree the UI tests query), `default_fonts`, `system_fonts`
  (fallback glyphs for non-Latin file names), `glow`, `x11`, `wayland`.
- On Linux, `crates/app` also enables winit's `wayland-dlopen` and
  `wayland-csd-adwaita` directly (same winit version as eframe). Turning off
  eframe's defaults drops them, while accesskit still enables Wayland, and
  without dlopen `cargo build` would need `libwayland-dev`.

## Consequences

- One OpenGL context is shared by egui and (in slice 2) libmpv's render
  context, so a video frame is just another egui texture: overlays,
  rounded corners and animations are free.
- Smaller dependency tree and faster builds than with wgpu.
- Requires OpenGL 3.x (or GLES 2) drivers. Some Windows VMs and remote sessions
  without GPU drivers only have the GDI OpenGL 1.1 fallback, and the window will
  fail to open there. The workaround is a Mesa llvmpipe `opengl32.dll` next to
  the exe. We'll revisit (ANGLE, or a wgpu path with GL interop) if users hit it.
- The build needs no system packages on Windows or Linux: X11, Wayland, EGL/GLX
  and fontconfig are all loaded at runtime.

## Alternatives considered

- **wgpu + mpv `wid` embedding** (mpv renders into a child HWND with d3d11):
  best decoding path, but the native child window covers egui ("airspace"),
  so there are no overlays on video.
- **wgpu + mpv software rendering**: CPU-bound, unacceptable for 4K.
- **wgpu + GL/D3D interop**: possible later; too much complexity for the MVP.
- **GStreamer backend**: needs a large runtime install with separate plugin sets
  for common codecs; mpv ships as a single DLL.
