//! Inline message bar.

use eframe::egui::{
    Align, Button, CornerRadius, FontId, Frame, Label, Layout, Margin, Rect, RichText, Sense, Ui,
    UiBuilder, Vec2, pos2, vec2,
};

use super::icons::{self, Icon};
use crate::theme;

const ICON: f32 = 20.0;
const BUTTON: Vec2 = Vec2::new(88.0, 28.0);

/// Shows `message` as a full-width error bar with a Dismiss button.
/// Returns `true` once dismissed.
pub fn error(ui: &mut Ui, message: &str) -> bool {
    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    let gap = tokens.spacing.md;
    let font = FontId::proportional(tokens.text.body);
    let mut dismissed = false;
    Frame::new()
        .fill(palette.danger_surface)
        .corner_radius(CornerRadius::same(tokens.radius.card))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            let width = ui.available_width();
            let text_width = (width - ICON - BUTTON.x - 2.0 * gap).max(80.0);
            let text_height = ui
                .painter()
                .layout(message.to_owned(), font.clone(), palette.text, text_width)
                .size()
                .y;
            let (row, _) =
                ui.allocate_exact_size(vec2(width, text_height.max(BUTTON.y)), Sense::hover());

            let icon = Rect::from_center_size(
                pos2(row.left() + ICON / 2.0, row.center().y),
                Vec2::splat(ICON),
            );
            icons::paint(ui.painter(), icon, Icon::Alert, palette.danger);

            let text = Rect::from_min_size(
                pos2(icon.right() + gap, row.top()),
                vec2(text_width, row.height()),
            );
            let text_layout = UiBuilder::new()
                .max_rect(text)
                .layout(Layout::left_to_right(Align::Center));
            ui.scope_builder(text_layout, |ui| {
                ui.add(Label::new(RichText::new(message).font(font).color(palette.text)).wrap());
            });

            let button =
                Rect::from_center_size(pos2(row.right() - BUTTON.x / 2.0, row.center().y), BUTTON);
            dismissed = ui.put(button, Button::new("Dismiss")).clicked();
        });
    dismissed
}
