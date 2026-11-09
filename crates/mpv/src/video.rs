//! Draws libmpv's video into the UI's OpenGL context.
//!
//! libmpv renders into a framebuffer we own (sized to the on-screen rectangle), which
//! is then copied into egui's framebuffer from a paint callback. See
//! docs/adr/0004-video-output.md.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::mem::ManuallyDrop;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex};

use glow::HasContext as _;

use crate::Waker;
use crate::ffi::{self, Api};
use crate::handle::Handle;

/// Resolves OpenGL function pointers; eframe's `CreationContext::get_proc_address`.
pub type GetProcAddress = dyn Fn(&CStr) -> *const c_void + Send + Sync;

/// Where to draw, in framebuffer pixels with OpenGL's bottom-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintTarget {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Scissor rectangle `[x, y, width, height]` the picture is clipped to.
    pub clip: [i32; 4],
}

/// libmpv's renderer plus the framebuffer it draws into. Offers no playback control.
pub struct VideoOutput {
    state: Mutex<State>,
}

struct State {
    render: Option<RenderContext>,
    target: Option<Framebuffer>,
    warned: bool,
}

impl VideoOutput {
    pub(crate) fn new(
        handle: Arc<Handle>,
        get_proc_address: Arc<GetProcAddress>,
        frame_ready: Waker,
    ) -> Result<Self, String> {
        let render = RenderContext::new(handle, get_proc_address, frame_ready)?;
        Ok(Self {
            state: Mutex::new(State {
                render: Some(render),
                target: None,
                warned: false,
            }),
        })
    }

    /// Draws the latest frame into `target`, letterboxed by libmpv.
    ///
    /// Call it from an egui paint callback, where the OpenGL context is current.
    pub fn paint(&self, gl: &glow::Context, target: PaintTarget) {
        if target.width <= 0 || target.height <= 0 {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let State {
            render,
            target: framebuffer,
            warned,
        } = &mut *state;
        let Some(render) = render.as_ref() else {
            return;
        };
        // SAFETY: paint callbacks run with egui's GL context current. We only touch
        // objects we created, and we restore the framebuffer binding egui uses.
        unsafe {
            let previous = gl.get_parameter_framebuffer(glow::FRAMEBUFFER_BINDING);
            match Framebuffer::ensure(gl, framebuffer, target.width, target.height) {
                Ok((fb, resized)) => {
                    reset_state_for_libmpv(gl);
                    if render.has_new_frame() || resized {
                        render.render(fb);
                    }
                    gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fb.fbo));
                    gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, previous);
                    gl.enable(glow::SCISSOR_TEST);
                    let [x, y, width, height] = target.clip;
                    gl.scissor(x, y, width, height);
                    gl.blit_framebuffer(
                        0,
                        0,
                        fb.width,
                        fb.height,
                        target.x,
                        target.y,
                        target.x + target.width,
                        target.y + target.height,
                        glow::COLOR_BUFFER_BIT,
                        glow::NEAREST,
                    );
                }
                Err(err) if !*warned => {
                    *warned = true;
                    log::error!("video output disabled: {err}");
                }
                Err(_) => {}
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, previous);
        }
    }

    /// Frees libmpv's renderer and our GL objects. Call once, with the context
    /// current (eframe's `App::on_exit`), before the engine is dropped.
    pub fn destroy(&self, gl: &glow::Context) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if let Some(render) = state.render.take() {
            render.free();
        }
        if let Some(fb) = state.target.take() {
            // SAFETY: GL context current (caller contract); we created these objects.
            unsafe {
                gl.delete_framebuffer(fb.fbo);
                gl.delete_texture(fb.texture);
            }
        }
    }
}

/// An RGBA8 texture with a framebuffer object around it.
struct Framebuffer {
    fbo: glow::NativeFramebuffer,
    texture: glow::NativeTexture,
    width: i32,
    height: i32,
}

