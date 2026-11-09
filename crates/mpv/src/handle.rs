//! Safe wrapper around one `mpv_handle`.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::{Mutex, PoisonError};

use crate::Waker;
use crate::events::{PropertyValue, RawEvent};
use crate::ffi::{self, Api};

/// A negative libmpv status code (`mpv_error`).
pub(crate) type Status = c_int;

/// An initialised-or-not mpv core. Destroyed (`mpv_terminate_destroy`) on drop.
pub(crate) struct Handle {
    api: &'static Api,
    ptr: NonNull<ffi::MpvHandle>,
    wakeup: Option<NonNull<Waker>>,
    /// mpv_wait_event must not run concurrently; this serialises it.
    events: Mutex<()>,
}

// SAFETY: the libmpv client API is thread-safe, except mpv_wait_event, which
// `next_event` serialises. The wakeup box is only freed in Drop.
unsafe impl Send for Handle {}
// SAFETY: see `Send`; all `&self` methods are safe to call concurrently.
unsafe impl Sync for Handle {}

impl Handle {
    pub(crate) fn create(api: &'static Api) -> Result<Self, String> {
        // SAFETY: mpv_create has no preconditions and returns null on failure.
        let ptr = unsafe { (api.create)() };
        NonNull::new(ptr)
            .map(|ptr| Self {
                api,
                ptr,
                wakeup: None,
                events: Mutex::new(()),
            })
            .ok_or_else(|| "mpv_create failed".to_owned())
    }

