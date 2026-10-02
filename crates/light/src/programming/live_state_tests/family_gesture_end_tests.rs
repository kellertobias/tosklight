//! Terminal family capture cleanup uses real service requests, without new motion or pose work.
use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct EndPorts {
    authorization_calls: AtomicUsize,
    deny: bool,
}
impl ProgrammingPorts for EndPorts {
    fn authorize_programming_change(&self, _: &ActionContext) -> Result<(), crate::ActionError> {
        self.authorization_calls.fetch_add(1, Ordering::Relaxed);
        if self.deny {
            Err(crate::ActionError::new(
                ActionErrorKind::Forbidden,
                "test surface is read only",
            ))
        } else {
            Ok(())
        }
    }
    fn execute(
        &self,
        _: &ProgrammerRegistry,
        _: &ActionContext,
        _: &str,
        _: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("FinishGesture must not execute a command")
    }
    fn values_environment(
        &self,
        _: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, crate::ActionError> {
        panic!("FinishGesture must not read an adoption environment")
    }
    fn prepare_family_edit_context(
        &self,
        _: &ActionContext,
        _: bool,
        _: &ProgrammingValueIntent,
        _: &mut ProgrammingValuesEnvironment,
    ) -> Result<(), crate::ActionError> {
        panic!("FinishGesture must not capture a pose")
    }
    fn persist(&self, _: &ActionContext, _: &'static str) -> Option<String> {
        panic!("FinishGesture must not persist")
    }
    fn reconcile(&self, _: &ActionContext, _: ProgrammingReconciliation) {
        panic!("FinishGesture must not reconcile output")
    }
    fn commit_preload(&self, _: &ActionContext) -> Result<Option<String>, String> {
        panic!("FinishGesture must not commit Preload")
    }
}

fn end(
    desk: &GestureDesk,
    preload: bool,
    context: ActionContext,
    attribute: AttributeKey,
    gesture: &str,
    ports: &EndPorts,
) -> Result<(u64, u64, bool), crate::ActionError> {
    // Cleanup uses exact retained identity rather than the caller's possibly stale revisions.
    let context = context.with_expected_revision(u64::MAX);
    if preload {
        let result = desk.setup.service.handle_preload_values(
            ActionEnvelope {
                context,
                command: ProgrammingPreloadValuesRequest {
                    expected_capture_mode_revision: u64::MAX,
                    command: ProgrammingPreloadValuesCommand::FinishGesture {
                        attribute,
                        undo_group: gesture.into(),
                    },
                },
            },
            ports,
        )?;
        let ProgrammingPreloadValuesOutcome::NoChange { revision } = result.outcome else {
            panic!("End cannot author values")
        };
        assert!(result.interaction_event_sequence.is_none());
        assert!(result.warning.is_none());
        Ok((revision, result.capture_mode_revision, result.replayed))
    } else {
        let result = desk.setup.service.handle_values(
            ActionEnvelope {
                context,
                command: ProgrammingValuesRequest {
                    expected_capture_mode_revision: u64::MAX,
                    command: ProgrammingValuesCommand::FinishGesture {
                        attribute,
                        undo_group: gesture.into(),
                    },
                },
            },
            ports,
        )?;
        let ProgrammingValuesOutcome::NoChange { revision } = result.outcome else {
            panic!("End cannot author values")
        };
        assert!(result.interaction_event_sequence.is_none());
        assert!(result.warning.is_none());
        Ok((revision, result.capture_mode_revision, result.replayed))
    }
}
fn context(desk: &GestureDesk, request: &str) -> ActionContext {
    desk.setup.context.clone().with_request_id(request)
}
fn revision(desk: &GestureDesk) -> u64 {
    if desk.preload {
        desk.setup.registry.preload_values_revision()
    } else {
        desk.setup.registry.normal_values_revision()
    }
}
fn neutral(desk: &GestureDesk, request: &str, gesture: &str) {
    desk.apply(
        request,
        desk.intent(false, ScalarEdit::Relative(0.), gesture),
    )
    .unwrap();
}

#[test]
fn end_neutral_capture_is_quiet_and_a_new_touch_adopts_a_new_pose_in_both_lanes() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        neutral(&desk, "neutral", "first-touch");
        let stamp = desk.setup.registry.value_gesture_stamp().unwrap();
        let depth = desk.depth();
        let value_revision = revision(&desk);
        let events = desk.setup.events.latest_sequence();
        let ports = EndPorts::default();
        let result = end(
            &desk,
            preload,
            context(&desk, "end"),
            ProgrammingOwner::Position.key(),
            "first-touch",
            &ports,
        )
        .unwrap();
        assert_eq!(
            result,
            (
                value_revision,
                desk.setup.registry.capture_mode_revision(),
                false
            )
        );
        assert_ne!(desk.setup.registry.value_gesture_stamp().unwrap(), stamp);
        assert_eq!(desk.depth(), depth);
        assert_eq!(revision(&desk), value_revision);
        assert_eq!(desk.setup.events.latest_sequence(), events);
        desk.pose(fixture, 100., 80.);
        desk.apply(
            "new-turn",
            desk.intent(false, ScalarEdit::Relative(1.), "new-touch"),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(101., 80.));
        assert_eq!(desk.depth(), depth + 1);
        assert_eq!(ports.authorization_calls.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn end_non_neutral_capture_closes_only_undo_group_without_changing_recorded_values() {
    for preload in [false, true] {
        let desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        desk.apply(
            "turn",
            desk.intent(false, ScalarEdit::Relative(1.), "touch"),
        )
        .unwrap();
        let depth = desk.depth();
        let value_revision = revision(&desk);
        let events = desk.setup.events.latest_sequence();
        end(
            &desk,
            preload,
            context(&desk, "end"),
            ProgrammingOwner::Position.key(),
            "touch",
            &EndPorts::default(),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(1., 20.));
        assert_eq!(desk.depth(), depth);
        assert_eq!(revision(&desk), value_revision);
        assert_eq!(desk.setup.events.latest_sequence(), events);
        // No tombstone promises: even a caller reusing its ID gets a newly captured Undo unit.
        desk.apply(
            "next-turn",
            desk.intent(false, ScalarEdit::Relative(2.), "touch"),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(3., 20.));
        assert_eq!(desk.depth(), depth + 1);
    }
}

#[test]
fn foreign_session_desk_lane_attribute_and_old_id_cannot_end_the_active_capture() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        neutral(&desk, "neutral", "current-touch");
        let stamp = desk.setup.registry.value_gesture_stamp();
        let events = desk.setup.events.latest_sequence();
        for index in 0..5 {
            let mut action_context = context(&desk, &format!("wrong-{index}"));
            let mut lane = preload;
            let mut attribute = ProgrammingOwner::Position.key();
            let mut gesture = "current-touch";
            match index {
                0 => action_context.session_id = Some(Uuid::new_v4()),
                1 => action_context.desk_id = Uuid::new_v4(),
                2 => lane = !preload,
                3 => attribute = ProgrammingOwner::Color.key(),
                4 => gesture = "old-touch",
                _ => unreachable!(),
            }
            end(
                &desk,
                lane,
                action_context,
                attribute,
                gesture,
                &EndPorts::default(),
            )
            .unwrap();
            assert_eq!(desk.setup.registry.value_gesture_stamp(), stamp);
        }
        assert_eq!(desk.setup.events.latest_sequence(), events);
        desk.pose(fixture, 100., 80.);
        desk.apply(
            "continuing-turn",
            desk.intent(false, ScalarEdit::Relative(1.), "current-touch"),
        )
        .unwrap();
        assert_eq!(
            desk.value(fixture, false),
            angles(1., 20.),
            "wrong End cannot replace the captured pose"
        );
    }
}

