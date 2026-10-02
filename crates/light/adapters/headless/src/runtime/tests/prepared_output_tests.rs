//! A failed output projection must not discard an already committed automatic Cue transition.
use super::*;
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use light_application::{ApplicationEvent, EventFilter, EventReplay, PlaybackEvent};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_engine::EngineError;
use light_wire::v2::visualization::VisualizationScope;

#[derive(Clone, Copy, Debug)]
enum Supersession {
    ClearContinuity,
    NewerCommittedFrame,
    SamplingFailure,
}

fn retained_output_resource(
    engine: Arc<Engine>,
    clock: Arc<ManualClock>,
) -> (OutputResource, Arc<Mutex<OutputControl>>) {
    let events = EventBus::default();
    let control = Arc::new(Mutex::new(OutputControl::default()));
    let dynamic_snapshot = Arc::new(DynamicSnapshotPublication::new(engine.snapshot()));
    let output = OutputResource::new(
        OutputRuntimeService::new(events.clone()),
        SpeedGroupService::new(events),
        engine,
        Arc::new(std::sync::Mutex::new(OutputHealth::default())),
        Arc::new(AtomicU16::new(40)),
        OutputControlCapability::new(Arc::clone(&control)),
        Arc::new(Mutex::new(TimecodeRouter::default())),
        None,
        Arc::new(light_output::UsbOutputFanout::new(Arc::new(
            light_output::UnavailableUsbDriverFactory,
        ))),
        Arc::default(),
        Some(clock),
        Arc::new(Mutex::new(std::array::from_fn(|_| {
            SpeedGroupController::new(120.0, Default::default()).unwrap()
        }))),
        Arc::new(Mutex::new(light_dynamics::DynamicRuntime::default())),
        dynamic_snapshot,
        Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::runtime::dynamic_source_origins::DynamicSourceOrigins::default(),
        )),
        Arc::default(),
        Arc::new(super::super::visualization_frame::VisualizationFrameHub::default()),
    );
    (output, control)
}

fn chaser(fixture: FixtureId) -> light_playback::CueList {
    let mut list = operational_cue_list(light_core::CueListId::new(), fixture);
    list.name = "Prepared output Chaser".into();
    list.mode = light_playback::CueListMode::Chaser;
    list.looped = true;
    list.chaser_step_millis = 100;
    list.wrap_mode = Some(light_playback::WrapMode::Reset);
    list.cues = [0.25, 0.75]
        .into_iter()
        .enumerate()
        .map(|(index, level)| {
            let mut next = light_playback::Cue::new(((index + 1) as u16).into());
            next.changes.push(light_playback::CueChange::set(
                fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(level),
            ));
            next
        })
        .collect();
    list
}

fn retained_events(state: &AppState, after: u64) -> Vec<Arc<light_application::EventEnvelope>> {
    let EventReplay::Events(events) = state.events.replay(after, &EventFilter::default()) else {
        panic!("prepared output events must remain replayable");
    };
    events
}

fn automatic_transitions(
    state: &AppState,
    after: u64,
) -> Vec<light_application::PlaybackCueTransition> {
    retained_events(state, after)
        .iter()
        .filter_map(|event| match &event.payload {
            ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)) => {
                change.transition.clone()
            }
            _ => None,
        })
        .collect()
}

#[test]
fn automatic_timecode_action_uses_captured_cue_and_can_be_claimed_once() {
    let started = "2026-01-01T00:00:00Z".parse().unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let fixture = FixtureId::new();
    let mut list = chaser(fixture);
    let list_id = list.id;
    let action = light_playback::CueAction::TimecodeStop {
        timecode_id: light_playback::TimecodeId(uuid::Uuid::from_u128(0xace)),
    };
    list.cues[1].actions.push(action.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            cue_lists: vec![list].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: list_id,
            action: light_engine::CueListPlaybackAction::GoAt(started),
        })
        .unwrap();
    clock.advance_millis(100);
    let frame = engine.prepare_output_frame(RenderOptions::default());
    assert_eq!(frame.automatic_playback_transitions().len(), 1);

    // The current engine can already have a different show by the time a caller handles the
    // captured transition. Resolution must use the captured Cue and final playback flags.
    engine.replace_snapshot(EngineSnapshot::default()).unwrap();
    assert_eq!(
        output_scheduler::claim_automatic_cue_action_batches(&frame),
        vec![(list_id, vec![action])],
    );
    assert!(output_scheduler::claim_automatic_cue_action_batches(&frame).is_empty());
}