    pub(crate) fn api(&self) -> &'static Api {
        self.api
    }

    pub(crate) fn raw(&self) -> *mut ffi::MpvHandle {
        self.ptr.as_ptr()
    }

    pub(crate) fn set_option(&self, name: &CStr, value: &CStr) -> Result<(), Status> {
        // SAFETY: valid handle; both strings are NUL-terminated and outlive the call.
        check(unsafe { (self.api.set_option_string)(self.raw(), name.as_ptr(), value.as_ptr()) })
    }

    pub(crate) fn initialize(&self) -> Result<(), Status> {
        // SAFETY: valid handle; called once, before any playback.
        check(unsafe { (self.api.initialize)(self.raw()) })
    }

    pub(crate) fn command(&self, args: &[&CStr]) -> Result<(), Status> {
        let mut argv = argv(args);
        // SAFETY: argv is a NULL-terminated array of C strings that outlive the call.
        check(unsafe { (self.api.command)(self.raw(), argv.as_mut_ptr()) })
    }

    /// Runs a command and returns the integer `key` from its result map.
    pub(crate) fn command_returning(
        &self,
        args: &[&CStr],
        key: &str,
    ) -> Result<Option<i64>, Status> {
        let mut argv = argv(args);
        let mut result = ffi::Node::empty();
        // SAFETY: as in `command`; on success libmpv fills `result`, freed below.
        check(unsafe { (self.api.command_ret)(self.raw(), argv.as_mut_ptr(), &raw mut result) })?;
        // SAFETY: libmpv just produced this node tree, and it lives until freed below.
        let value = unsafe { map_int(&result, key) };
        // SAFETY: frees what libmpv allocated in `result`, exactly once.
        unsafe { (self.api.free_node_contents)(&raw mut result) };
        Ok(value)
    }

    pub(crate) fn set_flag(&self, name: &CStr, value: bool) -> Result<(), Status> {
        let mut flag = c_int::from(value);
        // SAFETY: MPV_FORMAT_FLAG reads one int through the pointer during the call.
        check(unsafe {
            (self.api.set_property)(
                self.raw(),
                name.as_ptr(),
                ffi::FORMAT_FLAG,
                (&raw mut flag).cast(),
            )
        })
    }

    pub(crate) fn set_double(&self, name: &CStr, value: f64) -> Result<(), Status> {
        let mut value = value;
        // SAFETY: MPV_FORMAT_DOUBLE reads one double through the pointer during the call.
        check(unsafe {
            (self.api.set_property)(
                self.raw(),
                name.as_ptr(),
                ffi::FORMAT_DOUBLE,
                (&raw mut value).cast(),
            )
        })
    }

    pub(crate) fn get_double(&self, name: &CStr) -> Option<f64> {
        let mut value = 0.0_f64;
        // SAFETY: MPV_FORMAT_DOUBLE writes one double through the pointer.
        let status = unsafe {
            (self.api.get_property)(
                self.raw(),
                name.as_ptr(),
                ffi::FORMAT_DOUBLE,
                (&raw mut value).cast(),
            )
        };
        (status >= 0).then_some(value)
    }

    pub(crate) fn get_string(&self, name: &CStr) -> Option<String> {
        // SAFETY: valid handle and name; the result is null or a string we must mpv_free.
        let raw = unsafe { (self.api.get_property_string)(self.raw(), name.as_ptr()) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: a non-null result is a valid C string until freed.
        let value = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: frees the string libmpv allocated, exactly once.
        unsafe { (self.api.free)(raw.cast()) };
        Some(value)
    }

    pub(crate) fn observe(&self, id: u64, name: &CStr, format: c_int) -> Result<(), Status> {
        // SAFETY: valid handle; libmpv copies the name.
        check(unsafe { (self.api.observe_property)(self.raw(), id, name.as_ptr(), format) })
    }

    pub(crate) fn request_log_messages(&self, level: &CStr) -> Result<(), Status> {
        // SAFETY: valid handle; libmpv copies the level string.
        check(unsafe { (self.api.request_log_messages)(self.raw(), level.as_ptr()) })
    }

    pub(crate) fn error_string(&self, status: Status) -> String {
        // SAFETY: mpv_error_string accepts any code and returns a static string.
        let raw = unsafe { (self.api.error_string)(status) };
        if raw.is_null() {
            return format!("libmpv error {status}");
        }
        // SAFETY: non-null results are static C strings.
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    }

    /// The next pending event, without blocking. `None` once the queue is empty.
    pub(crate) fn next_event(&self) -> Option<RawEvent> {
        let _serialised = self.events.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: valid handle; calls are serialised; a zero timeout never blocks.
        // libmpv never returns null here.
        let event = unsafe { &*(self.api.wait_event)(self.raw(), 0.0) };
        // SAFETY: the event and its data stay valid until the next mpv_wait_event,
        // which can't happen while `_serialised` is held.
        unsafe { decode(event) }
    }

    /// Registers `waker` to run whenever events become available. Call once.
    pub(crate) fn set_wakeup(&mut self, waker: Waker) {
        if self.wakeup.is_some() {
            return;
        }
        let data = NonNull::from(Box::leak(Box::new(waker)));
        // SAFETY: `data` stays valid until after mpv_terminate_destroy in Drop, after
        // which libmpv makes no more callbacks.
        unsafe { (self.api.set_wakeup_callback)(self.raw(), Some(wake), data.as_ptr().cast()) };
        self.wakeup = Some(data);
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: valid handle, destroyed exactly once. A render context holds an
        // Arc<Handle>, so it has been freed (or deliberately leaked) by now.
        unsafe { (self.api.terminate_destroy)(self.raw()) };
        if let Some(data) = self.wakeup.take() {
            // SAFETY: created by Box::leak in set_wakeup; libmpv no longer calls it.
            drop(unsafe { Box::from_raw(data.as_ptr()) });
        }
    }
}

unsafe extern "C" fn wake(data: *mut c_void) {
    // SAFETY: `data` is the waker registered in `set_wakeup`, alive until the handle
    // is destroyed.
    let waker = unsafe { &*data.cast::<Waker>() };
    waker();
}

fn check(status: Status) -> Result<(), Status> {
    if status < 0 { Err(status) } else { Ok(()) }
}

/// A NULL-terminated argument vector borrowing `args`.
fn argv(args: &[&CStr]) -> Vec<*const c_char> {
    args.iter()
        .map(|arg| arg.as_ptr())
        .chain([ptr::null()])
        .collect()
}

