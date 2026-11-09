//! The navigation pane.

use eframe::egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Layout, Rect, Response, RichText, Role,
    Sense, Ui, Vec2, WidgetInfo, pos2, vec2,
};

use super::icons::{self, Icon};
use crate::{APP_NAME, theme};

/// Outer width of the pane.
pub const WIDTH: f32 = 232.0;
const ITEM_HEIGHT: f32 = 36.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    NowPlaying,
    Queue,
    Music,
    Videos,
    Playlists,
    Settings,
}

impl Route {
    /// Destinations at the top of the pane; Settings is pinned to the bottom.
    pub const PRIMARY: [Self; 5] = [
        Self::NowPlaying,
        Self::Queue,
        Self::Music,
        Self::Videos,
        Self::Playlists,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NowPlaying => "Now playing",
            Self::Queue => "Play queue",
            Self::Music => "Music library",
            Self::Videos => "Video library",
            Self::Playlists => "Playlists",
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::NowPlaying => Icon::NowPlaying,
            Self::Queue => Icon::Queue,
            Self::Music => Icon::Music,
            Self::Videos => Icon::Video,
            Self::Playlists => Icon::Playlist,
            Self::Settings => Icon::Settings,
        }
    }
}

/// Draws the pane. Returns `true` when the user asked to open a file.
pub fn show(ui: &mut Ui, route: &mut Route) -> bool {
    let tokens = theme::tokens(ui);
    ui.add_space(tokens.spacing.lg);
    brand(ui);
    ui.add_space(tokens.spacing.lg);
    let open = ui
        .add_sized([ui.available_width(), 32.0], Button::new("Open file…"))
        .on_hover_text("Open an audio or video file (Ctrl+O)")
        .clicked();
    ui.add_space(tokens.spacing.md);
    for destination in Route::PRIMARY {
        if item(ui, destination, *route == destination).clicked() {
            *route = destination;
        }
    }
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.add_space(tokens.spacing.sm);
        if item(ui, Route::Settings, *route == Route::Settings).clicked() {
            *route = Route::Settings;
        }
    });
    open
}

fn brand(ui: &mut Ui) {
    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    ui.horizontal(|ui| {
        ui.add_space(tokens.spacing.md);
        let (mark, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
        ui.painter()
            .rect_filled(mark, CornerRadius::same(6), palette.accent);
        icons::paint(
            ui.painter(),
            mark.shrink(5.5),
            Icon::Play,
            palette.on_accent,
        );
        ui.label(
            RichText::new(APP_NAME)
                .size(tokens.text.body)
                .strong()
                .color(palette.text),
        );
    });
}

fn item(ui: &mut Ui, route: Route, selected: bool) -> Response {
    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ITEM_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, route.label()));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if response.is_pointer_button_down_on() {
            palette.control_pressed
        } else if response.hovered() {
            palette.control_hover
        } else if selected {
            palette.control
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(tokens.radius.control), fill);
        if selected {
            let indicator =
                Rect::from_center_size(pos2(rect.left() + 1.5, rect.center().y), vec2(3.0, 16.0));
            painter.rect_filled(indicator, CornerRadius::same(2), palette.accent);
        }
        let icon =
            Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), Vec2::splat(16.0));
        icons::paint(painter, icon, route.icon(), palette.text);
        painter.text(
            pos2(rect.left() + 44.0, rect.center().y),
            Align2::LEFT_CENTER,
            route.label(),
            FontId::proportional(tokens.text.body),
            palette.text,
        );
    }
    response
}