#[test]
fn sampling_failure_preserves_capture_transition_without_completed_output() {
    assert_failed_capture_preserves_transition_and_retained_output(Supersession::SamplingFailure);
}

fn assert_failed_capture_preserves_transition_and_retained_output(supersession: Supersession) {
    let started = "2026-01-01T00:00:00Z".parse().unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let engine = Arc::new(Engine::new(ProgrammerRegistry::with_clock(clock.clone())));
    let (state, data_dir) = test_state();
    let show = ShowEntry {
        is_base_show: false,
        id: light_core::ShowId::new(),
        name: "Prepared output failure".into(),
        path: data_dir
            .join("shows/prepared-output.show")
            .display()
            .to_string(),
        revision: 1,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    state.active_show.replace_current(Some(show.clone()));
    let fixture_id = FixtureId::new();
    let mut fixture = operational_fixture(fixture_id);
    fixture.address = Some(7);
    let list = chaser(fixture_id);
    let list_id = list.id;
    let next_cue = list.cues[1].id;
    let mut snapshot = EngineSnapshot {
        fixtures: vec![fixture].into(),
        cue_lists: vec![list].into(),
        routes: vec![operational_route()].into(),
        revision: 1,
        ..Default::default()
    };
    engine.replace_snapshot(snapshot.clone()).unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: list_id,
            action: light_engine::CueListPlaybackAction::GoAt(started),
        })
        .unwrap();
    let (output, control) = retained_output_resource(Arc::clone(&engine), Arc::clone(&clock));
    let scope = VisualizationScope {
        show_id: Some(show.id.0),
    };
    let first = RenderedSemanticFrame::untraced(
        engine.render(RenderOptions::default()).unwrap(),
        RenderOptions::default(),
    );
    assert!(first.rendered.automatic_playback_transitions.is_empty());
    output.render_frames_and_publish(&first, scope);
    let initial_source = output.latest_visualization_frame().unwrap();
    let old_routes = Arc::clone(&first.rendered.routes);
    let old_slots = first.rendered.patched_slots.clone();
    let old_frames = first.rendered.universes.clone();
    assert_eq!(old_slots.get(&1), Some(&7));
    assert_eq!(old_frames[&1][6], 64);

    // A failed frame contains a different universe and patch footprint, making accidental
    // retention of only its bytes (or only its routes) observable.
    let fixture = &mut Arc::make_mut(&mut snapshot.fixtures)[0];
    fixture.universe = Some(2);
    fixture.address = Some(25);
    let route = &mut Arc::make_mut(&mut snapshot.routes)[0];
    route.logical_universe = 2;
    route.destination_universe = 9;
    snapshot.revision = 2;
    engine.replace_snapshot(snapshot).unwrap();
    clock.set(started + chrono::Duration::milliseconds(100));
    let frame = engine.prepare_output_frame(RenderOptions::default());
    assert_eq!(frame.automatic_playback_transitions().len(), 1);
    assert_eq!(engine.playback_runtime()[0].current_cue_id, Some(next_cue));
    let ordinal = engine.playback_runtime()[0].transition_ordinal;
    match supersession {
        Supersession::ClearContinuity => engine.clear_programmer_transitions(),
        Supersession::NewerCommittedFrame => {
            let newer = engine.prepare_output_frame(RenderOptions::default());
            assert!(newer.automatic_playback_transitions().is_empty());
            engine.render_prepared(&newer, &[]).unwrap();
        }
        Supersession::SamplingFailure => {}
    }
    let persistence = OutputPersistenceResource::open(&data_dir).unwrap();
    let before_events = state.events.latest_sequence();
    let attempt = || match supersession {
        Supersession::SamplingFailure => {
            let playback = state.playback.render_capability();
            output_scheduler::ordered_output_operation(&playback, || {
                let events = output_scheduler::captured_playback_events(
                    &engine,
                    &state.active_show.output_projection(),
                    &playback,
                    &frame,
                    None,
                    Some(&persistence),
                );
                light_application::PlaybackOperation::with_events(
                    Err(EngineError::Invalid(
                        "injected typed sampling failure".into(),
                    )),
                    events,
                )
            })
        }
        _ => output_scheduler::render_prepared_with_playback_events(
            &engine,
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            &frame,
            &[],
            Some(&persistence),
        ),
    };
    let result = attempt();
    assert!(
        matches!(
            (supersession, result),
            (Supersession::SamplingFailure, Err(EngineError::Invalid(_)))
                | (
                    Supersession::ClearContinuity | Supersession::NewerCommittedFrame,
                    Err(EngineError::StalePreparedFrame)
                )
        ),
        "{supersession:?}"
    );
    let events = retained_events(&state, before_events);
    let transitions = automatic_transitions(&state, before_events);
    assert_eq!(transitions.len(), 1, "{supersession:?}");
    assert_eq!(
        transitions[0].cause,
        light_application::PlaybackTransitionCause::Chaser
    );
    assert_eq!(transitions[0].cue_list_id, list_id.0);
    assert_eq!(transitions[0].transition_ordinal, ordinal);
    assert_eq!(transitions[0].advanced_steps, 1);
    assert!(
        !events.iter().any(|event| matches!(
            &event.payload,
            ApplicationEvent::Playback(PlaybackEvent::TelemetrySampled(_))
        )),
        "failed projection is not a completed output frame"
    );
    let checkpoint = state
        .installation
        .setting(&active_playbacks_setting(show.id))
        .unwrap()
        .expect("the actual automatic transition must be checkpointed despite failure");
    let persisted: Vec<light_playback::ActivePlayback> = serde_json::from_str(&checkpoint).unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].current_cue_id, Some(next_cue));
    assert_eq!(persisted[0].transition_ordinal, ordinal);

    // A retry of the same failed capture must not claim its automatic transition twice.
    let after_first_attempt = state.events.latest_sequence();
    let retried = attempt();
    assert!(
        matches!(
            (supersession, retried),
            (Supersession::SamplingFailure, Err(EngineError::Invalid(_)))
                | (
                    Supersession::ClearContinuity | Supersession::NewerCommittedFrame,
                    Err(EngineError::StalePreparedFrame)
                )
        ),
        "{supersession:?}"
    );
    assert_eq!(state.events.latest_sequence(), after_first_attempt);
    assert_eq!(automatic_transitions(&state, before_events).len(), 1);
    assert_eq!(
        state
            .installation
            .setting(&active_playbacks_setting(show.id))
            .unwrap()
            .as_deref(),
        Some(checkpoint.as_str()),
    );

    {
        let retained = control.lock();
        assert_eq!(retained.last_routes.as_ref(), old_routes.as_ref());
        assert_eq!(retained.last_frames, old_frames);
        assert_eq!(retained.last_patched_slots, old_slots);
    }
    let (dmx, still_source) = output.dmx_snapshot();
    assert_eq!(still_source.unwrap().identity(), initial_source.identity());
    assert_eq!(dmx.universes.len(), 1);
    assert_eq!(dmx.universes[0].universe, 1);
    assert_eq!(dmx.universes[0].slots, old_frames[&1].to_vec());

    // Recover at the same clock sample. The earlier transition is neither replayed nor
    // checkpointed a second time; only now may publication replace the retained output.
    let recovery = engine.prepare_output_frame(RenderOptions::default());
    assert!(recovery.automatic_playback_transitions().is_empty());
    let rendered = output_scheduler::render_prepared_with_playback_events(
        &engine,
        &state.active_show.output_projection(),
        &state.playback.render_capability(),
        &recovery,
        &[],
        Some(&persistence),
    )
    .unwrap();
    assert_eq!(automatic_transitions(&state, before_events).len(), 1);
    assert_eq!(
        state
            .installation
            .setting(&active_playbacks_setting(show.id))
            .unwrap(),
        Some(checkpoint)
    );
    let rendered = RenderedSemanticFrame::untraced(rendered, RenderOptions::default());
    output.render_frames_and_publish(&rendered, scope);
    let retained = control.lock();
    assert_eq!(
        retained.last_routes.as_ref(),
        rendered.rendered.routes.as_ref()
    );
    assert_eq!(retained.last_frames, *rendered.rendered.universes);
    assert_eq!(
        retained.last_patched_slots,
        *rendered.rendered.patched_slots
    );
    assert_eq!(retained.last_patched_slots.get(&2), Some(&25));
    assert!(!retained.last_frames.contains_key(&1));
    assert_eq!(retained.last_frames[&2][24], 191);
    assert!(output.latest_visualization_frame().unwrap().sequence > initial_source.sequence);
    drop(retained);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn cleared_prepared_frame_still_checkpoints_and_emits_its_automatic_transition_once() {
    assert_failed_capture_preserves_transition_and_retained_output(Supersession::ClearContinuity);
}

