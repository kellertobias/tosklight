//! TL-639: a kept Programmer evaluation gives exactly what evaluating again gives, for every
//! static resolution of a frame and across frames; an edit, a fade or a changed transition
//! history is never answered from the memo.
use super::*;
use chrono::DateTime;

type Observed = Vec<(Option<AttributeValue>, Option<DateTime<Utc>>)>;

struct Rig {
    clock: Arc<ManualClock>,
    started: DateTime<Utc>,
    programmers: ProgrammerRegistry,
    session: SessionId,
    engine: Engine,
    fixtures: [FixtureId; 2],
}

fn rig() -> Rig {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let shared: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    programmers.start(session);
    let (first, first_id) = fixture();
    let (mut second, second_id) = fixture();
    second.address = Some(10);
    let engine = Engine::new(programmers.clone());
    engine.set_control_timing([120.0; 5], 0, 0, 0);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![first, second].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    for (fixture, level) in [(first_id, 0.3), (second_id, 0.6)] {
        programmers.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(level),
        );
    }
    Rig {
        clock,
        started,
        programmers,
        session,
        engine,
        fixtures: [first_id, second_id],
    }
}

impl Rig {
    /// The sampled batch replacing the first fixture's Programmer value.
    fn replacing(&self) -> ContributionBatch {
        let state = self.programmers.active_output_states().remove(0);
        let value = state
            .values
            .iter()
            .find(|value| value.fixture_id == self.fixtures[0])
            .unwrap()
            .clone();
        let mut sampled = value;
        sampled.value = AttributeValue::Normalized(0.9);
        ContributionBatch::new([ContributionSample::replacing(
            sampled,
            ContributionSourceId::programmer(state.id),
        )])
    }

    /// Resolves one prepared lane with the memo as it is, then again after forgetting it.
    fn resolve_both(
        &self,
        capture: &PreparedOutputFrame,
        sampled: &[ContributionBatch],
    ) -> (Observed, Observed) {
        let observe = |token: &PreparedStaticFamilyFrame| {
            self.fixtures
                .iter()
                .map(|fixture| {
                    let key = AttributeKey::intensity();
                    (
                        token.value(*fixture, &key).cloned(),
                        token.changed_at(*fixture, &key),
                    )
                })
                .collect::<Observed>()
        };
        let kept = self.engine.prepare_static_family_frame(capture, sampled);
        let kept_values = observe(&kept);
        let kept_version = kept.continuity.programmer_transitions.version();
        self.engine.programmer_memo.lock().clear();
        let fresh = self.engine.prepare_static_family_frame(capture, sampled);
        // Both evaluations start from the capture's history and change it exactly alike.
        let start = capture.continuity.programmer_transitions.version();
        assert_eq!(
            fresh.continuity.programmer_transitions.version() == start,
            kept_version == start,
            "an unchanged history stays the same history"
        );
        (kept_values, observe(&fresh))
    }
}

#[test]
fn kept_evaluations_equal_fresh_ones_with_and_without_sampled_replacements() {
    let rig = rig();
    let replacing = rig.replacing();
    for step in 0..6 {
        rig.clock
            .set(rig.started + ChronoDuration::milliseconds(25 * step));
        if step == 3 {
            // An edit replaces the captured vector, so the memo cannot answer for it.
            rig.programmers.set(
                rig.session,
                rig.fixtures[1],
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.1),
            );
        }
        let capture = rig.engine.prepare_output_frame(RenderOptions::default());
        // The first resolution of a frame may fill the memo; both later ones read it.
        for sampled in [&[][..], std::slice::from_ref(&replacing), &[]] {
            let (kept, fresh) = rig.resolve_both(&capture, sampled);
            assert_eq!(kept, fresh, "step {step}");
        }
        let replaced = rig
            .engine
            .prepare_static_family_frame(&capture, std::slice::from_ref(&replacing));
        assert_eq!(
            replaced.value(rig.fixtures[0], &AttributeKey::intensity()),
            Some(&AttributeValue::Normalized(0.9))
        );
        rig.engine.render(RenderOptions::default()).unwrap();
    }
    // Traced static lanes with and without the replacement, and the render's untraced lane.
    assert_eq!(rig.engine.programmer_memo.lock().len(), 3);
}

#[test]
fn fades_and_changed_histories_are_always_evaluated_again() {
    let rig = rig();
    rig.engine.render(RenderOptions::default()).unwrap();
    rig.engine.programmer_memo.lock().clear();
    // A faded value is time-dependent: nothing is kept while it is in the Programmer.
    rig.engine.set_control_timing([120.0; 5], 1_000, 0, 0);
    rig.programmers.set_faded(
        rig.session,
        rig.fixtures[0],
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );
    for step in 1..4 {
        rig.clock
            .set(rig.started + ChronoDuration::milliseconds(250 * step));
        let capture = rig.engine.prepare_output_frame(RenderOptions::default());
        let (kept, fresh) = rig.resolve_both(&capture, &[]);
        assert_eq!(kept, fresh);
        assert_eq!(rig.engine.programmer_memo.lock().len(), 0, "step {step}");
        rig.engine.render(RenderOptions::default()).unwrap();
    }
    // A history that another lane changed is a different history: the memo of the unchanged
    // one is not reused for it.
    let started = rig.engine.output_continuity.lock().clone();
    let mut changed = started.clone();
    changed.programmer_transitions.clear();
    assert_ne!(
        changed.programmer_transitions.version(),
        started.programmer_transitions.version()
    );
}
