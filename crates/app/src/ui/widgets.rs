//! Token-styled widgets shared by several screens.

use eframe::egui::{
    Color32, CornerRadius, FontId, Response, RichText, Role, Sense, Ui, Vec2, WidgetInfo, vec2,
};

use super::icons::{self, Icon};
use crate::theme;

/// Borderless square button showing `icon`; `label` is its accessible name.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, size: f32) -> Response {
    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), sense);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    if ui.is_rect_visible(rect) {
        let fill = if !enabled {
            Color32::TRANSPARENT
        } else if response.is_pointer_button_down_on() {
            palette.control_pressed
        } else if response.hovered() {
            palette.control_hover
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(tokens.radius.control), fill);
        let color = if enabled {
            palette.text
        } else {
            palette.text_disabled
        };
        icons::paint(ui.painter(), rect.shrink(size * 0.27), icon, color);
    }
    response
}

/// The round accent button that plays or pauses.
pub fn play_pause_button(ui: &mut Ui, showing_pause: bool, enabled: bool, size: f32) -> Response {
    let palette = theme::tokens(ui).palette;
    let label = if showing_pause { "Pause" } else { "Play" };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), sense);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    if ui.is_rect_visible(rect) {
        let (fill, foreground) = if !enabled {
            (palette.control, palette.text_disabled)
        } else if response.hovered() {
            (palette.accent_hover, palette.on_accent)
        } else {
            (palette.accent, palette.on_accent)
        };
        ui.painter().circle_filled(rect.center(), size / 2.0, fill);
        let icon = if showing_pause {
            Icon::Pause
        } else {
            Icon::Play
        };
        icons::paint(ui.painter(), rect.shrink(size * 0.3), icon, foreground);
    }
    response
}

/// Square artwork placeholder with a painted note; real cover art comes with the library.
pub fn artwork(ui: &mut Ui, size: f32, sense: Sense) -> Response {
    let tokens = theme::tokens(ui);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), sense);
    if ui.is_rect_visible(rect) {
        let radius = if size >= 96.0 {
            tokens.radius.card
        } else {
            tokens.radius.control
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(radius), tokens.palette.card);
        icons::paint(
            ui.painter(),
            rect.shrink(size * 0.3),
            Icon::Note,
            tokens.palette.text_secondary,
        );
    }
    response
}

/// Small rounded label for statuses and roadmap hints, sized to its text.
pub fn chip(ui: &mut Ui, text: &str) {
    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    let padding = vec2(10.0, 4.0);
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::proportional(tokens.text.caption),
        palette.text_secondary,
    );
    let (rect, response) = ui.allocate_exact_size(galley.size() + 2.0 * padding, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(12), palette.card);
        ui.painter()
            .galley(rect.min + padding, galley, palette.text_secondary);
    }
}

pub fn page_title(ui: &mut Ui, text: &str) {
    let tokens = theme::tokens(ui);
    ui.label(
        RichText::new(text)
            .size(tokens.text.title)
            .color(tokens.palette.text),
    );
}
