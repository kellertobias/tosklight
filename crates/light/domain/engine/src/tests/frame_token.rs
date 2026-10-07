//! Captured frame tokens are identity handles: equal only for the same capture, generation,
//! sample and lane. Physical adapters rely on this to reject mixed or stale results.
use super::*;

fn engine() -> (Engine, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    registry.start(SessionId::new());
    let (patched, _) = fixture();
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            ..Default::default()
        })
        .unwrap();
    (engine, clock)
}

#[test]
fn live_tokens_identify_one_capture_and_generation() {
    let (engine, clock) = engine();
    let first = engine.prepare_output_frame(Default::default());
    let token = first.frame_token();
    assert_eq!(token, first.frame_token());
    assert_eq!(token.lane(), &CapturedFrameLane::Live);
    assert_eq!(token.generation(), first.generation());
    assert_eq!(token.sampled_at(), first.sampled_at());
    // Same clock, different capture: distinct identity even at an equal sample time.
    let same_time = engine.prepare_output_frame(Default::default());
    assert_ne!(token, same_time.frame_token());
    assert!(!token.same_capture(&same_time.frame_token()));
    clock.advance_millis(25);
    let snapshot = engine.snapshot().as_ref().clone();
    engine.replace_snapshot(snapshot).unwrap();
    let reloaded = engine.prepare_output_frame(Default::default());
    assert_ne!(reloaded.frame_token().generation(), token.generation());

    let static_frame = engine.prepare_static_family_frame(&first, &[]);
    assert!(token.matches_static_frame(&static_frame));
    assert!(!same_time.frame_token().matches_static_frame(&static_frame));
    let mut static_frame = static_frame;
    let geometry = engine
        .observe_static_family_geometry(&first, &mut static_frame)
        .unwrap();
    assert!(token.matches_geometry(&geometry));
    assert!(!reloaded.frame_token().matches_geometry(&geometry));
}

#[test]
fn preload_branch_tokens_are_distinct_from_live_and_each_other() {
    let (engine, _) = engine();
    let capture = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&capture, None);
    let state = PreloadFrameState::default();
    let before = input.frame_token(&state, PreloadBranch::BeforeRelease);
    let after = input.frame_token(&state, PreloadBranch::AfterRelease);
    assert_ne!(before, after);
    assert_ne!(before, capture.frame_token());
    assert!(before.same_capture(&capture.frame_token()));
    assert_eq!(
        before.lane().preload_branch(),
        Some(PreloadBranch::BeforeRelease)
    );
    assert_eq!(
        before,
        input.frame_token(&state, PreloadBranch::BeforeRelease)
    );
    let other_state = PreloadFrameState::default();
    assert_ne!(
        before,
        input.frame_token(&other_state, PreloadBranch::BeforeRelease)
    );
    let token = engine.prepare_preload_static_family_frame(
        &input,
        &[],
        &state,
        PreloadBranch::AfterRelease,
    );
    assert!(after.matches_static_frame(&token));
    assert!(!before.matches_static_frame(&token));
    assert!(!capture.frame_token().matches_static_frame(&token));
}
