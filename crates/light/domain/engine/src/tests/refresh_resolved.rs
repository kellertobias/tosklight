//! Discarded diagnostic reads keep the old resolution's exact observational semantics.
use super::*;

struct Rig {
    engine: Engine,
    clock: Arc<ManualClock>,
    programmers: ProgrammerRegistry,
    session: SessionId,
}
impl Rig {
    fn new(snapshot: EngineSnapshot, started: chrono::DateTime<Utc>, session: SessionId) -> Self {
        let clock = Arc::new(ManualClock::new(started));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let mut state = programmers.start(session);
        state.id = ProgrammerId(uuid::Uuid::from_u128(0x775d_1000));
        programmers.restore(state);
        let engine = Engine::new(programmers.clone());
        engine.replace_snapshot(snapshot).unwrap();
        Self {
            engine,
            clock,
            programmers,
            session,
        }
    }
    fn runtime(&self) -> serde_json::Value {
        serde_json::json!({
            "mib":self.engine.move_in_black_runtime(),
            "playback":self.engine.playback_runtime(),
            "status":self.engine.playback_runtime_status(),
            "source_watermark":self.engine.playback_source_occurrence_watermark(),
        })
    }
}
fn assert_render_equal(a: &RenderResult, b: &RenderResult) {
    assert_eq!(a.sampled_at, b.sampled_at);
    assert_eq!(a.revision, b.revision);
    assert_eq!(&*a.universes, &*b.universes);
    assert_eq!(&*a.patched_slots, &*b.patched_slots);
    assert_eq!(a.resolved_values.values(), b.resolved_values.values());
    assert_eq!(
        a.profile_visualization_values.as_ref(),
        b.profile_visualization_values.as_ref()
    );
    let physical = |frame: &RenderResult| {
        let mut rows = frame
            .physical
            .instances
            .iter()
            .map(|p| {
                format!(
                    "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
                    p.fixture_id,
                    p.instance_id,
                    p.native_raw,
                    p.colors(),
                    p.axes(),
                    p.lenses(),
                    p.optics(),
                    p.native_identity,
                    p.complete
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    };
    assert_eq!(physical(a), physical(b));
    assert_eq!(
        format!("{:?}", a.points.as_slice()),
        format!("{:?}", b.points.as_slice())
    );
    assert_eq!(
        format!("{:?}", a.mounts.mounts()),
        format!("{:?}", b.mounts.mounts())
    );
    assert_eq!(
        a.automatic_playback_transitions,
        b.automatic_playback_transitions
    );
}
fn observe_then_render(old: &Rig, refresh: &Rig) {
    assert_eq!(old.runtime(), refresh.runtime());
    let before = old.runtime();
    let old_revision = old.engine.capture_output_continuity().0;
    let refresh_revision = refresh.engine.capture_output_continuity().0;
    for _ in 0..3 {
        drop(old.engine.resolved_values());
        refresh.engine.refresh_resolved_state();
        assert_eq!(old.engine.capture_output_continuity().0, old_revision);
        assert_eq!(
            refresh.engine.capture_output_continuity().0,
            refresh_revision
        );
        assert_eq!(
            old.runtime(),
            before,
            "ordinary reads must not advance or consume authoritative playback/MIB"
        );
        assert_eq!(
            refresh.runtime(),
            before,
            "refresh must preserve the existing read-only continuity behavior"
        );
    }
    // A later value consumer still gets the complete named resolution after either read path.
    assert_eq!(
        old.engine.resolved_values(),
        refresh.engine.resolved_values()
    );
    let a = old.engine.render(RenderOptions::default()).unwrap();
    let b = refresh.engine.render(RenderOptions::default()).unwrap();
    assert_render_equal(&a, &b);
    assert_eq!(old.runtime(), refresh.runtime());
}
#[test]
fn discarded_refresh_matches_named_reads_at_exact_move_in_black_checkpoints() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let (enabled, first) = moving_fixture(1, true, 1_000);
    let (disabled, second) = moving_fixture(10, false, 1_000);
    let snapshot = mib_snapshot(vec![enabled, disabled], &[first, second]);
    let session = SessionId(uuid::Uuid::from_u128(0x775d_1001));
    let old = Rig::new(snapshot.clone(), started, session);
    let refresh = Rig::new(snapshot, started, session);
    for rig in [&old, &refresh] {
        execute_address(&rig.engine, playback_address(1, 1), PlaybackAction::Go);
        execute_address(&rig.engine, playback_address(1, 1), PlaybackAction::Go);
    }
    for millis in [1999, 2000, 2999, 3000, 4500, 6000] {
        for rig in [&old, &refresh] {
            rig.clock
                .set(started + ChronoDuration::milliseconds(millis));
        }
        observe_then_render(&old, &refresh);
    }
    for rig in [&old, &refresh] {
        execute_address(&rig.engine, playback_address(1, 1), PlaybackAction::Go);
    }
    observe_then_render(&old, &refresh);
}
#[test]
fn discarded_refresh_preserves_group_overflow_tracking_and_edited_generation_reads() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let (fixture, target) = schema_v2_fixture(&[
        ("intensity", false, false),
        ("point.position.x", false, false),
    ]);
    let group = GroupDefinition {
        id: "refresh-group".into(),
        name: "Refresh".into(),
        fixtures: vec![target],
        ..Default::default()
    };
    let snapshot = EngineSnapshot {
        fixtures: vec![fixture].into(),
        groups: vec![group].into(),
        revision: 1,
        ..Default::default()
    };
    let session = SessionId(uuid::Uuid::from_u128(0x775d_1002));
    let old = Rig::new(snapshot.clone(), started, session);
    let refresh = Rig::new(snapshot.clone(), started, session);
    for rig in [&old, &refresh] {
        rig.programmers.set(
            rig.session,
            target,
            AttributeKey("iris".into()),
            AttributeValue::Normalized(0.35),
        );
        rig.programmers.set_group(
            rig.session,
            "refresh-group".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
        );
        rig.engine
            .set_tracked_overrides([crate::TrackedOverride::new(
                target,
                AttributeKey("point.position.x".into()),
                AttributeValue::Normalized(0.8),
            )]);
        rig.engine
            .set_group_color(
                "refresh-group",
                Some(Xyz {
                    x: 0.1,
                    y: 0.2,
                    z: 0.3,
                }),
            )
            .unwrap();
    }
    observe_then_render(&old, &refresh);
    for rig in [&old, &refresh] {
        let values = rig.engine.resolved_values();
        assert_eq!(
            values[&(target, AttributeKey::intensity())],
            AttributeValue::Normalized(0.7)
        );
        assert_eq!(
            values[&(target, AttributeKey("color".into()))],
            AttributeValue::ColorXyz(Xyz {
                x: 0.1,
                y: 0.2,
                z: 0.3
            })
        );
        assert_eq!(
            values[&(target, AttributeKey("iris".into()))],
            AttributeValue::Normalized(0.35)
        );
        assert_eq!(
            values[&(target, AttributeKey("point.position.x".into()))],
            AttributeValue::Normalized(0.8)
        );
        let mut edited = snapshot.clone();
        edited.revision = 2;
        rig.engine.replace_snapshot(edited).unwrap();
        rig.programmers.set(
            rig.session,
            target,
            AttributeKey("iris".into()),
            AttributeValue::Normalized(0.65),
        );
        rig.clock.advance_millis(25);
    }
    observe_then_render(&old, &refresh);
}
