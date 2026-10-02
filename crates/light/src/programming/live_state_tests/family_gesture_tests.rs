use super::*;
use light_core::programming::*;

struct GestureDesk {
    setup: ValuesSetup,
    preload: bool,
}
impl GestureDesk {
    fn new(preload: bool) -> Self {
        let setup = ValuesSetup::new();
        if preload {
            setup
                .service
                .run_external_interaction(&setup.context, &setup.ports, || {
                    setup.registry.arm_preload(setup.session, true)
                })
                .unwrap();
        }
        let mut desk = Self { setup, preload };
        for fixture in desk.setup.fixtures {
            desk.pose(fixture, 0.0, 20.0);
        }
        desk
    }
    fn pose(&mut self, fixture: FixtureId, pan: f32, tilt: f32) {
        self.setup.ports.environment.current_values.insert(
            (fixture, ProgrammingOwner::Position.key()),
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [1.0, 2.0, 3.0],
            ))),
        );
        self.setup.ports.environment.family_contexts.insert(
            fixture,
            crate::ProgrammingFamilyContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: pan,
                    tilt_degrees: tilt,
                }),
                ..Default::default()
            },
        );
    }
    fn intent(&self, group: bool, operation: ScalarEdit, caller: &str) -> ProgrammingValueIntent {
        ProgrammingValueIntent {
            fixture_ids: if group {
                vec![]
            } else {
                vec![self.setup.fixtures[0]]
            },
            group_id: group.then(|| "front".into()),
            attribute: ProgrammingOwner::Position.key(),
            operation: ProgrammingValueOperation::ComponentEdits(vec![ComponentEdit::Scalar {
                component: ProgrammingComponent::Pan,
                operation,
            }]),
            undo_group: Some(caller.into()),
            timing: Default::default(),
            displayed_source: None,
            color_adoption: Default::default(),
        }
    }
    fn apply(
        &self,
        request: &str,
        intent: ProgrammingValueIntent,
    ) -> Result<(), crate::ActionError> {
        self.apply_with_ports(request, intent, &self.setup.ports)
    }
    fn apply_with_ports(
        &self,
        request: &str,
        intent: ProgrammingValueIntent,
        ports: &dyn ProgrammingPorts,
    ) -> Result<(), crate::ActionError> {
        let registry = &self.setup.registry;
        let capture = registry.capture_mode_revision();
        if self.preload {
            self.setup
                .service
                .handle_preload_values(
                    ActionEnvelope {
                        context: self
                            .setup
                            .context
                            .clone()
                            .with_request_id(request)
                            .with_expected_revision(registry.preload_values_revision()),
                        command: ProgrammingPreloadValuesRequest {
                            expected_capture_mode_revision: capture,
                            command: ProgrammingPreloadValuesCommand::ApplyIntent { intent },
                        },
                    },
                    ports,
                )
                .map(|_| ())
        } else {
            self.setup
                .service
                .handle_values(
                    self.setup.action_with_capture(
                        request,
                        registry.normal_values_revision(),
                        capture,
                        ProgrammingValuesCommand::ApplyIntent { intent },
                    ),
                    ports,
                )
                .map(|_| ())
        }
    }
    fn value(&self, fixture: FixtureId, group: bool) -> AttributeValue {
        let state = self.setup.registry.get(self.setup.session).unwrap();
        if group {
            let groups = if self.preload {
                &state.preload_group_pending
            } else {
                &state.group_values
            };
            let value = &groups["front"][&ProgrammingOwner::Position.key()].value;
            match value {
                AttributeValue::GroupFamily(assignment) => assignment.for_member(fixture).clone(),
                value => value.clone(),
            }
        } else {
            let values = if self.preload {
                &state.preload_pending
            } else {
                state.values.as_ref()
            };
            values
                .iter()
                .find(|v| v.fixture_id == fixture)
                .unwrap()
                .value
                .clone()
        }
    }
    fn depth(&self) -> usize {
        self.setup.registry.undo_depth(self.setup.session).unwrap()
    }
}
fn set(value: f32) -> ScalarEdit {
    ScalarEdit::Set(ScalarIntent::Value(value))
}
fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}

#[path = "family_gesture_capture_tests.rs"]
mod capture_tests;

