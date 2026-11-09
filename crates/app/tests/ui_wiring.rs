//! Drives the real shell through egui_kittest: clicks, key presses and on-screen text.

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::{self, Key, Modifiers, Role};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use mediaplayer_app::backend::BackendStatus;
use mediaplayer_app::{MediaPlayerApp, Route};
use mediaplayer_core::{ManualClock, NullEngine, PlaybackState, Player};

const TRACK: Duration = Duration::from_secs(60);

fn existing_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

fn app(clock: &ManualClock) -> MediaPlayerApp {
    let engine = NullEngine::with_clock(Box::new(clock.clone()), TRACK);
    MediaPlayerApp::new(Player::new(Box::new(engine)))
}

fn harness_for(app: MediaPlayerApp) -> Harness<'static, MediaPlayerApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1180.0, 740.0))
        .build_ui_state(|ui, app: &mut MediaPlayerApp| app.show(ui), app);
    harness.run();
    harness
}

fn harness(clock: &ManualClock) -> Harness<'static, MediaPlayerApp> {
    harness_for(app(clock))
}

fn playing(clock: &ManualClock) -> Harness<'static, MediaPlayerApp> {
    let mut harness = harness(clock);
    harness.state_mut().open(existing_file());
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Playing);
    harness
}

fn state(harness: &Harness<'_, MediaPlayerApp>) -> PlaybackState {
    harness.state().player().state()
}

fn position(harness: &Harness<'_, MediaPlayerApp>) -> Duration {
    harness.state().player().position()
}

#[test]
fn shell_shows_navigation_and_an_empty_now_playing_page() {
    let harness = harness(&ManualClock::new());
    for label in [
        "Now playing",
        "Play queue",
        "Music library",
        "Video library",
        "Playlists",
        "Settings",
    ] {
        harness.get_by_role_and_label(Role::Button, label);
    }
    assert!(harness.query_all_by_label("Nothing playing").count() >= 1);
    harness.get_by_label("Play");
    harness.get_by_label("Seek");
    harness.get_by_label("Volume");
}

#[test]
fn play_does_nothing_without_media() {
    let mut harness = harness(&ManualClock::new());
    harness.get_by_label("Play").click();
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Idle);
}

#[test]
fn navigation_switches_pages() {
    let mut harness = harness(&ManualClock::new());
    harness
        .get_by_role_and_label(Role::Button, "Music library")
        .click();
    harness.run();
    assert_eq!(harness.state().route(), Route::Music);
    harness.get_by_label("Planned for slice 4");

    harness
        .get_by_role_and_label(Role::Button, "Settings")
        .click();
    harness.run();
    assert_eq!(harness.state().route(), Route::Settings);
    harness.get_by_label("Dark");
}

#[test]
fn transport_buttons_drive_the_player() {
    let mut harness = playing(&ManualClock::new());

    harness.get_by_label("Pause").click();
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Paused);

    harness.get_by_label("Play").click();
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Playing);

    harness.get_by_label("Mute").click();
    harness.run();
    assert!(harness.state().player().is_muted());

    harness.get_by_label("Stop").click();
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Stopped);
}

#[test]
fn keyboard_shortcuts_drive_the_player() {
    let mut harness = playing(&ManualClock::new());

    harness.key_press(Key::Space);
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Paused);

    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(position(&harness), Duration::from_secs(5));

    harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    harness.run();
    assert_eq!(position(&harness), Duration::from_secs(35));

    harness.key_press(Key::ArrowLeft);
    harness.run();
    assert_eq!(position(&harness), Duration::from_secs(30));

    harness.key_press(Key::ArrowDown);
    harness.run();
    assert!((harness.state().player().volume() - 0.75).abs() < 1e-6);

    harness.key_press(Key::M);
    harness.run();
    assert!(harness.state().player().is_muted());

    harness.key_press(Key::S);
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Stopped);
}

#[test]
fn elapsed_time_follows_playback() {
    let clock = ManualClock::new();
    let mut harness = playing(&clock);
    clock.advance(Duration::from_secs(12));
    harness.run();
    harness.get_by_label("0:12");
    harness.get_by_label("1:00");
}

#[test]
fn a_missing_file_shows_an_error_that_can_be_dismissed() {
    let mut harness = harness(&ManualClock::new());
    let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("does-not-exist.mp4");
    harness.state_mut().open(missing);
    harness.run();
    assert_eq!(state(&harness), PlaybackState::Idle);
    harness.get_by_label_contains("File not found");

    harness.get_by_label("Dismiss").click();
    harness.run();
    assert!(harness.query_by_label_contains("File not found").is_none());
    assert!(harness.state().player().error().is_none());
}

#[test]
fn the_open_file_button_requests_the_file_dialog() {
    let mut harness = harness(&ManualClock::new());
    assert!(!harness.state_mut().take_open_request());
    harness
        .get_by_role_and_label(Role::Button, "Open file…")
        .click();
    harness.run();
    assert!(harness.state_mut().take_open_request());
    assert!(
        !harness.state_mut().take_open_request(),
        "a request is taken once"
    );
}

#[test]
fn ctrl_o_requests_the_file_dialog() {
    let mut harness = harness(&ManualClock::new());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::O);
    harness.run();
    assert!(harness.state_mut().take_open_request());
}

#[test]
fn a_backend_problem_shows_until_dismissed_and_stays_in_settings() {
    const PROBLEM: &str = "libmpv-2.dll was not found next to mediaplayer.exe";
    let status = BackendStatus {
        problem: Some(PROBLEM.to_owned()),
    };
    let mut harness = harness_for(app(&ManualClock::new()).with_backend(status, None));
    harness.get_by_label_contains(PROBLEM);

    harness.get_by_label("Dismiss").click();
    harness.run();
    assert!(harness.query_by_label_contains(PROBLEM).is_none());

    harness
        .get_by_role_and_label(Role::Button, "Settings")
        .click();
    harness.run();
    harness.get_by_label_contains(PROBLEM);
}

#[test]
fn simulated_playback_points_to_settings() {
    let harness = playing(&ManualClock::new());
    harness.get_by_label_contains("Simulated playback: libmpv isn't loaded");
}