impl Framebuffer {
    /// Makes `slot` a `width`×`height` render target. Returns it and whether its
    /// storage was (re)allocated, which means its contents must be redrawn.
    ///
    /// # Safety
    /// The GL context must be current.
    unsafe fn ensure<'a>(
        gl: &glow::Context,
        slot: &'a mut Option<Self>,
        width: i32,
        height: i32,
    ) -> Result<(&'a Self, bool), String> {
        let resized = !slot
            .as_ref()
            .is_some_and(|fb| fb.width == width && fb.height == height);
        if resized {
            // SAFETY: GL context current (caller contract).
            unsafe { Self::allocate(gl, slot, width, height)? };
        }
        let fb = slot.as_ref().ok_or_else(|| "no framebuffer".to_owned())?;
        Ok((fb, resized))
    }

    /// # Safety
    /// The GL context must be current.
    unsafe fn allocate(
        gl: &glow::Context,
        slot: &mut Option<Self>,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        // SAFETY: GL context current (caller contract); we only touch our objects.
        unsafe {
            if slot.is_none() {
                let texture = gl.create_texture()?;
                let fbo = gl
                    .create_framebuffer()
                    .inspect_err(|_| gl.delete_texture(texture))?;
                *slot = Some(Self {
                    fbo,
                    texture,
                    width: 0,
                    height: 0,
                });
            }
            let Some(fb) = slot.as_mut() else {
                return Err("no framebuffer".to_owned());
            };
            gl.bind_texture(glow::TEXTURE_2D, Some(fb.texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            for (parameter, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, value as i32);
            }
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fb.fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(fb.texture),
                0,
            );
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(format!("framebuffer incomplete (status 0x{status:x})"));
            }
            fb.width = width;
            fb.height = height;
            Ok(())
        }
    }
}

/// `mpv_render_context`, kept alive with everything its callbacks point to.
struct RenderContext {
    api: &'static Api,
    ptr: NonNull<ffi::MpvRenderContext>,
    // ManuallyDrop: if the context can't be freed (no GL context at drop time) these
    // are leaked with it, because libmpv may still use them. The core must also
    // outlive the render context.
    handle: ManuallyDrop<Arc<Handle>>,
    get_proc_address: ManuallyDrop<Box<Arc<GetProcAddress>>>,
    frame_ready: ManuallyDrop<Box<Waker>>,
    freed: bool,
}

// SAFETY: libmpv's render API may be used from any thread as long as calls don't
// overlap; VideoOutput's mutex serialises them.
unsafe impl Send for RenderContext {}