impl GestureDesk {
    fn align(&self, mode: light_programmer::ProgrammerAlignmentMode) {
        self.setup
            .service
            .set_alignment(&self.setup.context, &self.setup.ports, Some(mode))
            .unwrap();
    }
    fn align_intent(&self, group: bool, delta: f32, caller: &str) -> ProgrammingValueIntent {
        let mut intent = self.intent(group, ScalarEdit::Relative(delta), caller);
        if !group {
            intent.fixture_ids = self.setup.fixtures.to_vec();
        }
        intent
    }
}

#[test]
fn typed_align_uses_pinned_anchors_across_touches_and_reanchors_same_lane_without_values_or_undo() {
    use light_programmer::ProgrammerAlignmentMode::{Left, Right};
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let [a, b, c] = desk.setup.fixtures;
        desk.pose(a, 710.0, -20.0);
        desk.pose(b, -720.0, 30.0);
        desk.pose(c, 1080.0, 40.0);
        desk.setup.registry.select(desk.setup.session, [a, b, c]);
        desk.align(Left);
        desk.apply("align-first", desk.align_intent(false, 90.0, "first-touch"))
            .unwrap();
        assert_eq!(desk.value(b, false), angles(-675.0, 30.0));
        assert_eq!(desk.value(c, false), angles(1170.0, 40.0));
        let before = desk.setup.registry.alignment(desk.setup.session).unwrap();
        let depth = desk.depth();
        let mut empty = desk.align_intent(false, 90.0, "empty");
        empty.fixture_ids.clear();
        desk.apply("align-empty", empty).unwrap();
        assert_eq!(
            desk.setup.registry.alignment(desk.setup.session).unwrap(),
            before
        );
        assert_eq!(desk.depth(), depth);
        desk.pose(c, -900.0, 5.0);
        desk.apply("align-next", desk.align_intent(false, 90.0, "second-touch"))
            .unwrap();
        assert_eq!(desk.value(c, false), angles(1260.0, 40.0));
        let generation = if preload {
            desk.setup
                .registry
                .preload_values_generation(desk.setup.session)
        } else {
            desk.setup
                .registry
                .normal_values_generation(desk.setup.session)
        };
        let depth = desk.depth();
        desk.align(Right);
        assert_eq!(desk.depth(), depth);
        assert_eq!(
            if preload {
                desk.setup
                    .registry
                    .preload_values_generation(desk.setup.session)
            } else {
                desk.setup
                    .registry
                    .normal_values_generation(desk.setup.session)
            },
            generation
        );
        desk.apply("align-right", desk.align_intent(false, 10.0, "third-touch"))
            .unwrap();
        assert_eq!(desk.value(a, false), angles(720.0, -20.0));
        assert_eq!(desk.value(b, false), angles(-625.0, 30.0));
        assert_eq!(desk.value(c, false), angles(1260.0, 40.0));
        desk.setup
            .service
            .run_value_gesture_boundary(&desk.setup.context, || ());
        assert!(desk.setup.registry.alignment(desk.setup.session).is_none());
    }
}

