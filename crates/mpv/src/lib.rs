//! libmpv playback engine for mediaplayer.
//!
//! libmpv is loaded at runtime ([`Libmpv::load`]), so building never needs it and a
//! missing library is a recoverable error. [`MpvEngine`] implements
//! [`mediaplayer_core::PlayerEngine`]; [`VideoOutput`] draws frames into the UI's
//! OpenGL context and offers no playback control.
//!
//! This is the only crate allowed `unsafe`. Modules that need it opt in one by one,
//! and clippy requires a `SAFETY:` comment on every unsafe block.
#![warn(clippy::undocumented_unsafe_blocks)]

use std::sync::Arc;

mod engine;
mod events;
mod ffi;
#[allow(unsafe_code)]
mod handle;
#[allow(unsafe_code)]
mod library;
#[allow(unsafe_code)]
mod video;

pub use engine::{MpvEngine, MpvError};
pub use library::{Libmpv, LoadError};
pub use video::{GetProcAddress, PaintTarget, VideoOutput};

/// Called from libmpv's threads when there is something to process. Must be cheap
/// and must not call back into libmpv.
pub type Waker = Arc<dyn Fn() + Send + Sync>;