impl RenderContext {
    fn new(
        handle: Arc<Handle>,
        get_proc_address: Arc<GetProcAddress>,
        frame_ready: Waker,
    ) -> Result<Self, String> {
        let api = handle.api();
        let get_proc_address = Box::new(get_proc_address);
        let mut init = ffi::OpenGlInitParams {
            get_proc_address: Some(load_gl_function),
            get_proc_address_ctx: ptr::from_ref(&*get_proc_address).cast_mut().cast(),
        };
        let mut params = [
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_API_TYPE,
                data: ffi::RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&raw mut init).cast(),
            },
            ffi::RenderParam::END,
        ];
        let mut raw = ptr::null_mut();
        // SAFETY: the GL context is current (create_video_output's contract); the
        // parameter list is END-terminated and valid for the call; the loader box
        // outlives the render context.
        let status =
            unsafe { (api.render_context_create)(&raw mut raw, handle.raw(), params.as_mut_ptr()) };
        if status < 0 {
            return Err(format!(
                "libmpv can't render with OpenGL: {}",
                handle.error_string(status)
            ));
        }
        let ptr = NonNull::new(raw).ok_or("libmpv returned no render context")?;
        let frame_ready = Box::new(frame_ready);
        // SAFETY: valid context; the waker box lives until the context is freed (or
        // forever, if it is leaked).
        unsafe {
            (api.render_context_set_update_callback)(
                ptr.as_ptr(),
                Some(on_frame_ready),
                ptr::from_ref(&*frame_ready).cast_mut().cast(),
            );
        }
        Ok(Self {
            api,
            ptr,
            handle: ManuallyDrop::new(handle),
            get_proc_address: ManuallyDrop::new(get_proc_address),
            frame_ready: ManuallyDrop::new(frame_ready),
            freed: false,
        })
    }

    /// Whether libmpv has a new frame for us.
    fn has_new_frame(&self) -> bool {
        // SAFETY: valid context; calls are serialised by VideoOutput's mutex.
        let flags = unsafe { (self.api.render_context_update)(self.ptr.as_ptr()) };
        flags & ffi::RENDER_UPDATE_FRAME != 0
    }

    /// Renders the current frame into `target`.
    ///
    /// # Safety
    /// The GL context must be current and `target` must be a complete framebuffer.
    unsafe fn render(&self, target: &Framebuffer) {
        let mut fbo = ffi::OpenGlFbo {
            fbo: target.fbo.0.get() as c_int,
            w: target.width,
            h: target.height,
            internal_format: glow::RGBA8 as c_int,
        };
        // The framebuffer is later copied to the screen, whose origin is bottom-left.
        let mut flip_y: c_int = 1;
        // Never stall the UI thread waiting for a frame's display time.
        let mut block_for_target_time: c_int = 0;
        let mut params = [
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_OPENGL_FBO,
                data: (&raw mut fbo).cast(),
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_FLIP_Y,
                data: (&raw mut flip_y).cast(),
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: (&raw mut block_for_target_time).cast(),
            },
            ffi::RenderParam::END,
        ];
        // SAFETY: GL context current (caller contract); parameters valid for the call.
        let status =
            unsafe { (self.api.render_context_render)(self.ptr.as_ptr(), params.as_mut_ptr()) };
        if status < 0 {
            log::warn!("libmpv render failed ({status})");
        }
    }

    /// Frees the render context. The GL context must be current.
    fn free(mut self) {
        // SAFETY: valid context, freed exactly once, GL current (destroy's contract).
        // Afterwards libmpv makes no more callbacks, so their data can go.
        unsafe { (self.api.render_context_free)(self.ptr.as_ptr()) };
        self.freed = true;
        // SAFETY: each field is dropped exactly once here and never used again.
        unsafe {
            ManuallyDrop::drop(&mut self.frame_ready);
            ManuallyDrop::drop(&mut self.get_proc_address);
            ManuallyDrop::drop(&mut self.handle);
        }
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        if !self.freed {
            log::warn!("video output dropped without destroy(); leaking libmpv's renderer");
        }
    }
}

/// libmpv expects "reasonably default" OpenGL state (render_gl.h), but egui paints
/// with its own program, vertex array, buffers and texture bound, plus scissor and
/// blending on. Restores what libmpv's renderer and its queued work depend on.
///
/// # Safety
/// The GL context must be current.
unsafe fn reset_state_for_libmpv(gl: &glow::Context) {
    // SAFETY: GL context current (caller contract); only binding/state calls.
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.use_program(None);
        gl.bind_vertex_array(None);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.disable(glow::SCISSOR_TEST);
        gl.disable(glow::BLEND);
    }
}

unsafe extern "C" fn load_gl_function(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    if ctx.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `ctx` is the loader boxed in RenderContext::new, alive as long as the
    // render context; `name` is a C string from libmpv.
    let (loader, name) = unsafe { (&*ctx.cast::<Arc<GetProcAddress>>(), CStr::from_ptr(name)) };
    loader(name).cast_mut()
}

unsafe extern "C" fn on_frame_ready(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: `ctx` is the waker boxed in RenderContext::new, alive as long as the
    // render context.
    let waker = unsafe { &*ctx.cast::<Waker>() };
    waker();
}
