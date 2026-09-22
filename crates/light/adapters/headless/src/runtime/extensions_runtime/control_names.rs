//! The names the desk and the command line know a control-surface key by.
//!
//! An attached wing reports a key press as a contract enum, but everything downstream — the
//! command history an operator reads back, and the desk actions other surfaces mirror — speaks in
//! these spellings. They are operator wording, not Rust wording, so they live apart from the
//! runtime and change only when the desk's own vocabulary does.

use light_extensions_contract::{NavigationAction, ProgrammerKey};

/// The wording the desk expects for a cursor key. Menu and Escape leave the
/// navigation cross entirely, so they carry no direction and report nothing.
pub(super) fn action_value(action: NavigationAction) -> Option<&'static str> {
    match action {
        NavigationAction::Up => Some("up"),
        NavigationAction::Down => Some("down"),
        NavigationAction::Left => Some("left"),
        NavigationAction::Right => Some("right"),
        NavigationAction::PageUp => Some("page-up"),
        NavigationAction::PageDown => Some("page-down"),
        NavigationAction::Menu | NavigationAction::Escape => None,
    }
}

/// The name the command line knows a keypad key by. These spellings travel into
/// desk actions and feedback, so they stay exactly as an operator sees them in
/// the command history rather than following the Rust variant names.
pub(super) fn programmer_key_name(key: ProgrammerKey) -> &'static str {
    use ProgrammerKey::*;
    match key {
        Zero => "0",
        One => "1",
        Two => "2",
        Three => "3",
        Four => "4",
        Five => "5",
        Six => "6",
        Seven => "7",
        Eight => "8",
        Nine => "9",
        Plus => "plus",
        Minus => "minus",
        Point => "point",
        At => "at",
        Enter => "enter",
        Clear => "clear",
        Undo => "undo",
        Group => "group",
        Cue => "cue",
        Playback => "playback",
        Off => "off",
        Record => "record",
        Preload => "preload",
        Delete => "delete",
        Copy => "copy",
        Move => "move",
        Set => "set",
        Time => "time",
        Thru => "thru",
        Divide => "divide",
        Backspace => "backspace",
        Escape => "escape",
        Highlight => "highlight",
        Previous => "previous",
        Next => "next",
        All => "all",
        EncoderPlayback => "encoder_playback",
        PageUp => "page_up",
        PageDown => "page_down",
        Align => "align",
        Fade => "fade",
    }
}
