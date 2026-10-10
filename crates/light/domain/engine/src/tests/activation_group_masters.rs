//! TL-588: persisted Group Master levels are applied to a detached, finalized destination before
//! installation, so activation needs no fallible per-Group replay against Live.
use super::*;
use light_playback::{PlaybackControlState, PlaybackIdentity};

struct Desk {
    engine: Engine,
    front: FixtureId,
    back: FixtureId,
}

fn group(id: &str, fixtures: Vec<FixtureId>) -> GroupDefinition {
    GroupDefinition {
        replacement_projections: Default::default(),
        id: id.into(),
        name: id.into(),
        fixtures,
        ..Default::default()
    }
}

/// Two fixtures at full Programmer intensity. `front` (Playback 1, seed 0.5) and `back`
/// (Playback 2, seed 1.0) are fadered Group Masters; `spare` exists without an assignment.
fn desk() -> Desk {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (first, front) = fixture();
    let (mut second, back) = fixture();
    second.address = Some(2);
    for fixture_id in [front, back] {
        programmers.set(
            session,
            fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
    }
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![first, second].into(),
            playbacks: vec![
                test_group_playback_with_master(1, "front", 0.5),
                test_group_playback_with_master(2, "back", 1.0),
            ]
            .into(),
            groups: vec![
                group("front", vec![front]),
                group("back", vec![back]),
                group("spare", vec![front, back]),
            ]
            .into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    Desk {
        engine,
        front,
        back,
    }
}

fn physical(number: u16) -> PlaybackIdentity {
    PlaybackIdentity::physical(number).unwrap()
}

fn output(engine: &Engine) -> [u8; 2] {
    let frame = engine.render(RenderOptions::default()).unwrap();
    let universe = &frame.universes[&1];
    [universe[0], universe[1]]
}

fn release_candidate(engine: &Engine, snapshot: EngineSnapshot) -> FinalizedEngineSnapshot {
    engine
        .finalize_snapshot_playback_restoring_dynamics(
            engine.prepare_snapshot(snapshot).unwrap(),
            &[],
            None,
        )
        .unwrap()
}

fn next_revision(engine: &Engine) -> EngineSnapshot {
    let mut candidate = (*engine.snapshot()).clone();
    candidate.revision += 1;
    candidate
}

/// Everything preparation must leave untouched on the Live side.
struct LiveEvidence {
    generation: Arc<RuntimeGeneration>,
    playback: Arc<parking_lot::RwLock<light_playback::PlaybackEngine>>,
    masters: [Option<f32>; 3],
    controls: [PlaybackControlState; 2],
    runtime: Vec<light_playback::ActivePlayback>,
    watermark: u64,
    output: [u8; 2],
}

impl LiveEvidence {
    fn capture(engine: &Engine) -> Self {
        let generation = engine.generation.load_full();
        Self {
            playback: generation.playback_arc(),
            generation,
            masters: ["front", "back", "spare"].map(|id| engine.group_master(id)),
            controls: [1, 2].map(|number| engine.playback_control_state_at(physical(number))),
            runtime: engine.playback_runtime(),
            watermark: engine.playback_source_occurrence_watermark(),
            output: output(engine),
        }
    }

    fn assert_unchanged(&self, engine: &Engine) {
        let generation = engine.generation.load_full();
        assert!(
            Arc::ptr_eq(&self.generation, &generation),
            "Live generation"
        );
        assert_eq!(self.generation.identity(), generation.identity());
        assert!(Arc::ptr_eq(&self.playback, &generation.playback_arc()));
        assert_eq!(
            self.masters,
            ["front", "back", "spare"].map(|id| engine.group_master(id))
        );
        assert_eq!(
            self.controls,
            [1, 2].map(|number| engine.playback_control_state_at(physical(number)))
        );
        assert_eq!(self.runtime, engine.playback_runtime());
        assert_eq!(
            self.watermark,
            engine.playback_source_occurrence_watermark()
        );
        assert_eq!(self.output, output(engine));
    }
}

fn levels(entries: &[(&str, f32)]) -> HashMap<String, f32> {
    entries
        .iter()
        .map(|(id, level)| ((*id).to_owned(), *level))
        .collect()
}

#[test]
fn prepared_group_masters_restore_levels_and_physical_controls_before_install() {
    // Reused topology (only the revision changes) and changed topology (a new fadered assignment).
    for changed_topology in [false, true] {
        let desk = desk();
        let engine = &desk.engine;
        // A Live operator gesture that the destination must neither inherit nor disturb.
        engine
            .execute_playback(EnginePlaybackCommand::Pool {
                number: 1,
                action: PoolPlaybackAction::SetGroupMasterFader {
                    value: 0.5,
                    authoritative: 0.5,
                },
            })
            .unwrap();
        engine.reserve_playback_source_occurrence_watermark(300);
        let mut candidate = next_revision(engine);
        if changed_topology {
            Arc::make_mut(&mut candidate.playbacks)
                .push(test_group_playback_with_master(3, "front", 0.5));
        }
        let live = LiveEvidence::capture(engine);
        assert_eq!(live.output, [128, 255]);

        let mut finalized = release_candidate(engine, candidate);
        let snapshot = finalized.snapshot_arc();
        let dynamics = finalized.dynamic_playbacks().to_vec();
        let cue_dynamics = finalized.cue_dynamic_values().to_vec();
        let report = finalized
            .prepare_group_masters(&levels(&[("front", 0.0), ("back", 1.0)]))
            .unwrap();
        assert_eq!(
            report.restored(),
            &[
                PreparedGroupMaster {
                    group_id: "back".into(),
                    level: 1.0,
                    changed: false,
                },
                PreparedGroupMaster {
                    group_id: "front".into(),
                    level: 0.0,
                    changed: true,
                },
            ]
        );
        assert!(report.missing().is_empty());
        live.assert_unchanged(engine);

        // The detached destination already carries the restored levels and pickup targets.
        assert_eq!(finalized.group_master("front"), Some(0.0));
        assert_eq!(finalized.group_master("back"), Some(1.0));
        let mut fadered = vec![(1, 0.0), (2, 1.0)];
        if changed_topology {
            fadered.push((3, 0.0));
        }
        for &(number, level) in &fadered {
            let control = finalized.playback_control_state_at(physical(number));
            assert!(control.fader_pickup_required, "Playback {number}");
            assert_eq!(
                control.fader_pickup_target,
                Some(level),
                "Playback {number}"
            );
        }
        // The exact finalized token: identity, captured owners and watermark are unchanged.
        assert!(Arc::ptr_eq(&snapshot, &finalized.snapshot_arc()));
        assert_eq!(finalized.dynamic_playbacks(), dynamics.as_slice());
        assert_eq!(finalized.cue_dynamic_values(), cue_dynamics.as_slice());
        let prepared_playback = finalized.playback_arc_for_test();
        assert!(!Arc::ptr_eq(&prepared_playback, &live.playback));

        engine.install_finalized_snapshot(finalized);
        let installed = engine.generation.load_full();
        assert!(Arc::ptr_eq(&installed.playback_arc(), &prepared_playback));
        assert!(Arc::ptr_eq(&installed.snapshot_arc(), &snapshot));
        assert_eq!(engine.group_master("front"), Some(0.0));
        assert_eq!(engine.group_master("back"), Some(1.0));
        for &(number, level) in &fadered {
            let control = engine.playback_control_state_at(physical(number));
            assert!(control.fader_pickup_required);
            assert_eq!(control.fader_pickup_target, Some(level));
        }
        assert_eq!(engine.playback_source_occurrence_watermark(), 300);
        assert_eq!(output(engine), [0, 255]);
        let _ = (desk.front, desk.back);
    }
}

#[test]
fn prepared_install_matches_the_legacy_post_install_replay_exactly() {
    let saved = levels(&[
        ("front", 0.25),
        ("back", 0.0),
        ("ghost", 0.6),
        ("spare", 0.4),
    ]);
    // Negative control: the legacy path mutates Live after installation, one fallible Group at a
    // time, and must skip unassigned or deleted Groups by error.
    let legacy = desk();
    legacy.engine.install_finalized_snapshot(release_candidate(
        &legacy.engine,
        next_revision(&legacy.engine),
    ));
    let installed = legacy.engine.generation.load_full();
    let mut failures = Vec::new();
    let mut ids = saved.keys().collect::<Vec<_>>();
    ids.sort();
    for group_id in ids {
        if legacy
            .engine
            .set_group_master(group_id, saved[group_id])
            .is_err()
        {
            failures.push(group_id.clone());
        }
    }
    assert_eq!(failures, ["ghost", "spare"]);
    assert!(
        !Arc::ptr_eq(&installed, &legacy.engine.generation.load_full()),
        "legacy replay publishes further Live generations after installation"
    );

    let prepared = desk();
    let mut finalized = release_candidate(&prepared.engine, next_revision(&prepared.engine));
    let report = finalized.prepare_group_masters(&saved).unwrap();
    assert_eq!(report.missing(), ["ghost", "spare"]);
    prepared.engine.install_finalized_snapshot(finalized);
    let installed = prepared.engine.generation.load_full();

    for id in ["front", "back", "spare", "ghost"] {
        assert_eq!(
            prepared.engine.group_master(id),
            legacy.engine.group_master(id),
            "{id}"
        );
    }
    for number in [1, 2] {
        assert_eq!(
            prepared.engine.playback_control_state_at(physical(number)),
            legacy.engine.playback_control_state_at(physical(number))
        );
    }
    assert_eq!(output(&prepared.engine), output(&legacy.engine));
    assert_eq!(output(&prepared.engine), [64, 0]);
    // Installation of the prepared token is final; nothing replays afterwards.
    assert!(Arc::ptr_eq(
        &installed,
        &prepared.engine.generation.load_full()
    ));
}

#[test]
fn missing_and_deleted_groups_are_skipped_passively_with_structured_information() {
    let desk = desk();
    let engine = &desk.engine;
    let mut candidate = next_revision(engine);
    // Delete Group `back` and its Playback; `spare` stays defined but unassigned.
    Arc::make_mut(&mut candidate.playbacks).retain(|playback| playback.number != 2);
    Arc::make_mut(&mut candidate.groups).retain(|group| group.id != "back");
    let live = LiveEvidence::capture(engine);
    let mut finalized = release_candidate(engine, candidate);
    let report = finalized
        .prepare_group_masters(&levels(&[
            ("front", 1.0),
            ("back", 0.3),
            ("spare", 0.2),
            ("never", 0.1),
        ]))
        .unwrap();
    assert_eq!(report.missing(), ["back", "never", "spare"]);
    assert_eq!(report.restored().len(), 1);
    assert_eq!(report.restored()[0].group_id, "front");
    assert!(report.restored()[0].changed);
    assert_eq!(finalized.group_master("back"), None);
    assert_eq!(finalized.group_master("spare"), None);
    assert_eq!(
        finalized.playback_control_state_at(physical(2)),
        PlaybackControlState::default()
    );
    live.assert_unchanged(engine);
    engine.install_finalized_snapshot(finalized);
    assert_eq!(engine.group_master("front"), Some(1.0));
    assert_eq!(engine.group_master("back"), None);
    assert_eq!(output(engine), [255, 255]);
}

#[test]
fn invalid_saved_levels_reject_before_any_candidate_or_live_mutation() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
        let desk = desk();
        let engine = &desk.engine;
        let live = LiveEvidence::capture(engine);
        let mut finalized = release_candidate(engine, next_revision(engine));
        let seeds = (
            finalized.group_master("front"),
            finalized.playback_control_state_at(physical(1)),
            finalized.playback_control_state_at(physical(2)),
        );
        // Valid entries sort on both sides of the invalid one; none may be applied.
        let error = finalized
            .prepare_group_masters(&levels(&[
                ("back", 0.0),
                ("front", 0.2),
                ("middle", invalid),
            ]))
            .unwrap_err();
        assert!(error.to_string().contains("middle"), "{error}");
        assert_eq!(
            seeds,
            (
                finalized.group_master("front"),
                finalized.playback_control_state_at(physical(1)),
                finalized.playback_control_state_at(physical(2)),
            )
        );
        assert_eq!(finalized.group_master("front"), None, "not prepared");
        live.assert_unchanged(engine);
        // A rejected preparation leaves an ordinary installable token with seeded levels.
        engine.install_finalized_snapshot(finalized);
        assert_eq!(engine.group_master("front"), Some(0.5));
        assert_eq!(engine.group_master("back"), Some(1.0));
        assert_eq!(
            engine.playback_control_state_at(physical(1)),
            PlaybackControlState::default()
        );
    }
}