/// Copies `event` into owned Rust data.
///
/// # Safety
/// `event` must come from mpv_wait_event, with no later call to it yet.
unsafe fn decode(event: &ffi::Event) -> Option<RawEvent> {
    let data = event.data;
    let decoded = match event.event_id {
        ffi::EVENT_NONE => return None,
        ffi::EVENT_SHUTDOWN => RawEvent::Shutdown,
        ffi::EVENT_FILE_LOADED => RawEvent::FileLoaded,
        ffi::EVENT_VIDEO_RECONFIG => RawEvent::VideoReconfig,
        ffi::EVENT_START_FILE if !data.is_null() => {
            // SAFETY: START_FILE carries an mpv_event_start_file.
            let start = unsafe { &*data.cast::<ffi::EventStartFile>() };
            RawEvent::StartFile {
                entry: start.playlist_entry_id,
            }
        }
        ffi::EVENT_END_FILE if !data.is_null() => {
            // SAFETY: END_FILE carries an mpv_event_end_file.
            let end = unsafe { &*data.cast::<ffi::EventEndFile>() };
            RawEvent::EndFile {
                entry: end.playlist_entry_id,
                reason: end.reason,
                error: end.error,
            }
        }
        ffi::EVENT_PROPERTY_CHANGE if !data.is_null() => {
            // SAFETY: PROPERTY_CHANGE carries an mpv_event_property.
            let property = unsafe { &*data.cast::<ffi::EventProperty>() };
            // SAFETY: `property.data` points to a value of `property.format`.
            let value = unsafe { property_value(property) };
            RawEvent::Property {
                id: event.reply_userdata,
                value,
            }
        }
        ffi::EVENT_LOG_MESSAGE if !data.is_null() => {
            // SAFETY: LOG_MESSAGE carries an mpv_event_log_message.
            let message = unsafe { &*data.cast::<ffi::EventLogMessage>() };
            // SAFETY: its strings are valid C strings while the event is.
            unsafe {
                RawEvent::Log {
                    level: text(message.level),
                    prefix: text(message.prefix),
                    text: text(message.text),
                }
            }
        }
        _ => RawEvent::Other,
    };
    Some(decoded)
}

/// # Safety
/// `property.data` must be null or point to a value of `property.format`.
unsafe fn property_value(property: &ffi::EventProperty) -> PropertyValue {
    let data = property.data;
    if data.is_null() {
        return PropertyValue::None;
    }
    // SAFETY: the format tells us which type `data` points to.
    unsafe {
        match property.format {
            ffi::FORMAT_DOUBLE => PropertyValue::Double(*data.cast::<f64>()),
            ffi::FORMAT_INT64 => PropertyValue::Int(*data.cast::<i64>()),
            ffi::FORMAT_FLAG => PropertyValue::Flag(*data.cast::<c_int>() != 0),
            _ => PropertyValue::None,
        }
    }
}

/// # Safety
/// `raw` must be null or a valid C string.
unsafe fn text(raw: *const c_char) -> String {
    if raw.is_null() {
        return String::new();
    }
    // SAFETY: checked non-null; validity is the caller's contract.
    unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

/// The INT64 value stored under `key` in a node map.
///
/// # Safety
/// `node` must be a valid node tree produced by libmpv.
unsafe fn map_int(node: &ffi::Node, key: &str) -> Option<i64> {
    if node.format != ffi::FORMAT_NODE_MAP {
        return None;
    }
    // SAFETY: format NODE_MAP means `u.list` is the active field.
    let list = unsafe { node.u.list.as_ref()? };
    let len = usize::try_from(list.num).ok()?;
    if len == 0 || list.keys.is_null() || list.values.is_null() {
        return None;
    }
    // SAFETY: a node map has `num` keys and `num` values.
    let (keys, values) = unsafe {
        (
            slice::from_raw_parts(list.keys, len),
            slice::from_raw_parts(list.values, len),
        )
    };
    keys.iter().zip(values).find_map(|(&name, value)| {
        // SAFETY: map keys are valid C strings.
        let matches =
            !name.is_null() && unsafe { CStr::from_ptr(name) }.to_bytes() == key.as_bytes();
        // Read the union only when INT64 is the active field: a narrower field
        // (e.g. a 4-byte flag) would leave bytes uninitialised.
        if matches && value.format == ffi::FORMAT_INT64 {
            // SAFETY: format INT64 means `u.int64` is the active field.
            Some(unsafe { value.u.int64 })
        } else {
            None
        }
    })
}
