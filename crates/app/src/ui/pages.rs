//! Placeholder destinations and the Settings page.

use eframe::egui::{Label, RichText, ThemePreference, Ui};
use mediaplayer_core::EngineInfo;

use super::nav::Route;
use super::widgets;
use crate::backend::BackendStatus;
use crate::{APP_NAME, theme};

/// Roadmap page for a destination that a later slice fills in.
pub fn placeholder(ui: &mut Ui, route: Route) {
    let (summary, plan) = match route {
        Route::Queue => (
            "Add, remove and reorder what plays next, and open whole folders.",
            "Planned for slice 3",
        ),
        Route::Playlists => ("Save and load M3U playlists.", "Planned for slice 3"),
        Route::Music => (
            "Scan music folders into a local SQLite library, with durations and search.",
            "Planned for slice 4",
        ),
        Route::Videos => (
            "Browse scanned videos, then watch fullscreen with subtitle and audio track choices.",
            "Planned for slices 4 and 5",
        ),
        Route::NowPlaying | Route::Settings => return,
    };
    let tokens = theme::tokens(ui);
    widgets::page_title(ui, route.label());
    ui.add_space(tokens.spacing.sm);
    ui.label(
        RichText::new(summary)
            .size(tokens.text.body)
            .color(tokens.palette.text_secondary),
    );
    ui.add_space(tokens.spacing.md);
    widgets::chip(ui, plan);
}

pub fn settings(ui: &mut Ui, engine: &EngineInfo, backend: &BackendStatus) {
    let tokens = theme::tokens(ui);
    let palette = tokens.palette;
    widgets::page_title(ui, "Settings");
    ui.add_space(tokens.spacing.lg);

    section(ui, "Appearance");
    let current = ui.ctx().options(|options| options.theme_preference);
    let mut preference = current;
    ui.horizontal(|ui| {
        ui.radio_value(
            &mut preference,
            ThemePreference::System,
            "Use system setting",
        );
        ui.radio_value(&mut preference, ThemePreference::Light, "Light");
        ui.radio_value(&mut preference, ThemePreference::Dark, "Dark");
    });
    if preference != current {
        ui.ctx().set_theme(preference);
    }

    ui.add_space(tokens.spacing.xl);
    section(ui, "Playback backend");
    ui.label(RichText::new(&engine.name).color(palette.text));
    if let Some(problem) = &backend.problem {
        ui.add(Label::new(RichText::new(problem).color(palette.danger)).wrap());
    }
    if cfg!(windows) {
        ui.label(
            RichText::new(
                "mediaplayer loads libmpv-2.dll only from its own folder or from System32.",
            )
            .size(tokens.text.caption)
            .color(palette.text_secondary),
        );
    }

    ui.add_space(tokens.spacing.xl);
    section(ui, "About");
    ui.label(
        RichText::new(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"))).color(palette.text),
    );
    ui.label(
        RichText::new(
            "An independent open-source project. \
             Not affiliated with or endorsed by Microsoft or VideoLAN.",
        )
        .color(palette.text_secondary),
    );
}

fn section(ui: &mut Ui, title: &str) {
    let tokens = theme::tokens(ui);
    ui.label(
        RichText::new(title)
            .size(tokens.text.body)
            .strong()
            .color(tokens.palette.text),
    );
    ui.add_space(tokens.spacing.xs);
}
