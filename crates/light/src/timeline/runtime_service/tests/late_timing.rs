use super::*;

#[test]
fn forward_cue_timing_executes_once_then_repairs_only_elapsed_triggers() {
    for (micros, expected_executions, expected_repairs) in
        [(360_000, 0, 0), (400_000, 1, 0), (520_000, 1, 1)]
    {
        let (service, clock, _) = service();
        let list = execution_cue_list();
        let engine = execution_engine(&list);
        let definition = execution_definition(&list);
        service.install(definition.clone(), None).unwrap();
        service
            .handle(definition.id, TimecodeTransportAction::Go)
            .unwrap();
        service.reconcile_cue_lists(&engine);
        clock.advance(micros);
        service.tick();
        let mut executions = 0;
        let mut repairs = 0;
        let mut commands = Vec::new();
        let report = service.reconcile_cue_lists_with_executor(&engine, |command, origin| {
            assert_eq!(origin, Some(definition.id));
            if let EnginePlaybackCommand::CueList { action, .. } = &command {
                match action {
                    CueListPlaybackAction::ExecuteCueId(cue_id) => {
                        assert_eq!(*cue_id, list.cues[1].id);
                        executions += 1;
                        commands.push("execute");
                    }
                    CueListPlaybackAction::ExecuteTimelineCues(history) => {
                        assert_eq!(history.last(), Some(&(list.cues[1].id, 120)));
                        executions += 1;
                        repairs += 1;
                        commands.push("timed execute");
                    }
                    _ => panic!("unexpected timing command: {action:?}"),
                }
            }
            engine.execute_playback(command)
        });
        assert!(report.failures.is_empty());
        if expected_executions > 0 {
            assert!(
                !engine
                    .playback_runtime_status_for_cue_list(list.id)
                    .unwrap()
                    .playback
                    .discrete_cue_actions_suppressed
            );
        }
        assert_eq!(
            (executions, repairs),
            (expected_executions, expected_repairs)
        );
        if expected_repairs == 1 {
            assert_eq!(commands, ["timed execute"]);
        }
        let mut repeated = 0;
        service.reconcile_cue_lists_with_executor(&engine, |command, _| {
            repeated += 1;
            engine.execute_playback(command)
        });
        assert_eq!(repeated, 0, "the settled Cue must not execute again");
    }
}

#[test]
fn refused_late_cue_execution_never_reconstructs_or_hides_the_failure() {
    let (service, clock, _) = service();
    let list = execution_cue_list();
    let engine = execution_engine(&list);
    let definition = execution_definition(&list);
    service.install(definition.clone(), None).unwrap();
    service
        .handle(definition.id, TimecodeTransportAction::Go)
        .unwrap();
    service.reconcile_cue_lists(&engine);
    clock.advance(520_000);
    service.tick();
    let mut attempts = 0;
    let report = service.reconcile_cue_lists_with_executor(&engine, |command, _| {
        assert!(matches!(
            command,
            EnginePlaybackCommand::CueList {
                action: CueListPlaybackAction::ExecuteTimelineCues(_),
                ..
            }
        ));
        attempts += 1;
        Err("Cue action refused".into())
    });
    assert_eq!(attempts, 1);
    assert_eq!(report.failures[0].message, "Cue action refused");
    assert!(report.activations.is_empty());
    assert_eq!(
        engine
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .current_cue_id,
        Some(list.cues[0].id)
    );
}
