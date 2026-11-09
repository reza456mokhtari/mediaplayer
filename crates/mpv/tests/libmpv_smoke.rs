//! End-to-end checks against a real libmpv, through `Player` and `MpvEngine`.
//!
//! Ignored by default because they need the library:
//! `cargo test -p mediaplayer-mpv -- --ignored`. `MPV_LIB=/path/to/libmpv` loads an
//! explicit file instead of searching the standard location. No audio device is
//! needed (`audio-fallback-to-null`), and no GPU: these files are audio only.

use std::f32::consts::TAU;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use mediaplayer_core::{PlaybackState, Player, PlayerCommand, PlayerError};
use mediaplayer_mpv::{Libmpv, MpvEngine};

const TIMEOUT: Duration = Duration::from_secs(15);

/// libmpv's audio-output probing isn't safe across concurrent instances in one
/// process: Ubuntu's build crashes inside SDL2's audio init when two cores probe at
/// once. The app only ever runs one core, so these tests take turns.
static ONE_CORE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn take_turn() -> MutexGuard<'static, ()> {
    ONE_CORE_AT_A_TIME
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn libmpv() -> Libmpv {
    let loaded = match std::env::var_os("MPV_LIB") {
        Some(path) => Libmpv::load_from(Path::new(&path)),
        None => Libmpv::load(),
    };
    loaded.unwrap_or_else(|err| panic!("{err}"))
}

fn player() -> Player {
    let engine = MpvEngine::new(libmpv(), Arc::new(|| {})).unwrap_or_else(|err| panic!("{err}"));
    Player::new(Box::new(engine))
}

fn temp_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mediaplayer-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the temp folder");
    dir.join(name)
}

/// A mono 16-bit PCM WAV with a 440 Hz tone.
fn write_wav(path: &Path, seconds: f32) {
    const RATE: u32 = 44_100;
    let samples = (RATE as f32 * seconds) as u32;
    let data_len = samples * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1_u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..samples {
        let sample = ((n as f32 / RATE as f32 * 440.0 * TAU).sin() * 8000.0) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, wav).expect("write the WAV file");
}

/// Ticks the player like the UI does until `done` holds.
fn wait_until(player: &mut Player, what: &str, done: impl Fn(&Player) -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        player.tick();
        if done(player) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "timed out waiting for {what}: state {:?}, position {:?}, error {:?}",
        player.state(),
        player.position(),
        player.error()
    );
}

#[test]
#[ignore = "needs libmpv"]
fn plays_pauses_seeks_and_finishes_a_real_file() {
    let _turn = take_turn();
    let wav = temp_file("tone.wav");
    write_wav(&wav, 3.0);
    let mut player = player();
    assert!(!player.engine_info().simulated);

    player.dispatch(PlayerCommand::Open(wav));
    wait_until(&mut player, "playback", |p| {
        p.state() == PlaybackState::Playing
    });
    let duration = player.duration().expect("libmpv reports the duration");
    assert!(
        (duration.as_secs_f64() - 3.0).abs() < 0.1,
        "duration {duration:?}"
    );
    assert_eq!(player.video(), None, "a WAV has no picture");
    wait_until(&mut player, "the clock to run", |p| {
        p.position() >= Duration::from_millis(200)
    });

    player.dispatch(PlayerCommand::Pause);
    std::thread::sleep(Duration::from_millis(100));
    player.tick();
    let paused_at = player.position();
    std::thread::sleep(Duration::from_millis(400));
    player.tick();
    assert!(
        player.position().saturating_sub(paused_at) < Duration::from_millis(100),
        "position moved while paused: {paused_at:?} -> {:?}",
        player.position()
    );

    player.dispatch(PlayerCommand::SetVolume(0.5));
    player.dispatch(PlayerCommand::ToggleMute);
    player.dispatch(PlayerCommand::SeekTo(Duration::from_millis(2500)));
    player.dispatch(PlayerCommand::Play);
    wait_until(&mut player, "the end", |p| {
        p.state() == PlaybackState::Ended
    });
    assert_eq!(player.position(), duration);
    assert!(player.error().is_none());
}

#[test]
#[ignore = "needs libmpv"]
fn opening_another_file_quickly_plays_the_second_one() {
    let _turn = take_turn();
    let first = temp_file("first.wav");
    let second = temp_file("second.wav");
    write_wav(&first, 4.0);
    write_wav(&second, 1.0);
    let mut player = player();

    player.dispatch(PlayerCommand::Open(first));
    player.dispatch(PlayerCommand::Open(second.clone()));
    wait_until(&mut player, "playback", |p| {
        p.state() == PlaybackState::Playing
    });
    let duration = player.duration().expect("duration").as_secs_f64();
    assert!(
        (duration - 1.0).abs() < 0.1,
        "got the first file: {duration}"
    );
    assert_eq!(
        player.media().map(|item| item.path().to_path_buf()),
        Some(second)
    );
    wait_until(&mut player, "the end", |p| {
        p.state() == PlaybackState::Ended
    });
}

#[test]
#[ignore = "needs libmpv"]
fn a_file_libmpv_cannot_decode_reports_an_error() {
    let _turn = take_turn();
    let junk = temp_file("junk.mp4");
    std::fs::write(&junk, b"not a video ".repeat(64)).unwrap();
    let mut player = player();

    player.dispatch(PlayerCommand::Open(junk));
    wait_until(&mut player, "an error", |p| p.error().is_some());
    assert!(
        matches!(player.error(), Some(PlayerError::Backend { .. })),
        "{:?}",
        player.error()
    );
    assert_eq!(player.state(), PlaybackState::Idle);
}
