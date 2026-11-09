//! Shows libmpv's frames inside egui through an OpenGL paint callback.

use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, Rect, Ui};
use eframe::egui_glow;
use mediaplayer_mpv::{PaintTarget, VideoOutput};

/// Paints the current frame into `rect`; libmpv letterboxes it.
pub fn show(ui: &Ui, rect: Rect, video: &Arc<VideoOutput>) {
    // Seen until the first frame arrives, and wherever no GL runs (UI tests).
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
    let video = Arc::clone(video);
    let callback = egui_glow::CallbackFn::new(move |info, painter| {
        let viewport = info.viewport_in_pixels();
        let clip = info.clip_rect_in_pixels();
        video.paint(
            painter.gl(),
            PaintTarget {
                x: viewport.left_px,
                y: viewport.from_bottom_px,
                width: viewport.width_px,
                height: viewport.height_px,
                clip: [
                    clip.left_px,
                    clip.from_bottom_px,
                    clip.width_px,
                    clip.height_px,
                ],
            },
        );
    });
    ui.painter().add(egui::PaintCallback {
        rect,
        callback: Arc::new(callback),
    });
}