#[test]
fn stale_end_does_not_close_newer_domain_undo_selection_or_mode_ownership() {
    for preload in [false, true] {
        for boundary in ["value", "selection", "mode"] {
            let desk = GestureDesk::new(preload);
            neutral(&desk, "neutral", "touch");
            let old = desk.setup.registry.value_gesture_stamp();
            match boundary {
                "value" => {
                    desk.setup.registry.set(
                        desk.setup.session,
                        desk.setup.fixtures[1],
                        AttributeKey::intensity(),
                        AttributeValue::Normalized(0.7),
                    );
                }
                "selection" => {
                    desk.setup
                        .registry
                        .select(desk.setup.session, [desk.setup.fixtures[1]]);
                }
                "mode" => {
                    desk.setup.registry.set_modes(
                        desk.setup.session,
                        Some(!preload),
                        None,
                        None,
                        None,
                    );
                }
                _ => unreachable!(),
            }
            let newer = desk.setup.registry.value_gesture_stamp();
            assert_ne!(newer, old, "test must establish a real lifecycle boundary");
            let depth = desk.depth();
            let value_revision = revision(&desk);
            let events = desk.setup.events.latest_sequence();
            end(
                &desk,
                preload,
                context(&desk, "late-end"),
                ProgrammingOwner::Position.key(),
                "touch",
                &EndPorts::default(),
            )
            .unwrap();
            assert_eq!(
                desk.setup.registry.value_gesture_stamp(),
                newer,
                "late End must not close newer domain ownership"
            );
            assert_eq!(desk.depth(), depth);
            assert_eq!(revision(&desk), value_revision);
            assert_eq!(desk.setup.events.latest_sequence(), events);
        }
    }
}

#[test]
fn end_replay_authorization_and_request_fingerprint_protect_a_new_capture() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        neutral(&desk, "neutral", "touch");
        let original_stamp = desk.setup.registry.value_gesture_stamp();
        let denied = EndPorts {
            deny: true,
            ..Default::default()
        };
        let error = end(
            &desk,
            preload,
            context(&desk, "denied-end"),
            ProgrammingOwner::Position.key(),
            "touch",
            &denied,
        )
        .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Forbidden);
        assert_eq!(desk.setup.registry.value_gesture_stamp(), original_stamp);
        let ports = EndPorts::default();
        let first = end(
            &desk,
            preload,
            context(&desk, "end"),
            ProgrammingOwner::Position.key(),
            "touch",
            &ports,
        )
        .unwrap();
        desk.pose(fixture, 100., 80.);
        neutral(&desk, "new-neutral", "touch");
        let new_stamp = desk.setup.registry.value_gesture_stamp();
        let replay = end(
            &desk,
            preload,
            context(&desk, "end"),
            ProgrammingOwner::Position.key(),
            "touch",
            &ports,
        )
        .unwrap();
        assert_eq!(replay, (first.0, first.1, true));
        assert_eq!(desk.setup.registry.value_gesture_stamp(), new_stamp);
        let mismatch = end(
            &desk,
            preload,
            context(&desk, "end"),
            ProgrammingOwner::Color.key(),
            "touch",
            &ports,
        )
        .unwrap_err();
        assert_eq!(mismatch.kind, ActionErrorKind::Conflict);
        assert_eq!(desk.setup.registry.value_gesture_stamp(), new_stamp);
        desk.pose(fixture, 200., 160.);
        desk.apply(
            "new-turn",
            desk.intent(false, ScalarEdit::Relative(1.), "touch"),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(101., 80.));
        assert_eq!(
            ports.authorization_calls.load(Ordering::Relaxed),
            3,
            "replay is still authorized"
        );
    }
}
