use super::*;

struct CohortCapturePorts {
    environment: ProgrammingValuesEnvironment,
    poses: Mutex<HashMap<FixtureId, crate::ProgrammingFamilyContext>>,
    calls: Mutex<Vec<(bool, Vec<FixtureId>)>>,
}

impl ProgrammingPorts for CohortCapturePorts {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("cohort capture does not execute commands")
    }

    fn values_environment(
        &self,
        _context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, crate::ActionError> {
        let mut environment = self.environment.clone();
        environment.family_contexts.clear();
        Ok(environment)
    }

    fn prepare_family_edit_context(
        &self,
        _context: &ActionContext,
        preload: bool,
        intent: &ProgrammingValueIntent,
        environment: &mut ProgrammingValuesEnvironment,
    ) -> Result<(), crate::ActionError> {
        let members = intent.group_id.as_ref().map_or_else(
            || intent.fixture_ids.clone(),
            |group| environment.group_members[group].clone(),
        );
        self.calls.lock().push((preload, members.clone()));
        let poses = self.poses.lock();
        for fixture in &members {
            let mut context = poses.get(fixture).cloned().unwrap_or_default();
            context.position_adoption_attempted = true;
            if let Some(pose) = context.solved_angles {
                environment
                    .current_values
                    .entry((*fixture, ProgrammingOwner::Position.key()))
                    .or_insert_with(|| angles(pose.pan_degrees, pose.tilt_degrees));
            }
            environment.family_contexts.insert(*fixture, context);
        }
        Ok(())
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[test]
fn native_only_pan_tilt_align_captures_both_owners_and_skips_unsupported_selection() {
    use light_programmer::ProgrammerAlignmentMode::Left;
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let [a, b, c] = desk.setup.fixtures;
        desk.pose(a, 100.0, 20.0);
        desk.pose(b, 200.0, 30.0);
        let poses = desk.setup.ports.environment.family_contexts.clone();
        desk.setup.ports.environment.current_values.clear();
        desk.setup.ports.environment.default_values.clear();
        desk.setup.ports.environment.family_contexts.clear();
        for fixture in [a, b] {
            desk.setup.ports.environment.supported_attributes.insert(
                fixture,
                HashSet::from([AttributeKey("pan".into()), AttributeKey("tilt".into())]),
            );
        }
        desk.setup
            .ports
            .environment
            .supported_attributes
            .insert(c, HashSet::from([AttributeKey::intensity()]));
        desk.setup.registry.select(desk.setup.session, [a, b, c]);
        desk.align(Left);
        let ports = CohortCapturePorts {
            environment: desk.setup.ports.environment.clone(),
            poses: Mutex::new(poses),
            calls: Mutex::new(Vec::new()),
        };
        let intent = desk.intent(false, ScalarEdit::Relative(90.0), "native-only-touch");
        assert_eq!(intent.fixture_ids, vec![a]);
        desk.apply_with_ports("native-only-first", intent, &ports)
            .unwrap();
        assert_eq!(*ports.calls.lock(), vec![(preload, vec![a, b])]);
        let alignment = desk.setup.registry.alignment(desk.setup.session).unwrap();
        let binding = alignment.family_binding.unwrap();
        assert_eq!(
            binding
                .bases
                .iter()
                .map(|base| base.fixture_id)
                .collect::<Vec<_>>(),
            vec![a, b]
        );
        assert_eq!(desk.value(a, false), angles(100.0, 20.0));
        assert_eq!(desk.value(b, false), angles(290.0, 30.0));
        let state = desk.setup.registry.get(desk.setup.session).unwrap();
        let values = if preload {
            &state.preload_pending
        } else {
            state.values.as_ref()
        };
        assert!(values.iter().all(|value| value.fixture_id != c));
    }
}

#[test]
fn attempted_missing_native_seed_holds_fixture_group_and_subset_align_without_inventing_angles() {
    use light_programmer::ProgrammerAlignmentMode::Left;
    for preload in [false, true] {
        for group in [false, true] {
            for aligned in [false, true] {
                let mut desk = GestureDesk::new(preload);
                let [a, b, c] = desk.setup.fixtures;
                let position = ProgrammingOwner::Position.key();
                desk.setup
                    .ports
                    .environment
                    .current_values
                    .remove(&(b, position.clone()));
                desk.setup.ports.environment.family_contexts.remove(&b);
                desk.setup
                    .ports
                    .environment
                    .supported_attributes
                    .entry(b)
                    .or_default()
                    .insert(position);
                if aligned {
                    desk.setup.registry.select(desk.setup.session, [a, b, c]);
                    desk.align(Left);
                }
                let alignment = desk.setup.registry.alignment(desk.setup.session);
                let depth = desk.depth();
                let before = desk.setup.registry.get(desk.setup.session).unwrap();
                let revision = if preload {
                    desk.setup.registry.preload_values_revision()
                } else {
                    desk.setup.registry.normal_values_revision()
                };
                let ports = CohortCapturePorts {
                    environment: desk.setup.ports.environment.clone(),
                    poses: Mutex::new(desk.setup.ports.environment.family_contexts.clone()),
                    calls: Mutex::new(Vec::new()),
                };
                let mut intent = desk.intent(group, ScalarEdit::Relative(10.0), "native-touch");
                if !group && !aligned {
                    intent.fixture_ids = vec![a, b];
                }
                desk.apply_with_ports("native-hold", intent.clone(), &ports)
                    .unwrap();
                let after = desk.setup.registry.get(desk.setup.session).unwrap();
                assert_eq!(after.values, before.values);
                assert_eq!(after.group_values, before.group_values);
                assert_eq!(after.preload_pending, before.preload_pending);
                assert_eq!(after.preload_group_pending, before.preload_group_pending);
                assert_eq!(desk.depth(), depth);
                assert_eq!(desk.setup.registry.alignment(desk.setup.session), alignment);
                assert_eq!(
                    if preload {
                        desk.setup.registry.preload_values_revision()
                    } else {
                        desk.setup.registry.normal_values_revision()
                    },
                    revision
                );

                ports.poses.lock().insert(
                    b,
                    crate::ProgrammingFamilyContext {
                        solved_angles: Some(JointAngles {
                            pan_degrees: 200.0,
                            tilt_degrees: 30.0,
                        }),
                        ..Default::default()
                    },
                );
                desk.apply_with_ports("native-pinned", intent.clone(), &ports)
                    .unwrap();
                assert_eq!(ports.calls.lock().len(), 1);
                assert_eq!(desk.depth(), depth);
                intent.undo_group = Some("native-fresh".into());
                desk.apply_with_ports("native-recovered", intent, &ports)
                    .unwrap();
                assert_eq!(ports.calls.lock().len(), 2);
                assert_eq!(
                    desk.value(b, group),
                    angles(if aligned { 205.0 } else { 210.0 }, 30.0)
                );
            }
        }
    }
}

#[test]
fn missing_seed_without_attempted_capture_retains_validation_errors() {
    for preload in [false, true] {
        for group in [false, true] {
            let mut desk = GestureDesk::new(preload);
            let fixture = desk.setup.fixtures[0];
            desk.setup
                .ports
                .environment
                .current_values
                .remove(&(fixture, ProgrammingOwner::Position.key()));
            // Other Group members must have valid authored Angles: a Target without a pose
            // correctly holds the whole operation before this deliberately malformed seed.
            for other in &desk.setup.fixtures[1..] {
                desk.setup
                    .ports
                    .environment
                    .current_values
                    .insert((*other, ProgrammingOwner::Position.key()), angles(0., 20.));
            }
            desk.setup.ports.environment.family_contexts.clear();
            let depth = desk.depth();
            assert!(
                desk.apply(
                    "uncaptured-missing",
                    desk.intent(group, ScalarEdit::Relative(1.0), "touch")
                )
                .is_err()
            );
            assert_eq!(desk.depth(), depth);
        }
    }
}

#[test]
fn subset_align_captures_actual_cohort_and_pins_absence_until_a_fresh_touch_in_both_lanes() {
    use light_programmer::ProgrammerAlignmentMode::Left;
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let [a, b, c] = desk.setup.fixtures;
        desk.pose(a, 100.0, 20.0);
        desk.pose(b, 200.0, 30.0);
        desk.pose(c, 300.0, 40.0);
        desk.setup.registry.select(desk.setup.session, [a, b, c]);
        desk.align(Left);
        let mut poses = desk.setup.ports.environment.family_contexts.clone();
        let b_pose = poses.remove(&b).unwrap();
        let ports = CohortCapturePorts {
            environment: desk.setup.ports.environment.clone(),
            poses: Mutex::new(poses),
            calls: Mutex::new(Vec::new()),
        };
        let before = desk.setup.registry.get(desk.setup.session).unwrap();
        let alignment = desk.setup.registry.alignment(desk.setup.session).unwrap();
        let depth = desk.depth();
        let revision = if preload {
            desk.setup.registry.preload_values_revision()
        } else {
            desk.setup.registry.normal_values_revision()
        };
        let intent = desk.intent(false, ScalarEdit::Relative(90.0), "subset-touch");
        assert_eq!(intent.fixture_ids, vec![a]);
        desk.apply_with_ports("subset-hold", intent.clone(), &ports)
            .unwrap();
        assert_eq!(*ports.calls.lock(), vec![(preload, vec![a, b, c])]);
        assert_eq!(
            desk.setup.registry.alignment(desk.setup.session).unwrap(),
            alignment
        );
        let after = desk.setup.registry.get(desk.setup.session).unwrap();
        assert_eq!(after.values, before.values);
        assert_eq!(after.preload_pending, before.preload_pending);
        assert_eq!(desk.depth(), depth);
        assert_eq!(
            if preload {
                desk.setup.registry.preload_values_revision()
            } else {
                desk.setup.registry.normal_values_revision()
            },
            revision
        );

        ports.poses.lock().insert(b, b_pose);
        desk.apply_with_ports("subset-still-holds", intent.clone(), &ports)
            .unwrap();
        assert_eq!(ports.calls.lock().len(), 1);
        assert_eq!(desk.depth(), depth);
        assert_eq!(
            desk.setup.registry.alignment(desk.setup.session).unwrap(),
            alignment
        );

        let mut fresh = intent;
        fresh.undo_group = Some("fresh-subset-touch".into());
        desk.apply_with_ports("subset-fresh", fresh.clone(), &ports)
            .unwrap();
        assert_eq!(desk.value(b, false), angles(245.0, 30.0));
        assert_eq!(desk.value(c, false), angles(390.0, 40.0));
        assert_eq!(desk.depth(), depth + 1);
        fresh.operation = ProgrammingValueOperation::ComponentEdits(vec![ComponentEdit::Scalar {
            component: ProgrammingComponent::Pan,
            operation: ScalarEdit::Relative(10.0),
        }]);
        desk.apply_with_ports("subset-bound", fresh, &ports)
            .unwrap();
        assert_eq!(
            ports.calls.lock().len(),
            2,
            "binding does not restart the touch"
        );
        assert_eq!(desk.value(b, false), angles(250.0, 30.0));
        assert_eq!(desk.value(c, false), angles(400.0, 40.0));
        assert_eq!(desk.depth(), depth + 1);
    }
}

