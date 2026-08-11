use super::*;

#[tokio::test]
async fn macro_preflight_keeps_one_detached_programmer_and_never_mutates_live_state() {
    let scenario = OperationalScenario::new().await;
    scenario.seed_and_open_show().await;
    let session = authenticate_token(&scenario.state, &scenario.token).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let assert_live_programmer_untouched = || {
        let live = scenario.state.programming.get(session.id).unwrap();
        assert!(live.selected.is_empty());
        assert!(live.selection_expression.is_none());
        assert!(live.values.is_empty());
        assert!(live.group_values.is_empty());
        assert!(live.dynamic_values.is_empty());
        assert!(live.command_line.is_empty());
        assert!(live.undo.is_empty());
    };
    assert_live_programmer_untouched();

    // The second line is valid only when preflight retains the first line's simulated selection.
    prevalidate_programmer_commands_from(
        &scenario.state,
        &session,
        &["FIXTURE 1", "AT 50"],
        &context,
    )
    .unwrap();
    assert_live_programmer_untouched();

    // A context-invalid later line rejects the entire document before the valid earlier lines can
    // touch the live Programmer.
    let failure = prevalidate_programmer_commands_from(
        &scenario.state,
        &session,
        &["FIXTURE 1", "AT 50", "NOT-A-COMMAND 9999"],
        &context,
    )
    .unwrap_err();
    assert_eq!(failure.0, 2);
    assert!(failure.1.contains("invalid"), "{}", failure.1);
    assert_live_programmer_untouched();

    drop(scenario);
}
