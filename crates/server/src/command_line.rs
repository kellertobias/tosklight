use light_programmer::{CommandLineState, CommandTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKey {
    Set,
    Group,
    Cue,
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
    Select,
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
            "SELECT" => Ok(Self::Select),
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
    if !state.pristine
        && state
            .visible_text()
            .trim()
            .eq_ignore_ascii_case(opposite.as_str())
    {
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
        return edited(replace_last_word(command, "DEGRP"), state.target);
    }
    if key == CommandKey::At && last_word_is_any(command, &["AT"]) {
        let mut edit = edited(replace_last_word(command, "AT FULL"), state.target);
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
        let mut edit = edited(format!("{without_dot} AT 0"), state.target);
        edit.execute = true;
        return edit;
    }
    if key == CommandKey::Time && last_word_is_any(command, &["TIME"]) {
        return edited(
            format!("{} ", replace_last_word(command, "DELAY")),
            state.target,
        );
    }

    let token = command_token(key);
    let spaced = matches!(
        key,
        CommandKey::Group
            | CommandKey::Cue
            | CommandKey::Delete
            | CommandKey::Move
            | CommandKey::Copy
            | CommandKey::Thru
            | CommandKey::Divide
            | CommandKey::Set
            | CommandKey::At
            | CommandKey::Time
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
                .is_some_and(|character| character.is_ascii_alphabetic()) =>
        {
            format!(" {digit}")
        }
        _ => token,
    };
    let combined = if spaced {
        format!("{command} {next_token} ")
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
        CommandKey::Delete => Some("DELETE"),
        CommandKey::Move => Some("MOVE"),
        CommandKey::Copy => Some("COPY"),
        CommandKey::Set => Some("SET"),
        CommandKey::At => Some("AT"),
        CommandKey::Time => Some("TIME"),
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
        CommandKey::Delete => "DELETE".into(),
        CommandKey::Move => "MOVE".into(),
        CommandKey::Copy => "COPY".into(),
        CommandKey::Thru => "THRU".into(),
        CommandKey::Divide => "DIV".into(),
        CommandKey::Set => "SET".into(),
        CommandKey::At => "AT".into(),
        CommandKey::Time => "TIME".into(),
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

fn strip_prefix_word<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    let remainder = &value[prefix.len()..];
    (remainder.is_empty() || remainder.starts_with(char::is_whitespace))
        .then(|| remainder.trim_start())
}

fn starts_with_words(value: &str, words: &[&str]) -> bool {
    value
        .split_whitespace()
        .zip(words)
        .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
        && value.split_whitespace().count() >= words.len()
}

fn is_selection_command(value: &str) -> bool {
    let trimmed = value.trim_start();
    let mut characters = trimmed.chars();
    if matches!(characters.next(), Some('F' | 'f' | 'G' | 'g'))
        && characters
            .next()
            .is_some_and(|value| value.is_ascii_digit())
    {
        return true;
    }
    ["FIXTURE", "GROUP", "DEGRP"].iter().any(|prefix| {
        trimmed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            && trimmed
                .get(prefix.len()..)
                .is_some_and(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace))
    })
}

fn contains_word(value: &str, word: &str) -> bool {
    value
        .split_whitespace()
        .any(|candidate| candidate.eq_ignore_ascii_case(word))
}

fn last_word_is_any(value: &str, expected: &[&str]) -> bool {
    value.split_whitespace().next_back().is_some_and(|word| {
        expected
            .iter()
            .any(|candidate| word.eq_ignore_ascii_case(candidate))
    })
}

fn ends_operator(value: &str) -> bool {
    value
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|character| matches!(character, '+' | '-'))
}

fn replace_last_word(value: &str, replacement: &str) -> String {
    let trimmed = value.trim_end();
    let start = trimmed
        .char_indices()
        .rev()
        .find_map(|(index, character)| character.is_whitespace().then_some(index + 1))
        .unwrap_or(0);
    format!("{}{replacement}", &trimmed[..start])
}

pub fn remove_command_token(value: &str) -> String {
    let trimmed = value.trim_end();
    let Some(last) = trimmed.chars().next_back() else {
        return String::new();
    };
    if last.is_ascii_digit() || matches!(last, '.' | '-') {
        let end = trimmed.len() - last.len_utf8();
        return trimmed[..end].trim_end().to_owned();
    }
    let mut start = trimmed.len();
    for (index, character) in trimmed.char_indices().rev() {
        if character.is_ascii_alphabetic() {
            start = index;
        } else {
            break;
        }
    }
    trimmed[..start].trim_end().to_owned()
}