#[test]
fn missing_pose_holds_the_whole_fixture_or_group_selection_without_error_revision_or_undo() {
    for preload in [false, true] {
        for group in [false, true] {
            let mut desk = GestureDesk::new(preload);
            let missing = desk.setup.fixtures[1];
            desk.setup
                .ports
                .environment
                .family_contexts
                .remove(&missing);
            let before = desk.setup.registry.get(desk.setup.session).unwrap();
            let depth = desk.depth();
            let revision = if preload {
                desk.setup.registry.preload_values_revision()
            } else {
                desk.setup.registry.normal_values_revision()
            };
            let mut intent = desk.intent(group, ScalarEdit::Relative(10.), "hold");
            if !group {
                intent.fixture_ids = desk.setup.fixtures[..2].to_vec();
            }
            desk.apply("hold-first", intent.clone()).unwrap();
            assert_eq!(desk.depth(), depth);
            assert_eq!(
                if preload {
                    desk.setup.registry.preload_values_revision()
                } else {
                    desk.setup.registry.normal_values_revision()
                },
                revision
            );
            let after = desk.setup.registry.get(desk.setup.session).unwrap();
            assert_eq!(after.values, before.values);
            assert_eq!(after.group_values, before.group_values);
            assert_eq!(after.preload_pending, before.preload_pending);
            assert_eq!(after.preload_group_pending, before.preload_group_pending);
            // A successful passive first touch pins absence. A later pose must not change it.
            desk.pose(missing, 450., 30.);
            desk.apply("hold-later", intent.clone()).unwrap();
            assert_eq!(desk.depth(), depth);
            intent.undo_group = Some("new-touch".into());
            desk.apply("new-pose", intent).unwrap();
            assert_eq!(desk.value(missing, group), angles(460., 30.));
        }
    }
}