#[test]
fn superseded_prepared_frame_preserves_transition_and_retained_route_frame_slot_identity() {
    assert_failed_capture_preserves_transition_and_retained_output(
        Supersession::NewerCommittedFrame,
    );
}

struct HeldPlaybackOperation {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

impl light_application::PlaybackUnitOfWork for HeldPlaybackOperation {
    type Output = ();

    fn execute(self) -> light_application::PlaybackOperation<Self::Output> {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        light_application::PlaybackOperation::new(())
    }
}

#[test]
fn prepared_output_cannot_advance_a_cue_while_another_playback_operation_holds_the_lock() {
    let started = "2026-01-01T00:00:00Z".parse().unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let (state, data_dir) = test_state_with_clock(Arc::clone(&clock));
    let list = chaser(FixtureId::new());
    let list_id = list.id;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            cue_lists: vec![list].into(),
            ..Default::default()
        })
        .unwrap();
    state
        .output
        .execute_playback(EnginePlaybackCommand::CueList {
            id: list_id,
            action: light_engine::CueListPlaybackAction::GoAt(started),
        })
        .unwrap();
    clock.advance_millis(100);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let holding_capability = state.playback.render_capability();
    let holder = std::thread::spawn(move || {
        holding_capability.run_unit_of_work(HeldPlaybackOperation {
            entered: entered_tx,
            release: release_rx,
        });
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = std::sync::mpsc::channel();
    let output = state.output.clone();
    let active = state.active_show.output_projection();
    let capability = state.playback.render_capability();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = output.render_with_playback_events_timed(
            &active,
            &capability,
            RenderOptions::default(),
        );
        finished_tx
            .send(result.map(|(rendered, _)| rendered.rendered.sampled_at))
            .unwrap();
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    let completion_while_held = finished_rx.recv_timeout(std::time::Duration::from_millis(100));
    let cue_while_held = state.output.engine().playback_runtime()[0].cue_index;
    // Release both workers before assertions, so a regression reports instead of deadlocking.
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    worker.join().unwrap();
    assert!(matches!(
        completion_while_held,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(
        cue_while_held, 0,
        "capture must not tick Playback before entering its unit of work"
    );
    assert_eq!(
        finished_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        started + chrono::Duration::milliseconds(100)
    );
    assert_eq!(state.output.engine().playback_runtime()[0].cue_index, 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