fn collapse_whitespace(value: &str) -> String {
    let trailing = value.chars().next_back().is_some_and(char::is_whitespace);
    let mut collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if trailing && !collapsed.is_empty() {
        collapsed.push(' ');
    }
    collapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str, target: CommandTarget, pristine: bool) -> CommandLineState {
        CommandLineState {
            text: text.into(),
            target,
            pristine,
            revision: 0,
        }
    }

    fn press(text: &str, target: CommandTarget, pristine: bool, key: &str) -> CommandLineEdit {
        let CommandKeyIntent::Edit(edit) = command_key_intent(
            &state(text, target, pristine),
            CommandKey::try_from(key).unwrap(),
            CommandKeyPhase::Press,
        ) else {
            panic!("expected an edit intent");
        };
        edit
    }

    #[test]
    fn edits_documented_shortcuts_and_timing_tokens() {
        assert_eq!(
            press("1 AT ", CommandTarget::Fixture, false, "AT").text,
            "1 AT FULL"
        );
        assert!(press("1 AT ", CommandTarget::Fixture, false, "AT").execute);
        assert_eq!(
            press("1.", CommandTarget::Fixture, false, ".").text,
            "1 AT 0"
        );
        assert!(press("1.", CommandTarget::Fixture, false, ".").execute);
        assert_eq!(
            press("1..", CommandTarget::Fixture, false, ".").text,
            "1. AT 0"
        );
        assert_eq!(
            press("1 AT 100 TIME ", CommandTarget::Fixture, false, "TIME").text,
            "1 AT 100 DELAY "
        );
        assert_eq!(
            press("SPD GRP 2 AT 127", CommandTarget::Fixture, false, ".").text,
            "SPD GRP 2 AT 127,"
        );
    }

    #[test]
    fn keeps_target_scoping_and_group_dereference_rules() {
        assert_eq!(
            press("FIXTURE", CommandTarget::Fixture, true, "7").text,
            "F7"
        );
        assert_eq!(
            press("G7 + ", CommandTarget::Fixture, false, "8").text,
            "G7 + F8"
        );
        assert_eq!(press("+", CommandTarget::Group, false, "4").text, "+G4");
        assert_eq!(
            press("G7 + ", CommandTarget::Fixture, false, "GRP").text,
            "G7 + G"
        );
        assert_eq!(
            press("G7 + ", CommandTarget::Group, false, "GRP").text,
            "G7 + F"
        );
        assert_eq!(
            press("GROUP", CommandTarget::Fixture, false, "GRP").text,
            "DEGRP"
        );
        assert_eq!(
            press("RECORD + ", CommandTarget::Group, false, "GRP").text,
            "RECORD + GROUP "
        );
        assert_eq!(
            press("RECORD GROUP", CommandTarget::Fixture, false, "7").text,
            "RECORD GROUP 7"
        );
    }

    #[test]
    fn builds_cue_select_and_complete_group_mode_sequences() {
        let cue = press("FIXTURE", CommandTarget::Fixture, true, "CUE");
        assert_eq!(
            press(&cue.text, CommandTarget::Fixture, cue.pristine, "8").text,
            "CUE 8"
        );
        let nested_cue = press(&cue.text, CommandTarget::Fixture, cue.pristine, "CUE");
        assert_eq!(
            press(
                &nested_cue.text,
                CommandTarget::Fixture,
                nested_cue.pristine,
                "8"
            )
            .text,
            "CUE CUE 8"
        );
        assert_eq!(
            press("FIXTURE", CommandTarget::Fixture, true, "SELECT").text,
            "SELECT"
        );

        let fixture_override = press("G7 + ", CommandTarget::Group, false, "GRP");
        assert_eq!(fixture_override.text, "G7 + F");
        assert_eq!(
            press(
                &fixture_override.text,
                CommandTarget::Group,
                fixture_override.pristine,
                "8"
            )
            .text,
            "G7 + F8"
        );
        let fixture = press("GROUP", CommandTarget::Group, true, "GRP");
        assert_eq!(fixture.text, "FIXTURE");
        assert_eq!(
            press(&fixture.text, CommandTarget::Group, fixture.pristine, "1").text,
            "F1"
        );
    }

    #[test]
    fn bare_opposite_target_enter_changes_the_persistent_scope() {
        let group = press("GROUP", CommandTarget::Fixture, false, "ENT");
        assert_eq!(group, default_edit(CommandTarget::Group));
        let fixture = press("FIXTURE", CommandTarget::Group, false, "ENT");
        assert_eq!(fixture, default_edit(CommandTarget::Fixture));
    }

    #[test]
    fn release_is_ignored_and_shift_keeps_both_phases() {
        let current = state("F1", CommandTarget::Fixture, false);
        assert_eq!(
            command_key_intent(&current, CommandKey::Digit(2), CommandKeyPhase::Release),
            CommandKeyIntent::NoOp
        );
        assert_eq!(
            command_key_intent(&current, CommandKey::Shift, CommandKeyPhase::Press),
            CommandKeyIntent::Shift { pressed: true }
        );
        assert_eq!(
            command_key_intent(&current, CommandKey::Shift, CommandKeyPhase::Release),
            CommandKeyIntent::Shift { pressed: false }
        );
    }

    #[test]
    fn backspace_removes_words_as_tokens_and_numbers_as_characters() {
        assert_eq!(remove_command_token("FIXTURE 12 THRU"), "FIXTURE 12");
        assert_eq!(remove_command_token("FIXTURE 12"), "FIXTURE 1");
        assert_eq!(remove_command_token("FIXTURE 1.5"), "FIXTURE 1.");
        assert_eq!(remove_command_token("FIXTURE 1 -"), "FIXTURE 1");
    }
}
