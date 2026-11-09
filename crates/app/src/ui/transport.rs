//! The transport bar: a seek row above track info, playback buttons and volume.

use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    Align, Label, Layout, Rect, RichText, Role, Sense, Slider, Ui, UiBuilder, Vec2, WidgetInfo,
    pos2, vec2,
};
use mediaplayer_core::{PlaybackState, Player, PlayerCommand, format_timestamp};
use mediaplayer_mpv::VideoOutput;

use super::icons::Icon;
use super::{video, widgets};
use crate::theme;

/// Outer height of the bar, including its frame margin.
pub const HEIGHT: f32 = 116.0;

const ARTWORK: f32 = 48.0;
const BUTTON: f32 = 36.0;
const PLAY_BUTTON: f32 = 44.0;
const TIME_WIDTH: f32 = 52.0;
const VOLUME_WIDTH: f32 = 112.0;

/// What the user asked for this frame.
#[derive(Debug, Default)]
pub struct TransportOutput {
    pub commands: Vec<PlayerCommand>,
    pub show_now_playing: bool,
}

/// Draws the bar. `scrub` holds the seek position while the handle is held down;
/// `thumbnail` replaces the artwork with the playing video.
pub fn show(
    ui: &mut Ui,
    player: &Player,
    scrub: &mut Option<f64>,
    thumbnail: Option<&Arc<VideoOutput>>,
) -> TransportOutput {
    let mut output = TransportOutput::default();
    seek_row(ui, player, scrub, &mut output.commands);
    controls_row(ui, player, thumbnail, &mut output);
    output
}

fn seek_row(
    ui: &mut Ui,
    player: &Player,
    scrub: &mut Option<f64>,
    commands: &mut Vec<PlayerCommand>,
) {
    let length = player.duration().map_or(0.0, |d| d.as_secs_f64());
    let max = if length > 0.0 { length } else { 1.0 };
    let mut value = scrub
        .unwrap_or_else(|| player.position().as_secs_f64())
        .clamp(0.0, max);
    let total = player
        .duration()
        .map_or_else(|| "--:--".to_owned(), format_timestamp);

    ui.horizontal(|ui| {
        let elapsed = format_timestamp(Duration::from_secs_f64(value));
        time_label(ui, &elapsed, Layout::right_to_left(Align::Center));
        let spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().slider_width = (ui.available_width() - TIME_WIDTH - spacing).max(48.0);
        let slider = Slider::new(&mut value, 0.0..=max)
            .show_value(false)
            .trailing_fill(true);
        let response = ui
            .add_enabled(player.is_seekable(), slider)
            .accessible_name("Seek")
            .on_hover_text("Seek: Left/Right 5 s, Shift+Left/Right 30 s");
        // Seek once on release rather than on every drag step.
        if response.is_pointer_button_down_on() {
            *scrub = Some(value);
        } else if scrub.take().is_some() || response.changed() {
            commands.push(PlayerCommand::SeekTo(Duration::from_secs_f64(value)));
        }
        time_label(ui, &total, Layout::left_to_right(Align::Center));
    });
}

fn time_label(ui: &mut Ui, text: &str, layout: Layout) {
    let tokens = theme::tokens(ui);
    let height = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(vec2(TIME_WIDTH, height), layout, |ui| {
        ui.label(
            RichText::new(text)
                .size(tokens.text.caption)
                .color(tokens.palette.text_secondary),
        );
    });
}

fn controls_row(
    ui: &mut Ui,
    player: &Player,
    thumbnail: Option<&Arc<VideoOutput>>,
    output: &mut TransportOutput,
) {
    let tokens = theme::tokens(ui);
    let gap = tokens.spacing.sm;
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::hover());
    let cluster_width = 4.0 * BUTTON + PLAY_BUTTON + 4.0 * gap;
    let cluster = Rect::from_center_size(row.center(), vec2(cluster_width, row.height()));
    let left = Rect::from_min_max(
        row.min,
        pos2(cluster.left() - tokens.spacing.lg, row.bottom()),
    );
    let right = Rect::from_min_max(
        pos2(cluster.right() + tokens.spacing.lg, row.top()),
        row.max,
    );

    let zone = |rect: Rect, layout: Layout| UiBuilder::new().max_rect(rect).layout(layout);
    ui.scope_builder(zone(left, Layout::left_to_right(Align::Center)), |ui| {
        track_info(ui, player, thumbnail, output);
    });
    ui.scope_builder(zone(cluster, Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = gap;
        playback_buttons(ui, player, &mut output.commands);
    });
    ui.scope_builder(zone(right, Layout::right_to_left(Align::Center)), |ui| {
        volume_controls(ui, player, &mut output.commands);
    });
}

