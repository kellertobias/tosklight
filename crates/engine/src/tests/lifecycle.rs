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

    assert_eq!(engine.active_playbacks().len(), 1);
    assert!(engine.playback_dynamics().paused);
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
        cue_lists: vec![cue_list.clone()],
        playbacks: vec![playback.clone()],
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
            protocol: light_output::Protocol::ArtNet,
            logical_universe: 1,
            destination_universe,
            delivery_mode: Some(light_output::DeliveryMode::Broadcast),
            destination: None,
            enabled: true,
            minimum_slots: light_output::DMX_SLOTS as u16,
        }],
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
