//! The application shell: panels, routing and input plumbing.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, LayerId, Margin, Modifiers, Order,
    Rect,
};
use eframe::glow;
use mediaplayer_core::{PlaybackState, Player, PlayerCommand};
use mediaplayer_mpv::VideoOutput;

use crate::backend::BackendStatus;
use crate::ui::{info_bar, nav, now_playing, pages, transport};
use crate::{Route, file_dialog, hotkeys, theme};

/// Repaint interval while playing, so the clock and seek bar keep moving.
const PLAYING_REPAINT: Duration = Duration::from_millis(100);

pub struct MediaPlayerApp {
    player: Player,
    route: Route,
    /// Seek-bar value while its handle is held; the seek happens on release.
    scrub: Option<f64>,
    theme_installed: bool,
    video: Option<Arc<VideoOutput>>,
    backend: BackendStatus,
    /// Whether the backend problem still shows as a bar; Settings always shows it.
    backend_notice: bool,
    open_requested: bool,
}

impl MediaPlayerApp {
    pub fn new(player: Player) -> Self {
        Self {
            player,
            route: Route::NowPlaying,
            scrub: None,
            theme_installed: false,
            video: None,
            backend: BackendStatus::default(),
            backend_notice: false,
            open_requested: false,
        }
    }

    /// Attaches the backend's status and, with libmpv, its video output.
    pub fn with_backend(mut self, backend: BackendStatus, video: Option<Arc<VideoOutput>>) -> Self {
        self.backend_notice = backend.problem.is_some();
        self.backend = backend;
        self.video = video;
        self
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    pub fn route(&self) -> Route {
        self.route
    }

    /// Opens a local file and shows Now Playing.
    pub fn open(&mut self, path: PathBuf) {
        self.player.dispatch(PlayerCommand::Open(path));
        self.route = Route::NowPlaying;
    }

    /// Whether the file dialog was requested since the last call. The dialog opens
    /// outside [`Self::show`], so UI tests never block on a real one.
    pub fn take_open_request(&mut self) -> bool {
        std::mem::take(&mut self.open_requested)
    }

    /// Runs one frame. eframe calls this through [`eframe::App::ui`]; UI tests call it directly.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.theme_installed {
            theme::install(&ctx);
            self.theme_installed = true;
            ctx.request_repaint();
        }
        let window = ui.max_rect();

        if let Some(path) = dropped_file(&ctx) {
            self.open(path);
        }
        if !ctx.text_edit_focused()
            && ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::O))
        {
            self.open_requested = true;
        }
        for command in hotkeys::collect(&ctx) {
            self.player.dispatch(command);
        }
        self.player.tick();

        let tokens = theme::tokens(ui);
        let palette = tokens.palette;
        // Shows through the content layer's rounded corner instead of the GL clear colour.
        ui.painter()
            .rect_filled(window, CornerRadius::ZERO, palette.base);

        // Away from Now Playing, a playing video moves into the transport bar.
        let thumbnail = self
            .video
            .as_ref()
            .filter(|_| self.route != Route::NowPlaying && self.player.video().is_some());
        let transport = egui::Panel::bottom("transport")
            .exact_size(transport::HEIGHT)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(palette.base)
                    .inner_margin(Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                transport::show(ui, &self.player, &mut self.scrub, thumbnail)
            })
            .inner;

        let open_clicked = egui::Panel::left("navigation")
            .exact_size(nav::WIDTH)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(palette.base)
                    .inner_margin(Margin::symmetric(8, 0)),
            )
            .show(ui, |ui| nav::show(ui, &mut self.route))
            .inner;
        self.open_requested |= open_clicked;

        let mut dismiss_error = false;
        let mut dismiss_notice = false;
        let content = Frame::new()
            .fill(palette.layer)
            .corner_radius(CornerRadius {
                nw: tokens.radius.card,
                ..CornerRadius::ZERO
            })
            .inner_margin(Margin::same(24));
        egui::CentralPanel::default().frame(content).show(ui, |ui| {
            if let Some(error) = self.player.error() {
                dismiss_error = info_bar::error(ui, &error.to_string());
                ui.add_space(tokens.spacing.lg);
            }
            if self.backend_notice
                && let Some(problem) = &self.backend.problem
            {
                dismiss_notice = info_bar::error(ui, problem);
                ui.add_space(tokens.spacing.lg);
            }
            match self.route {
                Route::NowPlaying => now_playing::show(ui, &self.player, self.video.as_ref()),
                Route::Settings => {
                    pages::settings(ui, &self.player.engine_info(), &self.backend);
                }
                route => pages::placeholder(ui, route),
            }
        });

        let mut commands = transport.commands;
        if dismiss_error {
            commands.push(PlayerCommand::DismissError);
        }
        if dismiss_notice {
            self.backend_notice = false;
            ctx.request_repaint();
        }
        if transport.show_now_playing {
            self.route = Route::NowPlaying;
        }
        if !commands.is_empty() {
            for command in commands {
                self.player.dispatch(command);
            }
            ctx.request_repaint();
        }

        paint_drop_overlay(&ctx, window);

        match self.player.state() {
            PlaybackState::Loading => ctx.request_repaint(),
            PlaybackState::Playing => ctx.request_repaint_after(PLAYING_REPAINT),
            _ => {}
        }
    }
}

impl eframe::App for MediaPlayerApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.show(ui);
        if self.take_open_request()
            && let Some(path) = file_dialog::pick_media_file(frame)
        {
            self.open(path);
        }
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        // libmpv's renderer must be freed with the GL context current, before the engine.
        if let (Some(video), Some(gl)) = (&self.video, gl) {
            video.destroy(gl);
        }
    }
}

/// The first file dropped on the window this frame; multi-file drops wait for the play queue.
fn dropped_file(ctx: &egui::Context) -> Option<PathBuf> {
    ctx.input(|input| {
        input
            .raw
            .dropped_files
            .first()
            .map(|file| file.path().to_path_buf())
    })
}

fn paint_drop_overlay(ctx: &egui::Context, window: Rect) {
    if ctx.input(|input| input.raw.hovered_files.is_empty()) {
        return;
    }
    let tokens = theme::tokens_for(ctx.theme() == egui::Theme::Dark);
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("drop-overlay")));
    painter.rect_filled(window, CornerRadius::ZERO, tokens.palette.scrim);
    painter.text(
        window.center(),
        Align2::CENTER_CENTER,
        "Drop to play",
        FontId::proportional(tokens.text.title),
        Color32::WHITE,
    );
}
