//! Design tokens, and how they map onto egui's style.
//!
//! Screens read colours, spacing and type sizes from [`Tokens`] rather than
//! hard-coding them, so both themes (and later polish) change in one place.

use eframe::egui::style::{HandleShape, WidgetVisuals};
use eframe::egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Stroke, Style, TextStyle, Theme, Ui, vec2,
};

/// Colour roles. Values are our own, picked to sit near Windows 11's neutral surfaces.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window background: navigation pane and transport bar.
    pub base: Color32,
    /// Content surface on top of `base`.
    pub layer: Color32,
    /// Cards, chips and artwork placeholders.
    pub card: Color32,
    /// Subtle fill for buttons and the selected navigation item.
    pub control: Color32,
    pub control_hover: Color32,
    pub control_pressed: Color32,
    /// Slider rails and radio wells.
    pub control_strong: Color32,
    pub stroke: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Foreground drawn on `accent`.
    pub on_accent: Color32,
    pub danger: Color32,
    pub danger_surface: Color32,
    /// Veil behind the drop-to-play overlay.
    pub scrim: Color32,
}

#[derive(Debug, Clone, Copy)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Radius {
    pub control: u8,
    pub card: u8,
}

/// Font sizes following the Windows 11 type ramp.
#[derive(Debug, Clone, Copy)]
pub struct TypeRamp {
    pub caption: f32,
    pub body: f32,
    pub subtitle: f32,
    pub title: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub palette: Palette,
    pub spacing: Spacing,
    pub radius: Radius,
    pub text: TypeRamp,
}

const SPACING: Spacing = Spacing {
    xs: 4.0,
    sm: 8.0,
    md: 12.0,
    lg: 16.0,
    xl: 24.0,
};

const RADIUS: Radius = Radius {
    control: 4,
    card: 8,
};

const TYPE_RAMP: TypeRamp = TypeRamp {
    caption: 12.0,
    body: 14.0,
    subtitle: 20.0,
    title: 28.0,
};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub static DARK: Tokens = Tokens {
    palette: Palette {
        base: rgb(0x202020),
        layer: rgb(0x282828),
        card: rgb(0x363636),
        control: rgb(0x2D2D2D),
        control_hover: rgb(0x383838),
        control_pressed: rgb(0x262626),
        control_strong: rgb(0x8A8A8A),
        stroke: rgb(0x3A3A3A),
        text: rgb(0xFFFFFF),
        text_secondary: rgb(0xC5C5C5),
        text_disabled: rgb(0x787878),
        accent: rgb(0x4CC2FF),
        accent_hover: rgb(0x47B1E8),
        on_accent: rgb(0x000000),
        danger: rgb(0xFF99A4),
        danger_surface: rgb(0x442726),
        scrim: Color32::from_rgba_premultiplied(0, 0, 0, 170),
    },
    spacing: SPACING,
    radius: RADIUS,
    text: TYPE_RAMP,
};

pub static LIGHT: Tokens = Tokens {
    palette: Palette {
        base: rgb(0xF3F3F3),
        layer: rgb(0xFAFAFA),
        card: rgb(0xE6E6E6),
        control: rgb(0xEAEAEA),
        control_hover: rgb(0xE3E3E3),
        control_pressed: rgb(0xEDEDED),
        control_strong: rgb(0x8A8A8A),
        stroke: rgb(0xE0E0E0),
        text: rgb(0x1B1B1B),
        text_secondary: rgb(0x5F5F5F),
        text_disabled: rgb(0xA0A0A0),
        accent: rgb(0x005FB8),
        accent_hover: rgb(0x196EBF),
        on_accent: rgb(0xFFFFFF),
        danger: rgb(0xC42B1C),
        danger_surface: rgb(0xFDE7E9),
        scrim: Color32::from_rgba_premultiplied(0, 0, 0, 110),
    },
    spacing: SPACING,
    radius: RADIUS,
    text: TYPE_RAMP,
};

/// Tokens for the theme `ui` is drawn with.
pub fn tokens(ui: &Ui) -> &'static Tokens {
    tokens_for(ui.visuals().dark_mode)
}

pub fn tokens_for(dark_mode: bool) -> &'static Tokens {
    if dark_mode { &DARK } else { &LIGHT }
}

/// Installs both themes; egui picks one from the system or user preference.
pub fn install(ctx: &Context) {
    ctx.style_mut_of(Theme::Dark, |style| apply(style, &DARK));
    ctx.style_mut_of(Theme::Light, |style| apply(style, &LIGHT));
}

fn apply(style: &mut Style, tokens: &Tokens) {
    let palette = &tokens.palette;
    let text = &tokens.text;
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(text.caption, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(text.body, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(text.body, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(text.title, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    style.spacing.item_spacing = vec2(tokens.spacing.sm, tokens.spacing.sm);
    style.spacing.button_padding = vec2(tokens.spacing.md, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.slider_rail_height = 4.0;
    style.interaction.selectable_labels = false;

    let corner_radius = CornerRadius::same(tokens.radius.control);
    // egui fills buttons with `weak_bg_fill`; sliders and radios use `bg_fill`.
    let widget = |bg_fill: Color32, weak_bg_fill: Color32, foreground: Color32| WidgetVisuals {
        bg_fill,
        weak_bg_fill,
        bg_stroke: Stroke::NONE,
        corner_radius,
        fg_stroke: Stroke::new(1.0, foreground),
        expansion: 0.0,
    };

    let visuals = &mut style.visuals;
    visuals.panel_fill = palette.base;
    visuals.window_fill = palette.layer;
    visuals.window_stroke = Stroke::new(1.0, palette.stroke);
    visuals.window_corner_radius = CornerRadius::same(tokens.radius.card);
    visuals.menu_corner_radius = CornerRadius::same(tokens.radius.card);
    visuals.extreme_bg_color = palette.control_pressed;
    visuals.faint_bg_color = palette.layer;
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = Stroke::new(1.0, palette.on_accent);
    visuals.hyperlink_color = palette.accent;
    visuals.error_fg_color = palette.danger;
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = HandleShape::Circle;
    visuals.widgets.noninteractive = WidgetVisuals {
        bg_stroke: Stroke::new(1.0, palette.stroke),
        ..widget(palette.layer, palette.layer, palette.text)
    };
    visuals.widgets.inactive = widget(palette.control_strong, palette.control, palette.text);
    visuals.widgets.hovered = widget(palette.control_strong, palette.control_hover, palette.text);
    visuals.widgets.active = widget(
        palette.control_strong,
        palette.control_pressed,
        palette.text,
    );
    visuals.widgets.open = widget(palette.control_strong, palette.control_hover, palette.text);
}
