// Release builds are GUI-only on Windows: no console window behind the player.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use mediaplayer_app::{APP_NAME, MediaPlayerApp, backend, cli};
use mediaplayer_core::Player;

fn main() -> eframe::Result {
    env_logger::init();

    let files = match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Run { files }) => files,
        Ok(cli::Command::Help) => {
            println!("{}", cli::USAGE);
            return Ok(());
        }
        Ok(cli::Command::Version) => {
            println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id(APP_NAME)
            .with_inner_size([1180.0, 740.0])
            .with_min_inner_size([760.0, 520.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |creation_context| {
            // The GL context is current here, which libmpv's renderer needs.
            let backend = backend::start(creation_context);
            let mut app = MediaPlayerApp::new(Player::new(backend.engine))
                .with_backend(backend.status, backend.video);
            if let Some(file) = files.into_iter().next() {
                app.open(file);
            }
            Ok(Box::new(app))
        }),
    )
}
