use super::*;

fn state(text: &str, target: CommandTarget, pristine: bool) -> CommandLineState {
    CommandLineState {
        text: text.into(),
        target,
        pristine,
        revision: 0,
        pending_choice: None,
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
    assert_eq!(remove_command_token("FIXTURE 1 +"), "FIXTURE 1");
}
