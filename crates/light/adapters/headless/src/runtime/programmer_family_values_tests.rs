use super::*;
use light_core::programming::ColorComponent;

fn tokens(text: &str) -> Vec<String> {
    tokenize_programmer_command(text).unwrap().0
}

fn parse(text: &str) -> Result<FamilyValues, String> {
    let tokens = tokens(text);
    let family = family_value_keyword(&tokens).expect("a family keyword");
    parse_family_values(family, &tokens[1..])
}

#[test]
fn keyword_excludes_preset_recall_and_intensity() {
    assert_eq!(
        family_value_keyword(&tokens("COLOR 100")),
        Some(CommandFamily::Color)
    );
    assert_eq!(
        family_value_keyword(&tokens("COLOUR 100")),
        Some(CommandFamily::Color)
    );
    assert_eq!(family_value_keyword(&tokens("COLOR PRESET 22")), None);
    assert_eq!(family_value_keyword(&tokens("50")), None);
    assert_eq!(
        family_value_keyword(&tokens("FOCUS 20")),
        Some(CommandFamily::Focus)
    );
}

#[test]
fn components_follow_encoder_order_and_display_units() {
    let values = parse("COLOR 100 DIV DIV 25").unwrap();
    assert_eq!(values.edits.len(), 2);
    assert_eq!(
        values.edits[0].0.component,
        ProgrammingComponent::Color(ColorComponent::Red)
    );
    assert_eq!(values.edits[0].1, ScalarEdit::Set(ScalarIntent::Value(1.0)));
    assert_eq!(
        values.edits[1].0.component,
        ProgrammingComponent::Color(ColorComponent::Blue)
    );
    assert_eq!(
        values.edits[1].1,
        ScalarEdit::Set(ScalarIntent::Value(0.25))
    );

    let values = parse("POSITION - - 90 DIV + 5").unwrap();
    assert_eq!(
        values.edits[0].1,
        ScalarEdit::Set(ScalarIntent::Value(-90.))
    );
    assert_eq!(values.edits[1].1, ScalarEdit::Relative(5.));

    let values = parse("FOCUS DIV 20").unwrap();
    assert_eq!(values.edits[0].0.component, ProgrammingComponent::Zoom);
    assert_eq!(values.edits[0].1, ScalarEdit::Set(ScalarIntent::Value(20.)));

    // The keypad shows a doubled [DIV] as OFFSET.
    let keypad = parse("COLOR OFFSET 100").unwrap();
    assert_eq!(
        keypad.edits[0].0.component,
        ProgrammingComponent::Color(ColorComponent::Blue)
    );

    let values = parse("FOCUS FULL").unwrap();
    assert_eq!(values.edits[0].1, ScalarEdit::Set(ScalarIntent::Value(1.)));
}

#[test]
fn thru_spreads_every_given_component_and_rejects_partial_points() {
    let values = parse("COLOR 100 DIV 0 THRU 0 DIV 100 THRU 50 DIV 50").unwrap();
    assert_eq!(values.spread_points, 3);
    assert_eq!(
        values.edits[0].1,
        ScalarEdit::Set(ScalarIntent::Spread(vec![1., 0., 0.5]))
    );
    assert_eq!(
        values.edits[1].1,
        ScalarEdit::Set(ScalarIntent::Spread(vec![0., 1., 0.5]))
    );
    assert!(parse("COLOR 100 THRU DIV 5").is_err());
    assert!(parse("POSITION + 5 THRU 10").is_err());
}

#[test]
fn malformed_values_are_rejected() {
    assert!(parse("COLOR").is_err());
    assert!(parse("COLOR DIV DIV").is_err());
    assert!(parse("COLOR 1 DIV 2 DIV 3 DIV 4 DIV 5").is_err());
    assert!(parse("COLOR 101").is_err());
    assert!(parse("FOCUS DIV 200").is_err());
    assert!(parse("POSITION ABC").is_err());
    assert!(parse("POSITION 1 DIV 2 DIV 3").is_err());
}
