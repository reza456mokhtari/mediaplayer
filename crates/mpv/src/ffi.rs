//! Hand-written libmpv C declarations (client API 2.x).
//!
//! Checked against mpv's `client.h`, `render.h` and `render_gl.h` for client API 2.2
//! (mpv 0.37) and 2.6 (shinchiro build 20261109). Every type and constant used here
//! is identical in both.

use std::ffi::{CStr, c_char, c_double, c_int, c_ulong, c_void};
use std::ptr;

/// Opaque `mpv_handle`.
pub(crate) enum MpvHandle {}
/// Opaque `mpv_render_context`.
pub(crate) enum MpvRenderContext {}

// mpv_format
pub(crate) const FORMAT_NONE: c_int = 0;
pub(crate) const FORMAT_FLAG: c_int = 3;
pub(crate) const FORMAT_INT64: c_int = 4;
pub(crate) const FORMAT_DOUBLE: c_int = 5;
pub(crate) const FORMAT_NODE_MAP: c_int = 8;

// mpv_event_id
pub(crate) const EVENT_NONE: c_int = 0;
pub(crate) const EVENT_SHUTDOWN: c_int = 1;
pub(crate) const EVENT_LOG_MESSAGE: c_int = 2;
pub(crate) const EVENT_START_FILE: c_int = 6;
pub(crate) const EVENT_END_FILE: c_int = 7;
pub(crate) const EVENT_FILE_LOADED: c_int = 8;
pub(crate) const EVENT_PROPERTY_CHANGE: c_int = 22;

// mpv_end_file_reason
pub(crate) const END_FILE_REASON_EOF: c_int = 0;
pub(crate) const END_FILE_REASON_ERROR: c_int = 4;

// mpv_error
pub(crate) const ERROR_LOADING_FAILED: c_int = -13;
pub(crate) const ERROR_AO_INIT_FAILED: c_int = -14;
pub(crate) const ERROR_VO_INIT_FAILED: c_int = -15;
pub(crate) const ERROR_NOTHING_TO_PLAY: c_int = -16;
pub(crate) const ERROR_UNKNOWN_FORMAT: c_int = -17;

// mpv_render_param_type
pub(crate) const RENDER_PARAM_INVALID: c_int = 0;
pub(crate) const RENDER_PARAM_API_TYPE: c_int = 1;
pub(crate) const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub(crate) const RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub(crate) const RENDER_PARAM_FLIP_Y: c_int = 4;
pub(crate) const RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;

pub(crate) const RENDER_API_TYPE_OPENGL: &CStr = c"opengl";
// mpv_render_update_flag
pub(crate) const RENDER_UPDATE_FRAME: u64 = 1;

/// `mpv_event`
#[repr(C)]
pub(crate) struct Event {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

/// `mpv_event_property`
#[repr(C)]
pub(crate) struct EventProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

/// `mpv_event_start_file`
#[repr(C)]
pub(crate) struct EventStartFile {
    pub playlist_entry_id: i64,
}

/// `mpv_event_end_file`
#[repr(C)]
pub(crate) struct EventEndFile {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

/// `mpv_event_log_message`
#[repr(C)]
pub(crate) struct EventLogMessage {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: c_int,
}

/// The anonymous union inside `mpv_node`.
#[repr(C)]
pub(crate) union NodeValue {
    pub string: *mut c_char,
    pub flag: c_int,
    pub int64: i64,
    pub double: c_double,
    pub list: *mut NodeList,
    pub byte_array: *mut c_void,
}

/// `mpv_node`
#[repr(C)]
pub(crate) struct Node {
    pub u: NodeValue,
    pub format: c_int,
}

impl Node {
    pub(crate) const fn empty() -> Self {
        Self {
            u: NodeValue { int64: 0 },
            format: FORMAT_NONE,
        }
    }
}

/// `mpv_node_list`
#[repr(C)]
pub(crate) struct NodeList {
    pub num: c_int,
    pub values: *mut Node,
    pub keys: *mut *mut c_char,
}

/// `mpv_render_param`
#[repr(C)]
pub(crate) struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

impl RenderParam {
    /// Terminates a parameter list.
    pub(crate) const END: Self = Self {
        kind: RENDER_PARAM_INVALID,
        data: ptr::null_mut(),
    };
}

pub(crate) type Callback = unsafe extern "C" fn(*mut c_void);
pub(crate) type GetProcAddressFn = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;

/// `mpv_opengl_init_params`
#[repr(C)]
pub(crate) struct OpenGlInitParams {
    pub get_proc_address: Option<GetProcAddressFn>,
    pub get_proc_address_ctx: *mut c_void,
}

/// `mpv_opengl_fbo`
#[repr(C)]
pub(crate) struct OpenGlFbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

/// The libmpv functions mediaplayer calls, resolved when the library loads.
pub(crate) struct Api {
    pub client_api_version: unsafe extern "C" fn() -> c_ulong,
    pub error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    pub free: unsafe extern "C" fn(*mut c_void),
    pub create: unsafe extern "C" fn() -> *mut MpvHandle,
    pub initialize: unsafe extern "C" fn(*mut MpvHandle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(*mut MpvHandle),
    pub set_option_string:
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(*mut MpvHandle, *mut *const c_char) -> c_int,
    pub command_ret: unsafe extern "C" fn(*mut MpvHandle, *mut *const c_char, *mut Node) -> c_int,
    pub free_node_contents: unsafe extern "C" fn(*mut Node),
    pub set_property:
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int,
    pub get_property:
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int,
    pub get_property_string: unsafe extern "C" fn(*mut MpvHandle, *const c_char) -> *mut c_char,
    pub observe_property: unsafe extern "C" fn(*mut MpvHandle, u64, *const c_char, c_int) -> c_int,
    pub request_log_messages: unsafe extern "C" fn(*mut MpvHandle, *const c_char) -> c_int,
    pub wait_event: unsafe extern "C" fn(*mut MpvHandle, c_double) -> *mut Event,
    pub set_wakeup_callback: unsafe extern "C" fn(*mut MpvHandle, Option<Callback>, *mut c_void),
    pub render_context_create:
        unsafe extern "C" fn(*mut *mut MpvRenderContext, *mut MpvHandle, *mut RenderParam) -> c_int,
    pub render_context_set_update_callback:
        unsafe extern "C" fn(*mut MpvRenderContext, Option<Callback>, *mut c_void),
    pub render_context_update: unsafe extern "C" fn(*mut MpvRenderContext) -> u64,
    pub render_context_render:
        unsafe extern "C" fn(*mut MpvRenderContext, *mut RenderParam) -> c_int,
    pub render_context_free: unsafe extern "C" fn(*mut MpvRenderContext),
}
