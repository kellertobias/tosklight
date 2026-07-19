use super::*;

#[test]
fn invalid_snapshot_preparation_does_not_change_live_state() {
    let engine = Engine::new(ProgrammerRegistry::default());
    engine.replace_snapshot(snapshot(1)).unwrap();
    let invalid = EngineSnapshot {
        groups: vec![GroupDefinition {
            id: "invalid".into(),
            name: "Invalid".into(),
            master: 2.0,
            ..GroupDefinition::default()
        }],
        revision: 2,
        ..EngineSnapshot::default()
    };

    assert!(engine.prepare_snapshot(invalid).is_err());
    assert_eq!(engine.snapshot().revision, 1);
}

fn snapshot(revision: u64) -> EngineSnapshot {
    EngineSnapshot {
        revision,
        ..EngineSnapshot::default()
    }
}

#[test]
fn prepared_snapshot_is_installed_without_another_fallible_step() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let prepared = engine
        .prepare_snapshot(EngineSnapshot {
            revision: 2,
            ..EngineSnapshot::default()
        })
        .unwrap();

    assert_eq!(prepared.snapshot().revision, 2);
    assert_eq!(engine.snapshot().revision, 0);
    let installed: () = engine.install_prepared_snapshot(prepared);
    assert_eq!(installed, ());
    assert_eq!(engine.snapshot().revision, 2);
}

#[test]
fn prepared_installation_preserves_compatible_playback_runtime() {
    let cue_list = test_cue_list("Live", vec![]);
    let playback = test_playback(1, cue_list.id);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(playback_snapshot(&cue_list, &playback, 1))
        .unwrap();
    activate_playback(&engine, cue_list.id);

    let prepared = engine
        .prepare_snapshot(playback_snapshot(&cue_list, &playback, 2))
        .unwrap();
    engine.install_prepared_snapshot(prepared);

    assert_eq!(engine.playback().read().active().len(), 1);
    assert!(engine.playback().read().dynamics_paused());
}

fn activate_playback(engine: &Engine, cue_list_id: light_core::CueListId) {
    let mut playback = engine.playback().write();
    playback.go_at(cue_list_id, chrono::Utc::now()).unwrap();
    playback.set_dynamics_paused(true);
}

fn playback_snapshot(
    cue_list: &light_playback::CueList,
    playback: &PlaybackDefinition,
    revision: u64,
) -> EngineSnapshot {
    EngineSnapshot {
        cue_lists: vec![cue_list.clone()],
        playbacks: vec![playback.clone()],
        revision,
        ..EngineSnapshot::default()
    }
}
