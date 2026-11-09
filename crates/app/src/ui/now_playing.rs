//! The Now Playing page.

use std::sync::Arc;

use eframe::egui::{Label, RichText, Sense, Ui};
use mediaplayer_core::{PlaybackState, Player};
use mediaplayer_mpv::VideoOutput;

use super::{video, widgets};
use crate::theme;

pub fn show(ui: &mut Ui, player: &Player, video: Option<&Arc<VideoOutput>>) {
    if let Some(video) = video
        && player.video().is_some()
    {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        video::show(ui, rect, video);
        return;
    }

    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    let height = ui.available_height();
    ui.vertical_centered(|ui| {
        ui.add_space((height * 0.06).max(tokens.spacing.md));
        widgets::artwork(ui, (height * 0.45).clamp(120.0, 300.0), Sense::hover());
        ui.add_space(tokens.spacing.xl);

        let Some(item) = player.media() else {
            ui.label(
                RichText::new("Nothing playing")
                    .size(tokens.text.title)
                    .color(palette.text),
            );
            ui.add_space(tokens.spacing.sm);
            ui.label(
                RichText::new(
                    "Open a file with “Open file…” or Ctrl+O, drag one onto this window, \
                     or start mediaplayer with a file path.",
                )
                .size(tokens.text.body)
                .color(palette.text_secondary),
            );
            return;
        };

        ui.add(
            Label::new(
                RichText::new(item.title())
                    .size(tokens.text.title)
                    .color(palette.text),
            )
            .truncate(),
        );
        if let Some(folder) = item.folder() {
            ui.add(
                Label::new(
                    RichText::new(folder)
                        .size(tokens.text.caption)
                        .color(palette.text_secondary),
                )
                .truncate(),
            );
        }
        ui.add_space(tokens.spacing.md);
        widgets::chip(ui, state_label(player.state()));
        if player.engine_info().simulated {
            ui.add_space(tokens.spacing.md);
            ui.label(
                RichText::new(
                    "Simulated playback: libmpv isn't loaded, so there's no sound or picture. \
                     Settings > Playback backend says how to add it.",
                )
                .size(tokens.text.caption)
                .color(palette.text_secondary),
            );
        }
    });
}

pub fn state_label(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Idle => "Idle",
        PlaybackState::Loading => "Opening…",
        PlaybackState::Playing => "Playing",
        PlaybackState::Paused => "Paused",
        PlaybackState::Stopped => "Stopped",
        PlaybackState::Ended => "Finished",
    }
}
