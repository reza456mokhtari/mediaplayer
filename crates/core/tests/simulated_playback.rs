//! The player and the simulated backend working together, as the app uses them.

use std::path::Path;
use std::time::Duration;

use mediaplayer_core::{ManualClock, NullEngine, PlaybackState, Player, PlayerCommand};

#[test]
fn simulated_playback_runs_to_the_end_and_restarts() {
    let clock = ManualClock::new();
    let engine = NullEngine::with_clock(Box::new(clock.clone()), Duration::from_secs(30));
    let mut player = Player::new(Box::new(engine));

    player.dispatch(PlayerCommand::Open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    ));
    player.tick();
    assert_eq!(player.state(), PlaybackState::Playing);
    assert_eq!(player.duration(), Some(Duration::from_secs(30)));

    clock.advance(Duration::from_secs(10));
    player.tick();
    assert_eq!(player.position(), Duration::from_secs(10));

    player.dispatch(PlayerCommand::TogglePlayPause);
    clock.advance(Duration::from_secs(10));
    player.tick();
    assert_eq!(player.state(), PlaybackState::Paused);
    assert_eq!(player.position(), Duration::from_secs(10));

    player.dispatch(PlayerCommand::SeekForward(Duration::from_secs(5)));
    player.dispatch(PlayerCommand::TogglePlayPause);
    clock.advance(Duration::from_secs(60));
    player.tick();
    assert_eq!(player.state(), PlaybackState::Ended);
    assert_eq!(player.position(), Duration::from_secs(30));

    player.dispatch(PlayerCommand::Play);
    player.tick();
    assert_eq!(player.state(), PlaybackState::Playing);
    assert_eq!(player.position(), Duration::ZERO);
}