#[test]
fn group_align_keeps_targets_dormant_members_template_and_frozen_spatial_ranks_in_both_lanes() {
    use light_programmer::{FamilyAlignmentInput, ProgrammerAlignmentMode::Left};
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let [a, b, c] = desk.setup.fixtures;
        let added = FixtureId::new();
        desk.setup.ports.environment.fixture_ids.insert(added);
        desk.pose(a, 100.0, 10.0);
        desk.pose(b, 200.0, 20.0);
        desk.pose(c, 710.0, 30.0);
        let target = desk.setup.ports.environment.current_values
            [&(a, ProgrammingOwner::Position.key())]
            .clone();
        desk.setup.registry.select(desk.setup.session, [a, b, c]);
        desk.setup
            .ports
            .environment
            .group_rank_counts
            .insert("front".into(), 2);
        desk.setup
            .ports
            .environment
            .group_ranks
            .insert("front".into(), HashMap::from([(a, 0), (b, 0), (c, 1)]));
        desk.align(Left);
        desk.apply("group-align-first", desk.align_intent(true, 90.0, "one"))
            .unwrap();
        assert_eq!(desk.value(a, true), target);
        assert_eq!(desk.value(b, true), target);
        assert_eq!(desk.value(c, true), angles(800.0, 30.0));
        let assert_group_only = |desk: &GestureDesk| {
            let state = desk.setup.registry.get(desk.setup.session).unwrap();
            assert!(if preload {
                state.preload_pending.is_empty()
            } else {
                state.values.is_empty()
            });
        };
        assert_group_only(&desk);
        // An added identity receives the retained template; absent original members are not
        // evaluated. The current count/ranks deliberately differ from the frozen binding.
        desk.setup
            .ports
            .environment
            .group_members
            .insert("front".into(), vec![added]);
        desk.setup
            .ports
            .environment
            .group_memberships
            .insert("front".into(), 1);
        desk.setup
            .ports
            .environment
            .group_rank_counts
            .insert("front".into(), 1);
        desk.setup
            .ports
            .environment
            .group_ranks
            .insert("front".into(), HashMap::from([(added, 0)]));
        desk.setup.ports.environment.family_contexts.clear();
        let before = desk.setup.registry.alignment(desk.setup.session).unwrap();
        let depth = desk.depth();
        desk.apply("group-align-no-bound", desk.align_intent(true, 90.0, "two"))
            .unwrap();
        assert_eq!(
            desk.setup.registry.alignment(desk.setup.session).unwrap(),
            before
        );
        assert_eq!(desk.depth(), depth);
        assert_eq!(desk.value(added, true), target);
        assert_eq!(desk.value(c, true), angles(800.0, 30.0));
        // Re-added identity resumes from its original context despite spatial movement.
        desk.setup
            .ports
            .environment
            .group_members
            .insert("front".into(), vec![c, added, a, b]);
        desk.setup
            .ports
            .environment
            .group_memberships
            .insert("front".into(), 4);
        desk.setup
            .ports
            .environment
            .group_rank_counts
            .insert("front".into(), 3);
        desk.setup.ports.environment.group_ranks.insert(
            "front".into(),
            HashMap::from([(c, 0), (added, 1), (a, 2), (b, 2)]),
        );
        desk.apply("group-align-resume", desk.align_intent(true, 90.0, "three"))
            .unwrap();
        assert_eq!(desk.value(c, true), angles(890.0, 30.0));
        assert_eq!(desk.value(a, true), target);
        assert_eq!(desk.value(b, true), target);
        assert_eq!(desk.value(added, true), target);
        assert_eq!(
            desk.setup
                .registry
                .alignment(desk.setup.session)
                .unwrap()
                .family_binding
                .unwrap()
                .input,
            FamilyAlignmentInput::Scalar(180.0)
        );
        desk.apply(
            "group-align-reverse",
            desk.align_intent(true, -180.0, "four"),
        )
        .unwrap();
        assert_eq!(desk.value(c, true), angles(710.0, 30.0));
        assert_eq!(desk.value(a, true), target);
        assert_group_only(&desk);
    }
}

#[test]
fn group_align_materializes_nested_curves_before_weighting_and_rejects_missing_ranks_atomically() {
    use light_programmer::ProgrammerAlignmentMode::Left;
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let [a, b, c] = desk.setup.fixtures;
        desk.setup.registry.select(desk.setup.session, [a, b, c]);
        let mut initial = desk.align_intent(true, 0.0, "spread");
        initial.operation = ProgrammingValueOperation::AbsoluteSet(AttributeValue::Position(
            Arc::new(PositionIntent::Angles {
                pan_degrees: ScalarIntent::Spread(vec![-720.0, 720.0]),
                tilt_degrees: ScalarIntent::Value(20.0),
            }),
        ));
        desk.apply("group-curve", initial).unwrap();
        desk.align(Left);
        desk.setup
            .ports
            .environment
            .group_rank_counts
            .insert("front".into(), 2);
        desk.setup
            .ports
            .environment
            .group_ranks
            .insert("front".into(), HashMap::from([(a, 0), (b, 0)]));
        let before = desk.setup.registry.alignment(desk.setup.session);
        let depth = desk.depth();
        assert!(
            desk.apply("group-invalid-ranks", desk.align_intent(true, 90.0, "turn"))
                .is_err()
        );
        assert_eq!(desk.setup.registry.alignment(desk.setup.session), before);
        assert_eq!(desk.depth(), depth);
        desk.setup
            .ports
            .environment
            .group_ranks
            .get_mut("front")
            .unwrap()
            .insert(c, 1);
        desk.apply("group-good-ranks", desk.align_intent(true, 90.0, "turn"))
            .unwrap();
        assert_eq!(desk.value(a, true), angles(-720.0, 20.0));
        assert_eq!(desk.value(b, true), angles(-720.0, 20.0));
        assert_eq!(desk.value(c, true), angles(810.0, 20.0));
    }
}

