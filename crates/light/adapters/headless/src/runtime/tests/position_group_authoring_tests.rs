//! Real published fitted poses seed one live Group owner with calibrated member exceptions.
use super::*;
use light_core::programming::GroupFamilyAssignment;
use light_programmer::GroupDefinition;

struct GroupPositionDesk {
    desk: PositionDesk,
    members: [FixtureId; 2],
    group: String,
}
impl GroupPositionDesk {
    async fn new() -> Self {
        let desk = PositionDesk::new().await;
        let second = FixtureId::new();
        let snapshot = desk.scenario.state.output.snapshot();
        let profile = physical::moving_head();
        let mut fixture = physical::patched(&profile, second, 1);
        fixture.location = FixtureLocation {
            x: -3000,
            y: 1000,
            z: 4500,
        };
        fixture.invert_tilt = true;
        fixture.position_calibration = Some(InstalledPositionCalibration {
            pan_zero_degrees: -29.,
            tilt_zero_degrees: 23.,
            ..Default::default()
        });
        fixture.fixture_number = Some(
            snapshot
                .fixtures
                .iter()
                .filter_map(|fixture| fixture.fixture_number)
                .max()
                .unwrap_or(0)
                + 1,
        );
        fixture.universe = Some(
            snapshot
                .fixtures
                .iter()
                .filter_map(|fixture| fixture.universe)
                .max()
                .unwrap_or(0)
                + 1,
        );
        let members = [desk.owner, second];
        let group = "tl556-physical-position".to_owned();
        assert!(snapshot.groups.iter().all(|existing| existing.id != group));
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures.push(fixture);
        let mut groups = snapshot.groups.as_ref().clone();
        groups.push(GroupDefinition {
            id: group.clone(),
            name: "Calibrated physical Position group".into(),
            fixtures: members.to_vec(),
            ..Default::default()
        });
        desk.scenario
            .state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                groups: groups.into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        Self {
            desk,
            members,
            group,
        }
    }
    fn body(
        &self,
        request: &str,
        operation: serde_json::Value,
        gesture: Option<&str>,
    ) -> serde_json::Value {
        let mut body = self.desk.body(request, operation, gesture);
        body["action"]["fixture_ids"] = serde_json::json!([]);
        body["action"]["group_id"] = serde_json::json!(self.group);
        body
    }
    fn turn(
        &self,
        request: &str,
        component: &str,
        amount: f32,
        gesture: &str,
    ) -> serde_json::Value {
        self.body(
            request,
            serde_json::json!({"type":"component_edits", "edits":[{
                "kind":"scalar", "component":{"kind":component},
                "operation":{"kind":"relative", "value":amount}
            }]}),
            Some(gesture),
        )
    }
    async fn target(&self) {
        let request = self.body("group-target", serde_json::json!({"type":"absolute_set", "value":{
            "kind":"position", "value":{"kind":"target",
                "reference":{"kind":"point", "point_id":self.desk.point.0},
                "offset_metres":[{"kind":"value","value":0.},{"kind":"value","value":0.},{"kind":"value","value":0.}]}
        }}), None);
        assert_eq!(self.desk.http(request).await["status"], "changed");
        assert_eq!(self.stored(), self.desk.original);
    }
    fn stored(&self) -> AttributeValue {
        let state = self
            .desk
            .scenario
            .state
            .programming
            .get(self.desk.scenario.session.id)
            .unwrap();
        assert!(
            state
                .values
                .iter()
                .all(|row| !self.members.contains(&row.fixture_id)),
            "Group authoring must not materialize orphan fixture rows"
        );
        assert!(state.preload_pending.is_empty());
        let values = &state.group_values[&self.group];
        assert_eq!(values.len(), 1, "one family owns all Position components");
        values[&ProgrammingOwner::Position.key()].value.clone()
    }
    fn assert_members(&self, poses: [JointAngles; 2], pan_delta: f32, tilt_delta: f32) {
        let AttributeValue::GroupFamily(assignment) = self.stored() else {
            panic!("distinct calibrated bases require complete member exceptions")
        };
        let GroupFamilyAssignment {
            owner,
            template,
            members,
        } = assignment.as_ref();
        assert_eq!(*owner, ProgrammingOwner::Position);
        // Future Group members have no mounting pose; their declarative Angle template starts
        // at zero, while current members retain their own accepted calibrated joint pairs.
        assert_eq!(*template, physical::angles(pan_delta, tilt_delta));
        assert_eq!(members.len(), 2);
        for (member, pose) in self.members.into_iter().zip(poses) {
            assert_eq!(
                assignment.for_member(member),
                &physical::angles(pose.pan_degrees + pan_delta, pose.tilt_degrees + tilt_delta)
            );
        }
    }
    fn publish(&mut self, point_x: f32) -> [JointAngles; 2] {
        self.desk.clock.advance_millis(25);
        let engine = self.desk.scenario.state.output.engine();
        let capture = engine.prepare_output_frame(Default::default());
        let batches = [ContributionBatch::new(
            [
                ("point.position.x", point_x),
                ("point.position.y", 0.5),
                ("point.position.z", 0.5),
                ("point.rotation.x", 0.5),
                ("point.rotation.y", 0.5),
                ("point.rotation.z", 0.5),
            ]
            .into_iter()
            .map(|(attribute, value)| {
                ContributionSample::independent(TimedValue {
                    fixture_id: self.desk.point,
                    attribute: AttributeKey(attribute.into()),
                    value: AttributeValue::Normalized(value),
                    priority: 100,
                    changed_at: capture.sampled_at(),
                    programmer_order: 0,
                    merge_mode: MergeMode::Ltp,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            })
            .collect::<Vec<_>>(),
        )];
        let completed = physical::prepare_live_engine(
            engine,
            self.desk.owner,
            &capture,
            &capture,
            &self.desk.lane,
            &mut self.desk.runtime,
            &mut self.desk.origins,
            &mut self.desk.scratch,
            &batches,
        )
        .unwrap();
        assert!(completed.requirements.is_empty());
        assert_eq!(
            completed.results.len(),
            2,
            "both Group members fit in the same captured frame"
        );
        for row in &completed.results {
            assert!(!row.quality.held);
            assert!(!row.achieved.outcomes.is_empty());
            assert!(
                row.achieved.outcomes.iter().all(
                    |outcome| outcome.result.status == light_fixture::PositionFitStatus::Fitted
                )
            );
            for write in &row.writes {
                let instance = completed
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == write.slot.destination.0)
                    .unwrap();
                assert_eq!(
                    instance.native_raw[write.slot.channel_index as usize],
                    write.raw
                );
            }
        }
        let poses = self.members.map(|member| {
            engine
                .position_angles_from_physical(
                    completed.rendered.generation,
                    &completed.rendered.physical,
                    member,
                )
                .unwrap()
        });
        assert!((poses[0].pan_degrees - poses[1].pan_degrees).abs() > 1.);
        assert!((poses[0].tilt_degrees - poses[1].tilt_degrees).abs() > 1.);
        let frame = RenderedSemanticFrame::untraced(completed.rendered, Default::default());
        self.desk.scenario.state.output.render_frames_and_publish(
            &frame,
            VisualizationScope {
                show_id: self
                    .desk
                    .scenario
                    .state
                    .active_show
                    .current()
                    .map(|show| show.id.0),
            },
        );
        let published = self
            .desk
            .scenario
            .state
            .output
            .latest_visualization_frame()
            .unwrap();
        assert_eq!(published.generation, frame.rendered.generation);
        for member in self.members {
            assert_eq!(
                published
                    .values
                    .value(member, &ProgrammingOwner::Position.key()),
                Some(&self.desk.original),
                "fitting cannot replace the requested Group Target with physical joints"
            );
        }
        poses
    }
}

#[tokio::test]
async fn live_group_pan_edit_adopts_each_actual_calibrated_pose_and_retains_one_group_owner() {
    let mut group = GroupPositionDesk::new().await;
    group.target().await;
    let poses = group.publish(0.5);
    let depth = group.desk.depth();
    let request = group.turn("group-pan", "pan", 5., "group-pan-touch");
    assert_eq!(group.desk.http(request.clone()).await["status"], "changed");
    group.assert_members(poses, 5., 0.);
    assert_eq!(group.desk.depth(), depth + 1);
    let revision = group.desk.revision();
    assert_eq!(group.desk.http(request).await["replayed"], true);
    group.assert_members(poses, 5., 0.);
    assert_eq!(group.desk.revision(), revision);
    assert_eq!(group.desk.depth(), depth + 1);
    group.desk.undo("group-pan-undo").await;
    assert_eq!(group.stored(), group.desk.original);
    let _ = std::fs::remove_dir_all(&group.desk.scenario.data_dir);
}

#[tokio::test]
async fn neutral_group_touch_pins_both_fitted_pairs_across_point_motion_and_fresh_touch_recaptures()
{
    let mut group = GroupPositionDesk::new().await;
    group.target().await;
    let first = group.publish(0.5);
    let depth = group.desk.depth();
    let revision = group.desk.revision();
    let neutral = group.turn("group-neutral", "pan", 0., "group-retained-touch");
    assert_eq!(group.desk.http(neutral).await["status"], "no_change");
    assert_eq!(group.stored(), group.desk.original);
    assert_eq!(group.desk.depth(), depth);
    assert_eq!(group.desk.revision(), revision);
    let state = serde_json::to_value(
        group
            .desk
            .scenario
            .state
            .programming
            .get(group.desk.scenario.session.id)
            .unwrap(),
    )
    .unwrap();
    let second = group.publish(0.55);
    assert_eq!(
        serde_json::to_value(
            group
                .desk
                .scenario
                .state
                .programming
                .get(group.desk.scenario.session.id)
                .unwrap()
        )
        .unwrap(),
        state
    );
    assert_eq!(group.desk.revision(), revision);
    assert_eq!(group.desk.depth(), depth);
    for (first, second) in first.into_iter().zip(second) {
        assert!(
            (first.pan_degrees - second.pan_degrees).abs() > 1.,
            "moving the real Point must change each member's physical adoption base"
        );
    }
    let turn = group.turn("group-retained-tilt", "tilt", 4., "group-retained-touch");
    assert_eq!(group.desk.http(turn).await["status"], "changed");
    group.assert_members(first, 0., 4.);
    assert_eq!(group.desk.depth(), depth + 1);
    group.desk.undo("group-retained-undo").await;
    assert_eq!(group.stored(), group.desk.original);
    let turn = group.turn("group-fresh-tilt", "tilt", 4., "group-fresh-touch");
    assert_eq!(group.desk.http(turn).await["status"], "changed");
    group.assert_members(second, 0., 4.);
    let _ = std::fs::remove_dir_all(&group.desk.scenario.data_dir);
}
