use super::*;
use std::sync::Barrier;

#[test]
fn invalid_snapshot_preparation_does_not_change_live_state() {
    let engine = Engine::new(ProgrammerRegistry::default());
    engine.replace_snapshot(snapshot(1)).unwrap();
    let invalid = EngineSnapshot {
        groups: vec![GroupDefinition {
            id: "invalid".into(),
            name: "Invalid".into(),
            ..GroupDefinition::default()
        }]
        .into(),
        playbacks: vec![test_group_playback_with_master(1, "invalid", 2.0)].into(),
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

    assert_eq!(engine.active_playbacks().len(), 1);
    assert!(engine.playback_dynamics().paused);
}

#[test]
fn playback_runtime_survives_one_detach_and_releases_after_the_final_assignment() {
    let cue_list = test_cue_list("Shared", vec![]);
    let physical = test_playback(1, cue_list.id);
    let mut virtual_playback = test_playback(1_001, cue_list.id);
    virtual_playback.has_fader = false;
    let page = light_playback::PlaybackPage {
        number: 1,
        name: "Virtual".into(),
        slots: HashMap::new(),
        virtual_playbacks: HashMap::from([(1_001, virtual_playback)]),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list.clone()].into(),
            playbacks: vec![physical].into(),
            playback_pages: vec![page.clone()].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::On);

    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list.clone()].into(),
            playback_pages: vec![page].into(),
            revision: 2,
            ..EngineSnapshot::default()
        })
        .unwrap();
    assert_eq!(engine.active_playbacks().len(), 1);

    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![cue_list].into(),
            revision: 3,
            ..EngineSnapshot::default()
        })
        .unwrap();
    assert!(engine.active_playbacks().is_empty());
}

#[test]
fn reassignment_releases_the_final_reference_to_the_old_target() {
    let first = test_cue_list("First", vec![]);
    let second = test_cue_list("Second", vec![]);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![first.clone(), second.clone()].into(),
            playbacks: vec![test_playback(1, first.id)].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    activate_playback(&engine, first.id);

    engine
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![first, second.clone()].into(),
            playbacks: vec![test_playback(1, second.id)].into(),
            revision: 2,
            ..EngineSnapshot::default()
        })
        .unwrap();

    assert!(engine.active_playbacks().is_empty());
    assert!(
        engine
            .snapshot()
            .cue_lists
            .iter()
            .any(|cue| cue.id == second.id)
    );
}

#[test]
fn final_group_assignment_removes_transient_master_state() {
    let group = GroupDefinition {
        id: "front".into(),
        name: "Front".into(),
        ..GroupDefinition::default()
    };
    let group_playback = |number| {
        let mut playback = test_playback(number, light_core::CueListId::new());
        playback.target = PlaybackTarget::Group {
            group_id: "front".into(),
            initial_master: Some(0.4),
        };
        playback.buttons = PlaybackDefinition::default_buttons(&playback.target);
        playback.fader = PlaybackDefinition::default_fader(&playback.target);
        playback
    };
    let mut virtual_group = group_playback(1_001);
    virtual_group.has_fader = false;
    let page = light_playback::PlaybackPage {
        number: 1,
        name: "Virtual".into(),
        slots: HashMap::new(),
        virtual_playbacks: HashMap::from([(1_001, virtual_group)]),
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![group.clone()].into(),
            playbacks: vec![group_playback(1)].into(),
            playback_pages: vec![page.clone()].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_group_master_flash("front".into(), 0.9);

    engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![group.clone()].into(),
            playback_pages: vec![page].into(),
            revision: 2,
            ..EngineSnapshot::default()
        })
        .unwrap();
    assert_eq!(engine.group_master("front"), Some(0.4));
    assert_eq!(engine.group_master_flash("front"), 0.9);

    engine
        .replace_snapshot(EngineSnapshot {
            groups: vec![group].into(),
            revision: 3,
            ..EngineSnapshot::default()
        })
        .unwrap();
    assert_eq!(engine.group_master("front"), None);
    assert_eq!(engine.group_master_flash("front"), 0.0);
}

fn activate_playback(engine: &Engine, cue_list_id: light_core::CueListId) {
    execute_cue_list(
        engine,
        cue_list_id,
        CueListPlaybackAction::GoAt(chrono::Utc::now()),
    );
    engine
        .execute_playback(EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
}

fn playback_snapshot(
    cue_list: &light_playback::CueList,
    playback: &PlaybackDefinition,
    revision: u64,
) -> EngineSnapshot {
    EngineSnapshot {
        cue_lists: vec![cue_list.clone()].into(),
        playbacks: vec![playback.clone()].into(),
        revision,
        ..EngineSnapshot::default()
    }
}

#[test]
fn render_retains_one_generation_across_concurrent_installation() {
    let engine = Arc::new(Engine::new(ProgrammerRegistry::default()));
    engine.replace_snapshot(snapshot_with_route(1, 1)).unwrap();
    let loaded = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    let render_engine = Arc::clone(&engine);
    let render_loaded = Arc::clone(&loaded);
    let render_resume = Arc::clone(&resume);
    let rendering = std::thread::spawn(move || {
        render_engine
            .render_with_generation_hook(RenderOptions::default(), || {
                render_loaded.wait();
                render_resume.wait();
            })
            .unwrap()
    });

    loaded.wait();
    engine.replace_snapshot(snapshot_with_route(2, 2)).unwrap();
    resume.wait();
    let rendered = rendering.join().unwrap();

    assert_eq!(rendered.revision, 1);
    assert_eq!(rendered.routes[0].destination_universe, 1);
    assert_eq!(engine.snapshot().revision, 2);
    assert_eq!(engine.output_routes()[0].destination_universe, 2);
}

fn snapshot_with_route(revision: u64, destination_universe: u16) -> EngineSnapshot {
    EngineSnapshot {
        routes: vec![light_output::OutputRoute {
            target: Default::default(),
            protocol: light_output::Protocol::ArtNet,
            logical_universe: 1,
            destination_universe,
            delivery_mode: Some(light_output::DeliveryMode::Broadcast),
            destination: None,
            enabled: true,
            minimum_slots: light_output::DMX_SLOTS as u16,
        }]
        .into(),
        revision,
        ..EngineSnapshot::default()
    }
}

#[test]
fn read_only_projection_does_not_block_other_read_only_projections() {
    let engine = Arc::new(Engine::new(ProgrammerRegistry::default()));
    let projection_engine = Arc::clone(&engine);
    let (sent, received) = std::sync::mpsc::channel();

    let first = std::thread::spawn(move || {
        for _ in 0..1_000 {
            let _ = projection_engine.playback_runtime_status();
        }
    });
    let second_engine = Arc::clone(&engine);
    std::thread::spawn(move || sent.send(second_engine.resolved_values()).unwrap());

    assert!(
        received
            .recv_timeout(std::time::Duration::from_secs(1))
            .is_ok(),
        "read-only projection unexpectedly waited for exclusive Playback access"
    );
    first.join().unwrap();
}