#[test]
fn neutral_first_sample_pins_pose_without_authoring_and_empty_rejected_replayed_inputs_do_not_replace_it()
 {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let depth = desk.depth();
        let fixture = desk.setup.fixtures[0];
        let zero = desk.intent(false, set(0.0), "touch");
        desk.apply("first-zero", zero.clone()).unwrap();
        assert_eq!(desk.depth(), depth);
        assert!(
            desk.setup
                .registry
                .get(desk.setup.session)
                .unwrap()
                .values
                .is_empty()
        );
        assert!(
            desk.setup
                .registry
                .get(desk.setup.session)
                .unwrap()
                .preload_pending
                .is_empty()
        );
        desk.pose(fixture, 100.0, 80.0);
        let mut empty = zero.clone();
        empty.fixture_ids.clear();
        empty.undo_group = Some("empty".into());
        desk.apply("empty", empty).unwrap();
        assert!(
            desk.apply("invalid", desk.intent(false, set(f32::NAN), "invalid"))
                .is_err()
        );
        desk.apply("first-zero", zero).unwrap(); // Replay must not sample the moving frame.
        desk.apply(
            "move",
            desk.intent(false, ScalarEdit::Relative(1.0), "touch"),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(1.0, 20.0));
        desk.apply(
            "move-again",
            desk.intent(false, ScalarEdit::Relative(2.0), "touch"),
        )
        .unwrap();
        assert_eq!(
            desk.value(fixture, false),
            angles(3.0, 20.0),
            "deltas continue from authored values"
        );
        assert_eq!(desk.depth(), depth + 1);
    }
}

#[test]
fn mixed_group_retains_no_op_member_pose_while_other_members_take_over() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let [a, b, c] = desk.setup.fixtures;
        desk.pose(b, 10.0, 40.0);
        let before = desk.depth();
        desk.apply("group-zero", desk.intent(true, set(0.0), "group-touch"))
            .unwrap();
        assert!(
            matches!(desk.value(a, true), AttributeValue::Position(p) if matches!(p.as_ref(), PositionIntent::Target {..}))
        );
        desk.pose(a, 100.0, 70.0);
        desk.pose(b, 200.0, 90.0);
        desk.pose(c, 300.0, 110.0);
        desk.apply(
            "group-move",
            desk.intent(true, ScalarEdit::Relative(1.0), "group-touch"),
        )
        .unwrap();
        assert_eq!(desk.value(a, true), angles(1.0, 20.0));
        assert_eq!(desk.value(b, true), angles(1.0, 40.0));
        assert_eq!(desk.value(c, true), angles(1.0, 20.0));
        assert_eq!(desk.depth(), before + 1);
    }
}

#[test]
fn record_with_empty_redo_and_new_caller_start_fresh_pose_adoption() {
    for preload in [false, true] {
        for new_caller in [false, true] {
            let mut desk = GestureDesk::new(preload);
            let fixture = desk.setup.fixtures[0];
            desk.apply("zero", desk.intent(false, set(0.0), "touch"))
                .unwrap();
            desk.pose(fixture, 100.0, 80.0);
            if !new_caller {
                assert!(!desk.setup.registry.clear_redo(desk.setup.session));
            }
            desk.apply(
                "move",
                desk.intent(
                    false,
                    ScalarEdit::Relative(1.0),
                    if new_caller { "next-touch" } else { "touch" },
                ),
            )
            .unwrap();
            assert_eq!(desk.value(fixture, false), angles(101.0, 80.0));
        }
    }
}

#[test]
fn changed_order_or_group_membership_never_coalesces_reused_caller_id() {
    for preload in [false, true] {
        for group in [false, true] {
            let mut desk = GestureDesk::new(preload);
            let [a, b, _] = desk.setup.fixtures;
            let before = desk.depth();
            desk.apply(
                "first",
                desk.intent(group, ScalarEdit::Relative(1.0), "touch"),
            )
            .unwrap();
            let mut next = desk.intent(group, ScalarEdit::Relative(1.0), "touch");
            if group {
                desk.setup
                    .ports
                    .environment
                    .group_members
                    .insert("front".into(), vec![b, a]);
                desk.setup
                    .ports
                    .environment
                    .group_memberships
                    .insert("front".into(), 2);
            } else {
                next.fixture_ids = vec![b, a];
            }
            desk.apply("second", next).unwrap();
            assert_eq!(desk.depth(), before + 2);
            assert_eq!(desk.value(a, group), angles(2.0, 20.0));
        }
    }
}

