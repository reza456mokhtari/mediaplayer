//! Chooses the playback backend at startup: libmpv when it loads, otherwise the
//! simulated engine plus a message saying how to fix that.

use std::sync::Arc;
use std::time::Duration;

use mediaplayer_core::{NullEngine, PlayerEngine};
use mediaplayer_mpv::{Libmpv, MpvEngine, VideoOutput, Waker};

/// libmpv reports positions many times a second; a short delay batches those wakeups.
const EVENT_REPAINT_DELAY: Duration = Duration::from_millis(30);

/// What the UI tells the user about the backend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendStatus {
    /// Why playback is simulated or limited, phrased so the user can act on it.
    pub problem: Option<String>,
}

pub struct Backend {
    pub engine: Box<dyn PlayerEngine>,
    pub video: Option<Arc<VideoOutput>>,
    pub status: BackendStatus,
}

/// Runs in eframe's app-creation closure, where the OpenGL context is current, which
/// libmpv's renderer needs.
pub fn start(cc: &eframe::CreationContext<'_>) -> Backend {
    let libmpv = match Libmpv::load() {
        Ok(libmpv) => libmpv,
        Err(err) => {
            log::warn!(
                "libmpv unavailable ({}), playback is simulated",
                err.summary()
            );
            return simulated(err.to_string());
        }
    };
    let ctx = cc.egui_ctx.clone();
    let wakeup: Waker = Arc::new(move || ctx.request_repaint_after(EVENT_REPAINT_DELAY));
    let engine = match MpvEngine::new(libmpv, wakeup) {
        Ok(engine) => engine,
        Err(err) => return simulated(format!("libmpv was found but couldn't start: {err}")),
    };

    let mut status = BackendStatus::default();
    let video = match cc.get_proc_address.clone() {
        Some(get_proc_address) => {
            let ctx = cc.egui_ctx.clone();
            match engine
                .create_video_output(get_proc_address, Arc::new(move || ctx.request_repaint()))
            {
                Ok(video) => Some(Arc::new(video)),
                Err(err) => {
                    status.problem = Some(format!(
                        "Video can't be shown, so files play as audio only: {err}"
                    ));
                    None
                }
            }
        }
        None => {
            status.problem = Some(
                "Video can't be shown without OpenGL, so files play as audio only.".to_owned(),
            );
            None
        }
    };
    Backend {
        engine: Box::new(engine),
        video,
        status,
    }
}

fn simulated(problem: String) -> Backend {
    Backend {
        engine: Box::new(NullEngine::new()),
        video: None,
        status: BackendStatus {
            problem: Some(problem),
        },
    }
}
