//! TL-548 C3 on the AppState Live paths: `render_test_tick` and
//! `OutputResource::render_with_playback_events` share one opted-in lane set; show activation
//! replaces it; the default headless test state stays on the legacy path.
use super::*;
use crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint;
use crate::runtime::tests::{test_state, test_state_with_family_adapters};

type Tokens = [Option<light_engine::CapturedFrameToken>; 4];

fn tokens(state: &crate::runtime::AppState) -> Tokens {
    state
        .output
        .live_family_adapters()
        .with_lanes(|lanes| lanes.last_accepted())
}

fn every(token: &Option<light_engine::CapturedFrameToken>) -> Tokens {
    [0; 4].map(|_| token.clone())
}

fn render(state: &crate::runtime::AppState) -> RenderResult {
    state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            state.output.render_options(),
        )
        .unwrap()
        .rendered
}

#[tokio::test]
async fn both_app_state_live_paths_share_the_lanes_and_show_activation_replaces_them() {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let (state, data_dir) = test_state_with_family_adapters(
        programmers.clone(),
        Some(clock.clone()),
        PROGRAMMING_CONTRACT_VERSION,
    );
    let show = Show::new();
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: show.fixtures().into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let session = SessionId::new();
    programmers.start(session);
    program_show(&programmers, &clock, session, &show, None);
    assert_eq!(tokens(&state), every(&None));
    // OutputResource's Live path.
    clock.advance_millis(25);
    let rendered = render(&state);
    let first = tokens(&state)[0]
        .clone()
        .expect("the family path accepted a frame");
    assert_eq!(tokens(&state), every(&Some(first.clone())));
    assert_eq!(first.sampled_at(), rendered.sampled_at);
    // The bench tick renders through the same boundary and lanes (sending then fails: this
    // test state has no network output).
    clock.advance_millis(25);
    let _ = crate::runtime::output_scheduler::render_test_tick(state.clone()).await;
    let ticked = tokens(&state)[0].clone().unwrap();
    assert!(ticked.sampled_at() > first.sampled_at());
    assert_eq!(tokens(&state), every(&Some(ticked.clone())));
    // A patch edit is not an activation: continuity and lanes survive it.
    let mut edited = (*state.output.snapshot()).clone();
    edited.revision += 1;
    state.output.replace_snapshot(edited).unwrap();
    assert_eq!(tokens(&state), every(&Some(ticked)));
    // Show activation installs new lanes: nothing accepted, nothing to release.
    let mut destination = (*state.output.snapshot()).clone();
    destination.revision += 1;
    let prepared = state.output.prepare_snapshot(destination).unwrap();
    let activation = state
        .output
        .prepare_destination_activation(
            prepared,
            DynamicRuntimeSourceCheckpoint {
                runtime: Default::default(),
                origins: None,
            },
            &[],
            None,
        )
        .unwrap();
    state
        .output
        .install_destination_activation(&state.playback.render_capability(), activation)
        .unwrap();
    assert_eq!(tokens(&state), every(&None), "activation resets the lanes");
    clock.advance_millis(25);
    render(&state);
    let after = tokens(&state)[0].clone().unwrap();
    assert_eq!(tokens(&state), every(&Some(after)));
    let released = state
        .output
        .live_family_adapters()
        .with_lanes(|lanes| lanes.released());
    assert!(
        released.is_empty(),
        "new lanes carry no previous show's owners"
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn the_default_contract_one_test_state_is_not_opted_in() {
    let (state, data_dir) = test_state();
    assert!(state.output.engine().supported_programming_contract() >= PROGRAMMING_CONTRACT_VERSION);
    assert!(
        !state
            .output
            .live_family_adapters()
            .engaged(state.output.engine())
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}
