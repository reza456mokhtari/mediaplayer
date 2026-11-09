//! The native "Open file" dialog.

use std::path::PathBuf;

const AUDIO: &[&str] = &["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "wma"];
const VIDEO: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "m4v", "mpg", "mpeg", "ts",
];

/// Asks for one media file. Blocks until the dialog closes; `None` if cancelled.
pub fn pick_media_file(parent: &eframe::Frame) -> Option<PathBuf> {
    let media: Vec<&str> = AUDIO.iter().chain(VIDEO).copied().collect();
    rfd::FileDialog::new()
        .set_title("Open media file")
        .add_filter("Media files", &media)
        .add_filter("Audio", AUDIO)
        .add_filter("Video", VIDEO)
        .add_filter("All files", &["*"])
        .set_parent(parent)
        .pick_file()
}
