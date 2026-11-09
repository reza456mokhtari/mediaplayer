//! Keyboard shortcuts for the transport.

use std::time::Duration;

use eframe::egui::{Context, Key, Modifiers};
use mediaplayer_core::PlayerCommand;

pub const SHORT_SEEK: Duration = Duration::from_secs(5);
pub const LONG_SEEK: Duration = Duration::from_secs(30);
pub const VOLUME_STEP: f32 = 0.05;

/// The shortcut table. More specific chords come first: egui matches modifiers
/// logically, so a plain `ArrowLeft` pattern would also swallow `Shift+ArrowLeft`.
pub fn bindings() -> [(Modifiers, Key, PlayerCommand); 9] {
    [
        (
            Modifiers::SHIFT,
            Key::ArrowLeft,
            PlayerCommand::SeekBackward(LONG_SEEK),
        ),
        (
            Modifiers::SHIFT,
            Key::ArrowRight,
            PlayerCommand::SeekForward(LONG_SEEK),
        ),
        (Modifiers::NONE, Key::Space, PlayerCommand::TogglePlayPause),
        (
            Modifiers::NONE,
            Key::ArrowLeft,
            PlayerCommand::SeekBackward(SHORT_SEEK),
        ),
        (
            Modifiers::NONE,
            Key::ArrowRight,
            PlayerCommand::SeekForward(SHORT_SEEK),
        ),
        (
            Modifiers::NONE,
            Key::ArrowUp,
            PlayerCommand::ChangeVolume(VOLUME_STEP),
        ),
        (
            Modifiers::NONE,
            Key::ArrowDown,
            PlayerCommand::ChangeVolume(-VOLUME_STEP),
        ),
        (Modifiers::NONE, Key::M, PlayerCommand::ToggleMute),
        (Modifiers::NONE, Key::S, PlayerCommand::Stop),
    ]
}

/// Takes this frame's bound key presses out of the input queue, so a focused
/// button doesn't also react to Space, and returns their commands.
/// Nothing is taken while a text field has focus.
pub fn collect(ctx: &Context) -> Vec<PlayerCommand> {
    if ctx.text_edit_focused() {
        return Vec::new();
    }
    ctx.input_mut(|input| {
        let mut commands = Vec::new();
        for (modifiers, key, command) in bindings() {
            let presses = input.count_and_consume_key(modifiers, key);
            commands.extend(std::iter::repeat_n(command, presses));
        }
        commands
    })
}