#[test]
fn preparation_is_single_use_and_requires_release_policy() {
    let desk = desk();
    let engine = &desk.engine;
    let mut finalized = release_candidate(engine, next_revision(engine));
    finalized
        .prepare_group_masters(&levels(&[("front", 0.1)]))
        .unwrap();
    let control = finalized.playback_control_state_at(physical(1));
    assert!(
        finalized
            .prepare_group_masters(&levels(&[("front", 0.9), ("back", 0.9)]))
            .is_err()
    );
    assert_eq!(finalized.group_master("front"), Some(0.1));
    assert_eq!(finalized.group_master("back"), Some(1.0));
    assert_eq!(finalized.playback_control_state_at(physical(1)), control);
    assert_eq!(
        finalized.playback_control_state_at(physical(2)),
        PlaybackControlState::default()
    );

    // A preserving token keeps Live levels at installation; persisted levels do not apply.
    let live = LiveEvidence::capture(engine);
    let mut preserving = engine
        .finalize_snapshot_playback(
            engine.prepare_snapshot(next_revision(engine)).unwrap(),
            true,
        )
        .unwrap();
    assert!(
        preserving
            .prepare_group_masters(&levels(&[("front", 0.1)]))
            .is_err()
    );
    live.assert_unchanged(engine);
    engine.install_finalized_snapshot(preserving);
    assert_eq!(engine.group_master("front"), Some(0.5));
}

#[test]
fn ordinary_group_master_gestures_after_prepared_install_are_unchanged() {
    let desk = desk();
    let engine = &desk.engine;
    let mut finalized = release_candidate(engine, next_revision(engine));
    finalized
        .prepare_group_masters(&levels(&[("front", 0.0)]))
        .unwrap();
    engine.install_finalized_snapshot(finalized);
    // Picking up the physical fader at the restored level moves the master as before.
    engine
        .execute_playback(EnginePlaybackCommand::Pool {
            number: 1,
            action: PoolPlaybackAction::SetGroupMasterFader {
                value: 0.0,
                authoritative: 0.0,
            },
        })
        .unwrap();
    assert!(
        !engine
            .playback_control_state_at(physical(1))
            .fader_pickup_required
    );
    assert!(!engine.set_group_master("front", 0.0).unwrap());
    assert!(engine.set_group_master("front", 0.75).unwrap());
    assert!(engine.set_group_master("ghost", 0.5).is_err());
    assert!(engine.set_group_master("front", 1.5).is_err());
    assert_eq!(engine.group_master("front"), Some(0.75));
    assert_eq!(output(engine)[0], 191);
}
