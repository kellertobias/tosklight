use crate::{CommandLineState, CommandTarget};

mod helpers;

pub use helpers::remove_command_token;
use helpers::{
    collapse_whitespace, contains_word, ends_operator, is_selection_command, last_word_is_any,
    replace_last_word, starts_with_words, strip_prefix_word,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandKeyPhase {
    Press,
    Release,
}

impl TryFrom<&str> for CommandKeyPhase {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "press" => Ok(Self::Press),
            "release" => Ok(Self::Release),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandKey {
    Set,
    Group,
    Cue,
    Playback,
    Off,
    Undo,
    Clear,
    Delete,
    Move,
    Copy,
    Thru,
    Divide,
    Backspace,
    At,
    Enter,
    Preload,
    Record,
    Escape,
    Shift,
    Time,
    Delay,
    Link,
    Select,
    Highlight,
    Previous,
    Next,
    All,
    EncoderPlayback,
    PageUp,
    PageDown,
    Align,
    Fade,
    Plus,
    Minus,
    Dot,
    Digit(u8),
}

impl TryFrom<&str> for CommandKey {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() == 1
            && let Some(digit) = value.as_bytes()[0].checked_sub(b'0')
            && digit <= 9
        {
            return Ok(Self::Digit(digit));
        }
        match value {
            "SET" => Ok(Self::Set),
            "GRP" => Ok(Self::Group),
            "CUE" => Ok(Self::Cue),
            "PBK" | "PLAYBACK" => Ok(Self::Playback),
            "OFF" => Ok(Self::Off),
            "UND" => Ok(Self::Undo),
            "CLR" => Ok(Self::Clear),
            "DEL" => Ok(Self::Delete),
            "MOV" => Ok(Self::Move),
            "CPY" => Ok(Self::Copy),
            "TRU" => Ok(Self::Thru),
            "DIV" => Ok(Self::Divide),
            "BACKSPACE" => Ok(Self::Backspace),
            "AT" => Ok(Self::At),
            "ENT" => Ok(Self::Enter),
            "PRE" => Ok(Self::Preload),
            "REC" => Ok(Self::Record),
            "ESC" => Ok(Self::Escape),
            "SHIFT" => Ok(Self::Shift),
            "TIME" => Ok(Self::Time),
            "DELAY" => Ok(Self::Delay),
            "LINK" => Ok(Self::Link),
            "SELECT" => Ok(Self::Select),
            "HIGH" => Ok(Self::Highlight),
            "PREV" => Ok(Self::Previous),
            "NEXT" => Ok(Self::Next),
            "ALL" => Ok(Self::All),
            "ENC" => Ok(Self::EncoderPlayback),
            "PGUP" => Ok(Self::PageUp),
            "PGDN" => Ok(Self::PageDown),
            "ALIGN" => Ok(Self::Align),
            "FADE" => Ok(Self::Fade),
            "+" => Ok(Self::Plus),
            "-" => Ok(Self::Minus),
            "." => Ok(Self::Dot),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandLineEdit {
    pub text: String,
    pub target: CommandTarget,
    pub pristine: bool,
    pub execute: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandKeyIntent {
    NoOp,
    Edit(CommandLineEdit),
    Clear,
    Undo,
    Preload,
    Shift { pressed: bool },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandGestureKind {
    Regular,
    Double,
    Hold,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CommandGesture {
    pub kind: CommandGestureKind,
    pub shifted: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandImmediateAction {
    AlignOff,
    ClearPreload,
    InspectFixtures,
    InspectGroups,
    InspectPreload,
    Lock,
    RecordOptions,
    RunningOutput,
    Undo,
    UpdateOptions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandGestureIntent {
    Key(CommandKeyIntent),
    Edit(CommandLineEdit),
    Immediate(CommandImmediateAction),
}

pub fn command_gesture_intent(
    state: &CommandLineState,
    key: CommandKey,
    gesture: CommandGesture,
) -> CommandGestureIntent {
    use CommandGestureKind::{Double, Hold};
    use CommandImmediateAction as Action;

    if gesture.kind == Hold {
        let action = match (key, gesture.shifted) {
            (CommandKey::Group, false) => Action::InspectGroups,
            (CommandKey::Group, true) => Action::InspectFixtures,
            (CommandKey::Record, false) => Action::RecordOptions,
            (CommandKey::Record, true) => Action::UpdateOptions,
            (CommandKey::Preload, _) => Action::InspectPreload,
            _ => return CommandGestureIntent::Key(CommandKeyIntent::NoOp),
        };
        return CommandGestureIntent::Immediate(action);
    }

    if !gesture.shifted {
        if gesture.kind == Double {
            if key == CommandKey::Off {
                return CommandGestureIntent::Immediate(Action::RunningOutput);
            }
            let replacement = match key {
                CommandKey::Group => Some(("DEGROUP", &["GROUP", "FIXTURE"][..])),
                CommandKey::Cue => Some(("CUELIST", &["CUE"][..])),
                CommandKey::Playback => Some(("VPBK", &["PBK"][..])),
                _ => None,
            };
            if let Some((text, roots)) = replacement {
                return CommandGestureIntent::Edit(gesture_edit(state, text, roots));
            }
        }
        return CommandGestureIntent::Key(command_key_intent(state, key, CommandKeyPhase::Press));
    }

    if gesture.kind == Double {
        if let CommandKey::Digit(digit) = key
            && let Some(family) = shifted_root(CommandKey::Digit(digit))
        {
            return CommandGestureIntent::Edit(gesture_edit(state, preset_root(family), &[family]));
        }
        let replacement = match key {
            CommandKey::Group => Some(("DMX", &["FIXTURE"][..])),
            CommandKey::Divide => Some(("LOAD", &["GO TO"][..])),
            CommandKey::Clear => Some(("UNFREEZE", &["FREEZE"][..])),
            _ => None,
        };
        if let Some((text, roots)) = replacement {
            return CommandGestureIntent::Edit(gesture_edit(state, text, roots));
        }
    }

    let action = match key {
        CommandKey::Enter => Some(Action::Lock),
        CommandKey::Escape => Some(Action::Undo),
        CommandKey::Preload => Some(Action::ClearPreload),
        CommandKey::Align => Some(Action::AlignOff),
        _ => None,
    };
    if let Some(action) = action {
        return CommandGestureIntent::Immediate(action);
    }
    if key == CommandKey::Clear {
        return CommandGestureIntent::Edit(edited("FREEZE".into(), state.target));
    }
    shifted_root(key).map_or(CommandGestureIntent::Key(CommandKeyIntent::NoOp), |text| {
        CommandGestureIntent::Edit(gesture_edit(state, text, &[]))
    })
}

fn shifted_root(key: CommandKey) -> Option<&'static str> {
    match key {
        CommandKey::Digit(0) => Some("ALL"),
        CommandKey::Digit(1) => Some("INTENSITY"),
        CommandKey::Digit(2) => Some("COLOR"),
        CommandKey::Digit(3) => Some("POSITION"),
        CommandKey::Digit(4) => Some("BEAM"),
        CommandKey::Digit(5) => Some("DYNAMICS"),
        CommandKey::Digit(6) => Some("SHAPERS"),
        CommandKey::Digit(7) => Some("FOCUS"),
        CommandKey::Digit(8) => Some("CONTROL"),
        CommandKey::Digit(9) => Some("MEDIA"),
        CommandKey::At => Some("FixAT"),
        CommandKey::Group => Some("FIXTURE"),
        CommandKey::Cue => Some("TIMECODE"),
        CommandKey::Playback => Some("MACRO"),
        CommandKey::Set => Some("ASSIGN"),
        CommandKey::Time => Some("SPD GRP"),
        CommandKey::Divide => Some("GO TO"),
        CommandKey::Off => Some("RELEASE"),
        CommandKey::Move => Some("COPY"),
        CommandKey::Record => Some("UPDATE"),
        CommandKey::Clear => Some("FREEZE"),
        _ => None,
    }
}

fn preset_root(family: &str) -> &'static str {
    match family {
        "ALL" => "ALL PRESET",
        "INTENSITY" => "INTENSITY PRESET",
        "COLOR" => "COLOR PRESET",
        "POSITION" => "POSITION PRESET",
        "BEAM" => "BEAM PRESET",
        "DYNAMICS" => "DYNAMICS PRESET",
        "SHAPERS" => "SHAPERS PRESET",
        "FOCUS" => "FOCUS PRESET",
        "CONTROL" => "CONTROL PRESET",
        "MEDIA" => "MEDIA PRESET",
        _ => unreachable!("only documented shifted family roots are passed"),
    }
}

fn gesture_edit(state: &CommandLineState, text: &str, replace: &[&str]) -> CommandLineEdit {
    let current = state.visible_text().trim();
    let next = replace
        .iter()
        .find_map(|root| {
            let prefix = current.get(..current.len().saturating_sub(root.len()))?;
            current
                .ends_with(root)
                .then(|| format!("{} {text}", prefix.trim_end()).trim().to_owned())
        })
        .unwrap_or_else(|| {
            if state.pristine || matches!(current, "FIXTURE" | "GROUP") {
                text.to_owned()
            } else {
                format!("{current} {text}")
            }
        });
    edited(next, state.target)
}

pub fn command_key_intent(
    state: &CommandLineState,
    key: CommandKey,
    phase: CommandKeyPhase,
) -> CommandKeyIntent {
    if key == CommandKey::Shift {
        return CommandKeyIntent::Shift {
            pressed: phase == CommandKeyPhase::Press,
        };
    }
    if phase == CommandKeyPhase::Release {
        return CommandKeyIntent::NoOp;
    }
    match key {
        CommandKey::Clear => CommandKeyIntent::Clear,
        CommandKey::Undo => CommandKeyIntent::Undo,
        CommandKey::Preload => CommandKeyIntent::Preload,
        CommandKey::Escape => CommandKeyIntent::Edit(default_edit(state.target)),
        CommandKey::Record => CommandKeyIntent::Edit(toggle_record(state)),
        CommandKey::Enter => CommandKeyIntent::Edit(enter_edit(state)),
        _ => CommandKeyIntent::Edit(edit_text(state, key)),
    }
}

fn default_edit(target: CommandTarget) -> CommandLineEdit {
    CommandLineEdit {
        text: target.as_str().to_owned(),
        target,
        pristine: true,
        execute: false,
    }
}

fn toggle_record(state: &CommandLineState) -> CommandLineEdit {
    let text = state.visible_text().trim();
    let next = if let Some(remainder) = strip_prefix_word(text, "RECORD") {
        if remainder.is_empty() {
            state.target.as_str().to_owned()
        } else {
            remainder.to_owned()
        }
    } else {
        "RECORD ".into()
    };
    CommandLineEdit {
        pristine: next.eq_ignore_ascii_case(state.target.as_str()),
        text: next,
        target: state.target,
        execute: false,
    }
}

fn enter_edit(state: &CommandLineState) -> CommandLineEdit {
    let opposite = match state.target {
        CommandTarget::Fixture => CommandTarget::Group,
        CommandTarget::Group => CommandTarget::Fixture,
    };
    if !state.pristine && state.visible_text().trim().eq_ignore_ascii_case("DEGROUP") {
        return default_edit(opposite);
    }
    CommandLineEdit {
        text: state.visible_text().to_owned(),
        target: state.target,
        pristine: state.pristine,
        execute: true,
    }
}

fn edit_text(state: &CommandLineState, key: CommandKey) -> CommandLineEdit {
    let command = state.visible_text();
    if key == CommandKey::Backspace {
        if state.pristine {
            return default_edit(state.target);
        }
        let next = remove_command_token(command);
        if next.is_empty() {
            return default_edit(state.target);
        }
        return CommandLineEdit {
            text: next,
            target: state.target,
            pristine: false,
            execute: false,
        };
    }

    if state.pristine {
        if let CommandKey::Digit(digit) = key {
            return edited(
                format!("{}{}", short_target(state.target), digit),
                state.target,
            );
        }
        if key == CommandKey::Group {
            let opposite = match state.target {
                CommandTarget::Fixture => CommandTarget::Group,
                CommandTarget::Group => CommandTarget::Fixture,
            };
            return edited(opposite.as_str().to_owned(), state.target);
        }
        if let Some(root) = root_token(key) {
            return edited(root.to_owned(), state.target);
        }
    }

    let selection_command = is_selection_command(command);
    if key == CommandKey::Group && selection_command && ends_operator(command) {
        let override_target = match state.target {
            CommandTarget::Fixture => CommandTarget::Group,
            CommandTarget::Group => CommandTarget::Fixture,
        };
        return edited(
            format!("{} {}", command.trim_end(), short_target(override_target)),
            state.target,
        );
    }
    if key == CommandKey::Group && last_word_is_any(command, &["GROUP", "G", "F"]) {
        return edited(replace_last_word(command, "DEGROUP"), state.target);
    }
    if key == CommandKey::Cue && last_word_is_any(command, &["CUE"]) {
        return edited(replace_last_word(command, "CUELIST"), state.target);
    }
    if key == CommandKey::Playback && last_word_is_any(command, &["PBK"]) {
        return edited(replace_last_word(command, "VPBK"), state.target);
    }
    if key == CommandKey::At && last_word_is_any(command, &["AT"]) {
        let mut edit = edited(replace_last_word(command, "AT 100"), state.target);
        edit.execute = true;
        return edit;
    }
    if key == CommandKey::Dot && starts_with_words(command, &["SPD", "GRP"]) {
        return edited(format!("{command},"), state.target);
    }
    if key == CommandKey::Dot && command.trim_end().ends_with('.') {
        let without_dot = command
            .trim_end()
            .strip_suffix('.')
            .expect("the command was checked to end with a dot")
            .trim_end();
        let mut edit = edited(
            format!("{without_dot} AT 0").trim().to_owned(),
            state.target,
        );
        edit.execute = true;
        return edit;
    }
    if key == CommandKey::Time && last_word_is_any(command, &["TIME"]) {
        return edited(replace_last_word(command, "DELAY"), state.target);
    }
    if key == CommandKey::Divide && last_word_is_any(command, &["DIV"]) {
        return edited(replace_last_word(command, "OFFSET"), state.target);
    }

    let token = command_token(key);
    let spaced = matches!(
        key,
        CommandKey::Group
            | CommandKey::Cue
            | CommandKey::Playback
            | CommandKey::Delete
            | CommandKey::Move
            | CommandKey::Copy
            | CommandKey::Thru
            | CommandKey::Divide
            | CommandKey::Set
            | CommandKey::At
            | CommandKey::Time
            | CommandKey::Delay
            | CommandKey::Link
            | CommandKey::Select
            | CommandKey::Plus
            | CommandKey::Minus
    );
    if let CommandKey::Digit(digit) = key
        && ["GROUP", "FIXTURE"]
            .iter()
            .any(|target| command.trim().eq_ignore_ascii_case(target))
    {
        let prefix = if command.trim().eq_ignore_ascii_case("GROUP") {
            'G'
        } else {
            'F'
        };
        return edited(format!("{prefix}{digit}"), state.target);
    }
    let selection_continuation = (selection_command || command.trim() == "+")
        && ends_operator(command)
        && !contains_word(command, "AT");
    let next_token = match key {
        CommandKey::Digit(digit) if selection_continuation => {
            format!("{}{}", short_target(state.target), digit)
        }
        CommandKey::Digit(digit) if last_word_is_any(command, &["F", "G"]) => digit.to_string(),
        CommandKey::Digit(digit)
            if command
                .trim_end()
                .chars()
                .next_back()
                .is_some_and(|character| {
                    character.is_ascii_alphabetic() || matches!(character, '+' | '-')
                }) =>
        {
            format!(" {digit}")
        }
        _ => token,
    };
    let combined =
        if matches!(key, CommandKey::Digit(_)) && selection_continuation && command.trim() != "+" {
            format!("{} {next_token}", command.trim_end())
        } else if spaced {
            format!("{command} {next_token}")
        } else {
            format!("{command}{next_token}")
        };
    edited(collapse_whitespace(&combined), state.target)
}

fn edited(text: String, target: CommandTarget) -> CommandLineEdit {
    CommandLineEdit {
        text,
        target,
        pristine: false,
        execute: false,
    }
}

fn root_token(key: CommandKey) -> Option<&'static str> {
    match key {
        CommandKey::Cue => Some("CUE"),
        CommandKey::Playback => Some("PBK"),
        CommandKey::Off => Some("OFF"),
        CommandKey::Delete => Some("DELETE"),
        CommandKey::Move => Some("MOVE"),
        CommandKey::Copy => Some("COPY"),
        CommandKey::Set => Some("SET"),
        CommandKey::At => Some("AT"),
        CommandKey::Time => Some("TIME"),
        CommandKey::Link => Some("LINK"),
        CommandKey::Select => Some("SELECT"),
        CommandKey::Plus => Some("+"),
        CommandKey::Minus => Some("-"),
        CommandKey::Dot => Some("."),
        _ => None,
    }
}

fn command_token(key: CommandKey) -> String {
    match key {
        CommandKey::Group => "GROUP".into(),
        CommandKey::Cue => "CUE".into(),
        CommandKey::Playback => "PBK".into(),
        CommandKey::Off => "OFF".into(),
        CommandKey::Delete => "DELETE".into(),
        CommandKey::Move => "MOVE".into(),
        CommandKey::Copy => "COPY".into(),
        CommandKey::Thru => "THRU".into(),
        CommandKey::Divide => "DIV".into(),
        CommandKey::Set => "SET".into(),
        CommandKey::At => "AT".into(),
        CommandKey::Time => "TIME".into(),
        CommandKey::Delay => "DELAY".into(),
        CommandKey::Link => "LINK".into(),
        CommandKey::Select => "SELECT".into(),
        CommandKey::Plus => "+".into(),
        CommandKey::Minus => "-".into(),
        CommandKey::Dot => ".".into(),
        CommandKey::Digit(digit) => digit.to_string(),
        _ => String::new(),
    }
}

fn short_target(target: CommandTarget) -> char {
    match target {
        CommandTarget::Fixture => 'F',
        CommandTarget::Group => 'G',
    }
}

#[cfg(test)]
#[path = "command_line/tests.rs"]
mod tests;
