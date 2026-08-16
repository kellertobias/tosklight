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
        "1 AT 100"
    );
    assert!(press("1 AT ", CommandTarget::Fixture, false, "AT").execute);
    assert_eq!(
        press("1.", CommandTarget::Fixture, false, ".").text,
        "1 AT 0"
    );
    assert_eq!(
        press("1 DIV ", CommandTarget::Fixture, false, "DIV").text,
        "1 OFFSET"
    );
    assert!(press("1.", CommandTarget::Fixture, false, ".").execute);
    assert_eq!(press(".", CommandTarget::Fixture, false, ".").text, "AT 0");
    assert_eq!(
        press("1..", CommandTarget::Fixture, false, ".").text,
        "1. AT 0"
    );
    assert_eq!(
        press("1 AT 100 TIME ", CommandTarget::Fixture, false, "TIME").text,
        "1 AT 100 DELAY"
    );
    assert_eq!(
        press("SPD GRP 2 AT 127", CommandTarget::Fixture, false, ".").text,
        "SPD GRP 2 AT 127,"
    );
}

#[test]
fn normalizes_documented_update_mode_shorthands() {
    assert_eq!(
        press("UPDATE", CommandTarget::Fixture, false, "-").text,
        "UPDATE TRACKED"
    );
    let all = press("UPDATE", CommandTarget::Fixture, false, "+");
    assert_eq!(all.text, "UPDATE ALL");
    assert_eq!(
        press(&all.text, CommandTarget::Fixture, false, "+").text,
        "UPDATE KNOWN"
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
        "DEGROUP"
    );
    assert_eq!(
        press("RECORD + ", CommandTarget::Group, false, "GRP").text,
        "RECORD + GROUP"
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
        "CUELIST 8"
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
fn bare_double_group_enter_changes_the_persistent_scope_without_colliding_with_dereference() {
    let group = press("DEGROUP", CommandTarget::Fixture, false, "ENT");
    assert_eq!(group, default_edit(CommandTarget::Group));
    let fixture = press("DEGROUP", CommandTarget::Group, false, "ENT");
    assert_eq!(fixture, default_edit(CommandTarget::Fixture));

    assert_eq!(
        press("DEGROUP", CommandTarget::Fixture, false, "7").text,
        "DEGROUP 7"
    );
    assert!(press("GROUP", CommandTarget::Fixture, false, "ENT").execute);
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

fn gesture(
    text: &str,
    key: CommandKey,
    kind: CommandGestureKind,
    shifted: bool,
) -> CommandGestureIntent {
    command_gesture_intent(
        &state(text, CommandTarget::Fixture, text == "FIXTURE"),
        key,
        CommandGesture { kind, shifted },
    )
}

#[test]
fn maps_every_documented_shifted_command_root() {
    let cases = [
        (CommandKey::Digit(0), "ALL"),
        (CommandKey::Digit(1), "INTENSITY"),
        (CommandKey::Digit(2), "COLOR"),
        (CommandKey::Digit(3), "POSITION"),
        (CommandKey::Digit(4), "BEAM"),
        (CommandKey::Digit(5), "DYNAMICS"),
        (CommandKey::Digit(6), "SHAPERS"),
        (CommandKey::Digit(7), "FOCUS"),
        (CommandKey::Digit(8), "CONTROL"),
        (CommandKey::Digit(9), "MEDIA"),
        (CommandKey::At, "FixAT"),
        (CommandKey::Group, "FIXTURE"),
        (CommandKey::Cue, "TIMECODE"),
        (CommandKey::Playback, "MACRO"),
        (CommandKey::Set, "ASSIGN"),
        (CommandKey::Time, "SPD GRP"),
        (CommandKey::Divide, "GO TO"),
        (CommandKey::Off, "RELEASE"),
        (CommandKey::Move, "COPY"),
        (CommandKey::Record, "UPDATE"),
        (CommandKey::Clear, "FREEZE"),
    ];
    for (key, expected) in cases {
        let CommandGestureIntent::Edit(edit) =
            gesture("FIXTURE", key, CommandGestureKind::Regular, true)
        else {
            panic!("{key:?} should edit the command line");
        };
        assert_eq!(edit.text, expected, "{key:?}");
    }
}

#[test]
fn resolves_double_gestures_to_one_final_visible_intent() {
    let cases = [
        ("GROUP", CommandKey::Group, false, "DEGROUP"),
        ("CUE", CommandKey::Cue, false, "CUELIST"),
        ("PBK", CommandKey::Playback, false, "VPBK"),
        ("FIXTURE", CommandKey::Group, true, "DMX"),
        ("GO TO", CommandKey::Divide, true, "LOAD"),
        ("FREEZE", CommandKey::Clear, true, "UNFREEZE"),
        ("COLOR", CommandKey::Digit(2), true, "COLOR PRESET"),
    ];
    for (text, key, shifted, expected) in cases {
        let CommandGestureIntent::Edit(edit) =
            gesture(text, key, CommandGestureKind::Double, shifted)
        else {
            panic!("{key:?} should resolve to an edit");
        };
        assert_eq!(edit.text, expected, "{key:?}");
    }
}

#[test]
fn represents_immediate_and_hold_actions_without_adapter_strings() {
    let cases = [
        (
            CommandKey::Off,
            CommandGestureKind::Double,
            false,
            CommandImmediateAction::RunningOutput,
        ),
        (
            CommandKey::Enter,
            CommandGestureKind::Regular,
            true,
            CommandImmediateAction::Lock,
        ),
        (
            CommandKey::Escape,
            CommandGestureKind::Regular,
            true,
            CommandImmediateAction::Undo,
        ),
        (
            CommandKey::Preload,
            CommandGestureKind::Regular,
            true,
            CommandImmediateAction::ClearPreload,
        ),
        (
            CommandKey::Align,
            CommandGestureKind::Regular,
            true,
            CommandImmediateAction::AlignOff,
        ),
        (
            CommandKey::Group,
            CommandGestureKind::Hold,
            false,
            CommandImmediateAction::InspectGroups,
        ),
        (
            CommandKey::Group,
            CommandGestureKind::Hold,
            true,
            CommandImmediateAction::InspectFixtures,
        ),
        (
            CommandKey::Record,
            CommandGestureKind::Hold,
            false,
            CommandImmediateAction::RecordOptions,
        ),
        (
            CommandKey::Record,
            CommandGestureKind::Hold,
            true,
            CommandImmediateAction::UpdateOptions,
        ),
        (
            CommandKey::Preload,
            CommandGestureKind::Hold,
            false,
            CommandImmediateAction::InspectPreload,
        ),
    ];
    for (key, kind, shifted, expected) in cases {
        assert_eq!(
            gesture("FIXTURE", key, kind, shifted),
            CommandGestureIntent::Immediate(expected),
            "{key:?}"
        );
    }
}
