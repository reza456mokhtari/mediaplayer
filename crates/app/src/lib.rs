//! egui desktop shell for mediaplayer.

pub mod app;
pub mod backend;
pub mod cli;
pub mod file_dialog;
pub mod hotkeys;
pub mod theme;
pub mod ui;

pub use app::MediaPlayerApp;
pub use ui::nav::Route;

/// Product name shown in the title bar and the navigation pane.
pub const APP_NAME: &str = "mediaplayer";