fn track_info(
    ui: &mut Ui,
    player: &Player,
    thumbnail: Option<&Arc<VideoOutput>>,
    output: &mut TransportOutput,
) {
    let tokens = theme::tokens(ui);
    let artwork = match thumbnail {
        // Drawing the video here also keeps libmpv's frames flowing while Now
        // Playing is hidden; its renderer stalls if nobody draws.
        Some(video) => {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(ARTWORK), Sense::click());
            video::show(ui, rect, video);
            response
        }
        None => widgets::artwork(ui, ARTWORK, Sense::click()),
    };
    artwork.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Show Now Playing"));
    if artwork.on_hover_text("Show Now Playing").clicked() {
        output.show_now_playing = true;
    }
    let (title, subtitle) = match player.media() {
        Some(item) => (item.title().to_owned(), item.folder().unwrap_or_default()),
        None => (
            "Nothing playing".to_owned(),
            "Drop a file to start".to_owned(),
        ),
    };
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add(
            Label::new(
                RichText::new(title)
                    .size(tokens.text.body)
                    .strong()
                    .color(tokens.palette.text),
            )
            .truncate(),
        );
        ui.add(
            Label::new(
                RichText::new(subtitle)
                    .size(tokens.text.caption)
                    .color(tokens.palette.text_secondary),
            )
            .truncate(),
        );
    });
}

fn playback_buttons(ui: &mut Ui, player: &Player, commands: &mut Vec<PlayerCommand>) {
    let state = player.state();
    let can_stop = matches!(
        state,
        PlaybackState::Loading
            | PlaybackState::Playing
            | PlaybackState::Paused
            | PlaybackState::Ended
    );
    if widgets::icon_button(ui, Icon::Stop, "Stop", can_stop, BUTTON)
        .on_hover_text("Stop (S)")
        .clicked()
    {
        commands.push(PlayerCommand::Stop);
    }
    let _ = widgets::icon_button(ui, Icon::Previous, "Previous", false, BUTTON)
        .on_hover_text("Previous: arrives with the play queue (slice 3)");
    let pause = matches!(state, PlaybackState::Playing | PlaybackState::Loading);
    let hint = if pause {
        "Pause (Space)"
    } else {
        "Play (Space)"
    };
    if widgets::play_pause_button(ui, pause, player.media().is_some(), PLAY_BUTTON)
        .on_hover_text(hint)
        .clicked()
    {
        commands.push(PlayerCommand::TogglePlayPause);
    }
    let _ = widgets::icon_button(ui, Icon::Next, "Next", false, BUTTON)
        .on_hover_text("Next: arrives with the play queue (slice 3)");
    let _ = widgets::icon_button(ui, Icon::Repeat, "Repeat", false, BUTTON)
        .on_hover_text("Repeat: arrives with the play queue (slice 3)");
}

fn volume_controls(ui: &mut Ui, player: &Player, commands: &mut Vec<PlayerCommand>) {
    let _ = widgets::icon_button(ui, Icon::Fullscreen, "Fullscreen", false, BUTTON)
        .on_hover_text("Fullscreen video: arrives in slice 5");

    let mut percent = (player.volume() * 100.0).round();
    ui.spacing_mut().slider_width = VOLUME_WIDTH;
    let slider = Slider::new(&mut percent, 0.0..=100.0)
        .step_by(1.0)
        .show_value(false)
        .trailing_fill(true);
    let response = ui.add(slider).accessible_name("Volume");
    let response = response.on_hover_text(format!("Volume {percent:.0}% (Up/Down)"));
    if response.changed() {
        commands.push(PlayerCommand::SetVolume(percent / 100.0));
    }

    let silent = player.is_muted() || player.volume() <= 0.0;
    let icon = if silent {
        Icon::VolumeMuted
    } else if player.volume() < 0.5 {
        Icon::VolumeLow
    } else {
        Icon::VolumeHigh
    };
    let label = if player.is_muted() { "Unmute" } else { "Mute" };
    if widgets::icon_button(ui, icon, label, true, BUTTON)
        .on_hover_text(format!("{label} (M)"))
        .clicked()
    {
        commands.push(PlayerCommand::ToggleMute);
    }
}
