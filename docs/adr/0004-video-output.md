# ADR 0004: Video through libmpv's OpenGL render API, blitted in an egui paint callback

- Status: Accepted (slice 2)
- Date: 2026-11-09

## Context

The UI draws with egui's glow (OpenGL) renderer (ADR 0001), and libmpv is the
playback engine (ADR 0003). Video has to appear inside the egui layout: in the
Now Playing area, and as a thumbnail in the transport bar on other pages. libmpv's
render API (`MPV_RENDER_API_TYPE_OPENGL`) draws a frame into a framebuffer of the
caller's OpenGL context, on the caller's thread. Without a dedicated child window,
it's the only way to share egui's context.

## Decision

- `VideoOutput` (`crates/mpv/src/video.rs`) owns the `mpv_render_context` and an
  RGBA8 texture framebuffer (FBO) sized to the on-screen rectangle in physical pixels.
- It's created in eframe's app-creation closure, where the context is current,
  before any file loads, and freed in `App::on_exit` with the context current,
  before the engine.
- `ui::video::show` adds an egui paint callback. In it:
  1. Reset the GL state libmpv relies on. `render_gl.h` says libmpv expects the
     OpenGL state to be "reasonably set to OpenGL standard defaults", but egui
     paints with its own program, vertex array, buffers and texture bound, with
     blending and scissor on.
  2. Render only when `mpv_render_context_update` reports a new frame or the FBO
     was just (re)allocated, with `flip_y = 1` and `block_for_target_time = 0`
     (plus `video-timing-offset=0`), so the UI thread never waits for display time.
  3. `glBlitFramebuffer` the FBO into egui's framebuffer, scissored to the
     callback's clip rectangle, then restore egui's framebuffer binding.
- libmpv's update callback only calls `request_repaint` and never calls back into
  libmpv.
- **Scalers:** `scale`, `cscale` and `dscale` are `catmull_rom`, and
  `correct-downscaling=no`. The reason is a libmpv defect, found while
  investigating black video:
  - libmpv's `gpu` renderer stores separable-scaler weights in an RGBA16F
    lookup texture, `ceil(taps / 4)` texels per row. `reinit_scaler` allocates it
    with `talloc_array` and `mp_compute_lut` writes only `taps` values per row, so
    the padding is uninitialised. This was still the case in mpv master on 2026-11-09.
  - With 6 or 10 taps, the shader samples the first texel of each row with
    linear filtering, and the padded neighbour texel enters the interpolation with
    weight 0. If the padding holds NaN or ±Inf, `0 × NaN = NaN` poisons a weight
    that *is* used, and every pixel of the frame comes out black.
  - mpv 0.37's defaults hit this: `scale` and `cscale` are `lanczos` (6 taps;
    `cscale` runs for every 4:2:0 video), and `dscale` is `hermite`, whose tap
    count grows with the downscale ratio when `correct-downscaling` is on.
  - Whether a run is affected depends on leftover heap contents, so it's random
    per process and stays fixed for that process. With Mesa llvmpipe under Xvfb,
    most runs were black. Real GPUs may filter differently, but nothing guarantees it.
  - Radius-2 kernels need exactly 4 taps (one full texel, no padding), and
    `correct-downscaling=no` stops `dscale` from widening them. The options are
    best-effort, so a test runs them through a real libmpv to catch typos.

## Consequences

- Video works the same with every libmpv version and driver, with no dependence
  on uninitialised memory.
- Quality: `catmull_rom` is a sharp bicubic, a little softer than `lanczos` with
  less ringing. Downscaling by more than about 2× aliases more without
  `correct-downscaling`. Revisit this once libmpv zeroes the padding: version-gate
  the defaults, or expose a quality setting (slice 6).
- Video renders only while its widget is on screen. Until the player knows the
  video size, and while the window is minimised, libmpv's video thread times out
  waiting for us ("mpv_render_context_render() not being called or stuck") and
  drops that frame. Playback carries on.
- `unsafe` stays in `crates/mpv` (render context, GL calls), with a `SAFETY:`
  comment on each block.
- Not covered yet: hardware-decoding interop on Windows, HDR, ICC profiles.

## Alternatives considered

- **Render straight into egui's framebuffer:** libmpv clears and draws the whole
  target from the origin, so it would paint over the UI.
- **An egui texture (`register_native_texture`) instead of a blit:** that needs
  re-registering whenever the size changes, plus an extra textured draw. The paint
  callback also guarantees the context is current.
- **Software render API (`MPV_RENDER_API_TYPE_SW`) plus texture upload:** CPU
  conversion and an upload every frame. Too slow for HD.
- **A native child window (`wid`):** platform-specific, and egui can't draw over it
  or move it into the transport bar.
- **Keep libmpv's default scalers:** black video whenever the heap happens to hold
  NaNs. The client can't zero libmpv's internal buffer.
- **`bilinear` everywhere, or `gpu-dumb-mode`:** also avoid the lookup texture,
  but at lower quality (`gpu-dumb-mode` turns off colour management and dithering
  too). `catmull_rom` is cheap enough for weak GPUs.