#[test]
fn failed_outer_transaction_restores_original_frozen_context() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        desk.apply("zero", desk.intent(false, set(0.0), "touch"))
            .unwrap();
        let before = desk.depth();
        desk.pose(fixture, 100.0, 80.0);
        let events_before = desk.setup.events.latest_sequence();
        let result: Result<(), &str> =
            desk.setup
                .service
                .with_value_gesture_transaction(desk.setup.session, || {
                    // Public actions cannot persist/publish a result that this domain transaction might
                    // roll back. Their exact request remains usable after the failed outer operation.
                    assert!(
                        desk.apply(
                            "rolled-back",
                            desk.intent(false, ScalarEdit::Relative(1.0), "touch")
                        )
                        .is_err()
                    );
                    desk.setup.registry.set(
                        desk.setup.session,
                        fixture,
                        ProgrammingOwner::Position.key(),
                        angles(110.0, 80.0),
                    );
                    desk.setup.service.forget_value_gesture(None);
                    Err("later command failed")
                });
        assert!(result.is_err());
        assert_eq!(desk.depth(), before);
        assert_eq!(desk.setup.events.latest_sequence(), events_before);
        assert!(desk.setup.ports.persisted.lock().is_empty());
        desk.apply(
            "rolled-back",
            desk.intent(false, ScalarEdit::Relative(1.0), "touch"),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(1.0, 20.0));
    }
}

#[test]
fn committed_show_install_and_owner_disconnect_end_context_but_other_surface_does_not() {
    for boundary in [0, 1, 2] {
        let mut desk = GestureDesk::new(false);
        let fixture = desk.setup.fixtures[0];
        desk.apply("zero", desk.intent(false, set(0.0), "touch"))
            .unwrap();
        desk.pose(fixture, 100.0, 80.0);
        match boundary {
            0 => desk
                .setup
                .service
                .run_value_gesture_boundary(&desk.setup.context, || ()),
            1 => desk
                .setup
                .service
                .forget_value_gesture(Some(desk.setup.session)),
            _ => desk
                .setup
                .service
                .forget_value_gesture(Some(SessionId::new())),
        }
        desk.apply(
            "continue",
            desk.intent(false, ScalarEdit::Relative(1.0), "touch"),
        )
        .unwrap();
        assert_eq!(
            desk.value(fixture, false),
            if boundary == 2 {
                angles(1.0, 20.0)
            } else {
                angles(101.0, 80.0)
            }
        );
    }
}

#[test]
fn releasing_gesture_drops_retained_source_models() {
    let mut desk = GestureDesk::new(false);
    let fixture = desk.setup.fixtures[0];
    let model = Arc::new(VirtualColorAuthoringV1);
    let weak = Arc::downgrade(&model);
    desk.setup
        .ports
        .environment
        .family_contexts
        .get_mut(&fixture)
        .unwrap()
        .color_model = Some(model);
    desk.apply("zero", desk.intent(false, set(0.0), "touch"))
        .unwrap();
    desk.setup.ports.environment.family_contexts.clear();
    assert!(weak.upgrade().is_some());
    desk.setup
        .service
        .forget_value_gesture(Some(SessionId::new()));
    assert!(weak.upgrade().is_some());
    desk.setup
        .service
        .forget_value_gesture(Some(desk.setup.session));
    assert!(weak.upgrade().is_none());
}

#[test]
fn empty_clear_ends_a_neutral_touch_without_history_or_value_revision() {
    let mut desk = GestureDesk::new(false);
    let fixture = desk.setup.fixtures[0];
    desk.apply("zero", desk.intent(false, set(0.0), "touch"))
        .unwrap();
    let depth = desk.depth();
    desk.setup
        .handle("clear", 0, ProgrammingValuesCommand::Clear);
    assert_eq!(desk.setup.registry.normal_values_revision(), 0);
    assert_eq!(desk.depth(), depth);
    desk.pose(fixture, 100.0, 80.0);
    desk.apply(
        "after-clear",
        desk.intent(false, ScalarEdit::Relative(1.0), "touch"),
    )
    .unwrap();
    assert_eq!(desk.value(fixture, false), angles(101.0, 80.0));
}

#[path = "family_adoption_hold_tests.rs"]
mod family_adoption_hold_tests;

#[path = "family_gesture_end_tests.rs"]
mod family_gesture_end_tests;

#[path = "family_displayed_source_tests.rs"]
mod family_displayed_source_tests;

#[path = "color_adoption_tests.rs"]
mod color_adoption_tests;