#[test]
fn missing_initial_align_pose_does_not_take_ownership_or_advance_an_anchor() {
    use light_programmer::ProgrammerAlignmentMode::Left;
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        desk.setup.ports.environment.family_contexts.clear();
        desk.setup
            .registry
            .select(desk.setup.session, desk.setup.fixtures);
        desk.align(Left);
        let before = desk.setup.registry.alignment(desk.setup.session).unwrap();
        let depth = desk.depth();
        desk.apply("aligned-hold", desk.align_intent(false, 90., "hold"))
            .unwrap();
        assert_eq!(desk.depth(), depth);
        assert_eq!(
            desk.setup.registry.alignment(desk.setup.session).unwrap(),
            before
        );
    }
}

#[test]
fn authored_angles_do_not_require_a_destination_pose() {
    for preload in [false, true] {
        let mut desk = GestureDesk::new(preload);
        let fixture = desk.setup.fixtures[0];
        desk.setup.ports.environment.current_values.insert(
            (fixture, ProgrammingOwner::Position.key()),
            angles(720., -90.),
        );
        desk.setup.ports.environment.family_contexts.clear();
        desk.setup.ports.environment.family_contexts.insert(
            fixture,
            crate::ProgrammingFamilyContext {
                position_adoption_attempted: true,
                ..Default::default()
            },
        );
        desk.apply(
            "authored-edit",
            desk.intent(false, ScalarEdit::Relative(1.), "touch"),
        )
        .unwrap();
        assert_eq!(desk.value(fixture, false), angles(721., -90.));
    }
}
