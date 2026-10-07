//! Canonical Normal authoring consumes an actually fitted and published physical Position.
//! These synthetic calibrated profiles verify command semantics, not measured lamp accuracy.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use crate::runtime::output_scheduler::{
    PhysicalAdapterLane, PositionAdapter, position_test_support as physical,
};
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use light_core::programming::{
    JointAngles, PROGRAMMING_CONTRACT_VERSION, ProgrammingOwner, TargetReference,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_dynamics::DynamicRuntime;
use light_engine::{ContributionBatch, ContributionSample};
use light_fixture::{FixtureLocation, InstalledPositionCalibration};
use light_wire::v2::visualization::VisualizationScope;

struct PositionDesk {
    scenario: CommandHttpScenario,
    clock: Arc<ManualClock>,
    owner: FixtureId,
    point: FixtureId,
    original: AttributeValue,
    lane: PhysicalAdapterLane<PositionAdapter>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    scratch: physical::AuthoringScratch,
}
impl PositionDesk {
    async fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let scenario = CommandHttpScenario::with_clock(clock.clone()).await;
        let response = scenario
            .app
            .clone()
            .oneshot(open_default_show_request(&scenario.token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let profile = physical::moving_head();
        let owner = FixtureId::new();
        let point = FixtureId::new();
        let mut fixture = physical::patched(&profile, owner, 1);
        fixture.location.z = 3000;
        fixture.invert_pan = true;
        fixture.position_calibration = Some(InstalledPositionCalibration {
            pan_zero_degrees: 17.,
            tilt_zero_degrees: -11.,
            ..Default::default()
        });
        let aim = physical::point(
            point,
            FixtureLocation {
                x: 2000,
                y: 6000,
                z: 1000,
            },
        );
        let snapshot = scenario.state.output.snapshot();
        // Preserve the opened show's fixture/group/playback references. Add the physical
        // authoring rig in an unused universe rather than replacing the seeded show graph.
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
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures.extend([fixture, aim]);
        scenario
            .state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        let original = physical::target(TargetReference::Point { point_id: point.0 }, [0.; 3]);
        Self {
            scenario,
            clock,
            owner,
            point,
            original,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            runtime: DynamicRuntime::with_programming_contract_support(
                PROGRAMMING_CONTRACT_VERSION,
            ),
            origins: Default::default(),
            scratch: Default::default(),
        }
    }
    fn revision(&self) -> u64 {
        self.scenario.state.programming.normal_values_revision()
    }
    fn depth(&self) -> usize {
        self.scenario
            .state
            .programming
            .undo_depth(self.scenario.session.id)
            .unwrap()
    }
    fn body(
        &self,
        id: &str,
        operation: serde_json::Value,
        gesture: Option<&str>,
    ) -> serde_json::Value {
        serde_json::json!({
            "request_id":id, "expected_revision":self.revision(),
            "expected_capture_mode_revision":self.scenario.state.programming.capture_mode_revision(),
            "action":{"type":"apply_intent", "fixture_ids":[self.owner.0],
                "attribute":"position", "operation":operation,
                "undo_group":gesture, "timing":{"fade":false}}
        })
    }
    fn turn(&self, id: &str, amount: f32, gesture: &str) -> serde_json::Value {
        self.body(
            id,
            serde_json::json!({"type":"component_edits", "edits":[{
                "kind":"scalar", "component":{"kind":"pan"},
                "operation":{"kind":"relative", "value":amount}
            }]}),
            Some(gesture),
        )
    }
    async fn http(&self, body: serde_json::Value) -> serde_json::Value {
        let response = self.scenario.values_action(body).await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
        body
    }
    async fn target(&self, id: &str) {
        let operation = serde_json::json!({"type":"absolute_set", "value":{
            "kind":"position", "value":{"kind":"target",
                "reference":{"kind":"point", "point_id":self.point.0},
                "offset_metres":[{"kind":"value","value":0.},{"kind":"value","value":0.},{"kind":"value","value":0.}]}
        }});
        let result = self.http(self.body(id, operation, None)).await;
        assert_eq!(result["status"], "changed");
        assert_eq!(self.stored(), self.original);
    }
    fn stored(&self) -> AttributeValue {
        let programmer = self
            .scenario
            .state
            .programming
            .get(self.scenario.session.id)
            .unwrap();
        let values = programmer
            .values
            .iter()
            .filter(|value| value.fixture_id == self.owner)
            .collect::<Vec<_>>();
        assert_eq!(
            values.len(),
            1,
            "Position owns one complete family; no independent Pan/Tilt rows"
        );
        assert_eq!(values[0].attribute, ProgrammingOwner::Position.key());
        assert!(programmer.preload_pending.is_empty());
        values[0].value.clone()
    }
    fn publish(&mut self, point_x: f32) -> JointAngles {
        self.clock.advance_millis(25);
        let engine = self.scenario.state.output.engine();
        let capture = engine.prepare_output_frame(Default::default());
        // Real frame inputs move the Point without changing the Programmer's gesture stamp.
        let mut samples = Vec::new();
        for (attribute, value) in [
            ("point.position.x", point_x),
            ("point.position.y", 0.5),
            ("point.position.z", 0.5),
            ("point.rotation.x", 0.5),
            ("point.rotation.y", 0.5),
            ("point.rotation.z", 0.5),
        ] {
            samples.push(ContributionSample::independent(TimedValue {
                fixture_id: self.point,
                attribute: AttributeKey(attribute.into()),
                value: AttributeValue::Normalized(value),
                priority: 100,
                changed_at: capture.sampled_at(),
                programmer_order: 0,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            }));
        }
        let batches = [ContributionBatch::new(samples)];
        let completed = physical::prepare_live_engine(
            engine,
            self.owner,
            &capture,
            &capture,
            &self.lane,
            &mut self.runtime,
            &mut self.origins,
            &mut self.scratch,
            &batches,
        )
        .unwrap();
        assert!(completed.requirements.is_empty());
        assert_eq!(completed.results.len(), 1);
        let row = &completed.results[0];
        assert!(!row.quality.held);
        assert!(
            row.achieved
                .outcomes
                .iter()
                .all(|outcome| outcome.result.status == light_fixture::PositionFitStatus::Fitted)
        );
        let physical = &completed.rendered.physical;
        for write in &row.writes {
            let instance = physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == write.slot.destination.0)
                .unwrap();
            assert_eq!(
                instance.native_raw[write.slot.channel_index as usize],
                write.raw
            );
        }
        let pose = engine
            .position_angles_from_physical(completed.rendered.generation, physical, self.owner)
            .unwrap();
        let frame = RenderedSemanticFrame::untraced(completed.rendered, Default::default());
        self.scenario.state.output.render_frames_and_publish(
            &frame,
            VisualizationScope {
                show_id: self
                    .scenario
                    .state
                    .active_show
                    .current()
                    .map(|show| show.id.0),
            },
        );
        let published = self
            .scenario
            .state
            .output
            .latest_visualization_frame()
            .unwrap();
        assert_eq!(published.generation, frame.rendered.generation);
        assert_eq!(
            published
                .values
                .value(self.owner, &ProgrammingOwner::Position.key()),
            Some(&self.stored())
        );
        pose
    }
    async fn undo(&self, id: &str) {
        let response = self
            .scenario
            .press_key(&self.scenario.token, "UND", id)
            .await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{body:?}");
    }
}

#[tokio::test]
async fn canonical_pan_edit_adopts_actual_calibrated_fitted_pose_and_undo_restores_target() {
    let mut desk = PositionDesk::new().await;
    desk.target("author-target").await;
    let pose = desk.publish(0.5);
    assert_eq!(
        desk.stored(),
        desk.original,
        "fitting cannot rewrite requested Target"
    );
    let depth = desk.depth();
    let request = desk.turn("first-pan", 5., "pan-touch");
    let response = desk.http(request.clone()).await;
    assert_eq!(response["status"], "changed");
    assert_eq!(
        desk.stored(),
        physical::angles(pose.pan_degrees + 5., pose.tilt_degrees)
    );
    assert_eq!(desk.depth(), depth + 1);
    let revision = desk.revision();
    let replay = desk.http(request).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(desk.revision(), revision);
    assert_eq!(desk.depth(), depth + 1);
    desk.undo("undo-first-pan").await;
    assert_eq!(desk.stored(), desk.original);
    let _ = std::fs::remove_dir_all(&desk.scenario.data_dir);
}

#[tokio::test]
async fn neutral_http_touch_pins_accepted_pose_for_ws_turn_and_new_touch_captures_new_frame() {
    let mut desk = PositionDesk::new().await;
    desk.target("gesture-target").await;
    let first = desk.publish(0.5);
    let depth = desk.depth();
    let revision = desk.revision();
    let neutral = desk
        .http(desk.turn("neutral-touch", 0., "retained-touch"))
        .await;
    assert_eq!(neutral["status"], "no_change");
    assert_eq!(desk.revision(), revision);
    assert_eq!(desk.depth(), depth);
    assert_eq!(desk.stored(), desk.original);
    let second = desk.publish(0.55);
    assert!(
        (second.pan_degrees - first.pan_degrees).abs() > 1.,
        "a real Point movement must produce a distinct fitted command pose"
    );
    let request = desk.turn("ws-retained-turn", 5., "retained-touch");
    let action =
        serde_json::from_value(serde_json::json!({"type":"programming_values", "request":request}))
            .unwrap();
    let frame = live_action_frame(&desk.scenario.session, "ws-retained-turn", action);
    let result = dispatch_live_action(&desk.scenario.state, &desk.scenario.session, frame.clone());
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(
        desk.stored(),
        physical::angles(first.pan_degrees + 5., first.tilt_degrees)
    );
    assert_eq!(desk.depth(), depth + 1);
    let revision = desk.revision();
    let replay = dispatch_live_action(&desk.scenario.state, &desk.scenario.session, frame);
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(replay.payload.unwrap()["replayed"], true);
    assert_eq!(desk.revision(), revision);
    desk.undo("undo-ws-pan").await;
    assert_eq!(desk.stored(), desk.original);
    let result = desk
        .http(desk.turn("new-touch-turn", 5., "fresh-touch"))
        .await;
    assert_eq!(result["status"], "changed");
    assert_eq!(
        desk.stored(),
        physical::angles(second.pan_degrees + 5., second.tilt_degrees)
    );
    let _ = std::fs::remove_dir_all(&desk.scenario.data_dir);
}

#[tokio::test]
async fn absent_accepted_pose_is_quiet_and_pinned_until_a_fresh_touch() {
    let mut desk = PositionDesk::new().await;
    desk.target("unpublished-target").await;
    assert!(
        desk.scenario
            .state
            .output
            .latest_visualization_frame()
            .is_none()
    );
    let revision = desk.revision();
    let depth = desk.depth();
    let held = desk
        .http(desk.turn("missing-pose-turn", 5., "missing-touch"))
        .await;
    assert_eq!(held["status"], "no_change");
    assert_eq!(desk.stored(), desk.original);
    assert_eq!(desk.revision(), revision);
    assert_eq!(desk.depth(), depth);
    let pose = desk.publish(0.5);
    let still_held = desk
        .http(desk.turn("same-touch-after-publish", 5., "missing-touch"))
        .await;
    assert_eq!(still_held["status"], "no_change");
    assert_eq!(desk.stored(), desk.original);
    assert_eq!(desk.depth(), depth);
    let moved = desk
        .http(desk.turn("fresh-touch-after-publish", 5., "new-touch"))
        .await;
    assert_eq!(moved["status"], "changed");
    assert_eq!(
        desk.stored(),
        physical::angles(pose.pan_degrees + 5., pose.tilt_degrees)
    );
    let _ = std::fs::remove_dir_all(&desk.scenario.data_dir);
}

#[path = "position_group_authoring_tests.rs"]
mod position_group_authoring_tests;

#[path = "position_gesture_finish_tests.rs"]
mod position_gesture_finish_tests;

#[tokio::test]
async fn captured_position_readouts_keep_their_exact_requested_and_commanded_source() {
    use crate::runtime::position_readout::capture_position_readouts;
    let mut desk = PositionDesk::new().await;
    desk.target("readout-target").await;
    let first_pose = desk.publish(0.54);
    let source = desk
        .scenario
        .state
        .output
        .latest_visualization_frame()
        .unwrap();
    let before_revision = desk.revision();
    let before_depth = desk.depth();
    let before_snapshot = desk.scenario.state.output.snapshot();
    let before_show = desk.scenario.state.active_show.current().unwrap();
    let missing = FixtureId::new();
    let owners = [desk.owner, missing, desk.owner];
    let captured =
        capture_position_readouts(desk.scenario.state.output.engine(), &source, &owners).unwrap();
    assert_eq!(captured.identity, source.identity());
    assert_eq!(captured.scope, source.scope);
    assert_eq!(captured.show_revision, source.show_revision);
    assert_eq!(
        captured
            .owners
            .iter()
            .map(|entry| entry.owner)
            .collect::<Vec<_>>(),
        owners
    );
    assert_eq!(captured.owners[0].requested, Some(desk.original.clone()));
    assert_eq!(captured.owners[0].common_angles(), Some(first_pose));
    assert_eq!(
        captured.owners[0].readout.commands.as_ref().unwrap().len(),
        1
    );
    assert!(captured.owners[1].readout.commands.is_none());
    assert!(captured.owners[1].requested.is_none());
    assert!(captured.owners[1].common_angles().is_none());
    assert_eq!(captured.owners[0].readout, captured.owners[2].readout);
    assert_eq!(desk.revision(), before_revision);
    assert_eq!(desk.depth(), before_depth);
    assert_eq!(desk.stored(), desk.original);
    assert!(Arc::ptr_eq(
        &before_snapshot,
        &desk.scenario.state.output.snapshot()
    ));
    assert_eq!(
        serde_json::to_value(desk.scenario.state.active_show.current().unwrap()).unwrap(),
        serde_json::to_value(before_show).unwrap()
    );
    assert!(Arc::ptr_eq(
        &source,
        &desk
            .scenario
            .state
            .output
            .latest_visualization_frame()
            .unwrap()
    ));

    // Actual moving-target inputs produce another fitted accepted output. Retaining the old
    // source or its derived capture must not silently sample this newer pose on a later read.
    let second_pose = desk.publish(0.60);
    assert_ne!(first_pose, second_pose);
    let latest = desk
        .scenario
        .state
        .output
        .latest_visualization_frame()
        .unwrap();
    assert!(latest.sequence > source.sequence);
    let old_again =
        capture_position_readouts(desk.scenario.state.output.engine(), &source, &[desk.owner])
            .unwrap();
    let new_capture =
        capture_position_readouts(desk.scenario.state.output.engine(), &latest, &[desk.owner])
            .unwrap();
    assert_eq!(old_again.identity, captured.identity);
    assert_eq!(old_again.owners[0].requested, captured.owners[0].requested);
    assert_eq!(old_again.owners[0].readout, captured.owners[0].readout);
    assert_eq!(old_again.owners[0].common_angles(), Some(first_pose));
    assert_eq!(new_capture.identity, latest.identity());
    assert_eq!(new_capture.owners[0].requested, Some(desk.original.clone()));
    assert_eq!(new_capture.owners[0].common_angles(), Some(second_pose));
    assert_eq!(desk.revision(), before_revision);
    assert_eq!(desk.depth(), before_depth);
}

#[tokio::test]
async fn captured_position_readouts_reject_foreign_or_replaced_generation_without_mutation() {
    use crate::runtime::position_readout::capture_position_readouts;
    let mut desk = PositionDesk::new().await;
    desk.target("readout-generation").await;
    desk.publish(0.54);
    let source = desk
        .scenario
        .state
        .output
        .latest_visualization_frame()
        .unwrap();
    let foreign = PositionDesk::new().await;
    assert!(
        capture_position_readouts(
            foreign.scenario.state.output.engine(),
            &source,
            &[desk.owner]
        )
        .is_none()
    );
    let before_revision = desk.revision();
    let before_depth = desk.depth();
    let snapshot = desk.scenario.state.output.snapshot();
    desk.scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    assert!(
        capture_position_readouts(desk.scenario.state.output.engine(), &source, &[desk.owner])
            .is_none()
    );
    assert_eq!(desk.revision(), before_revision);
    assert_eq!(desk.depth(), before_depth);
    assert_eq!(desk.stored(), desk.original);
    assert!(Arc::ptr_eq(
        &source,
        &desk
            .scenario
            .state
            .output
            .latest_visualization_frame()
            .unwrap()
    ));
}
