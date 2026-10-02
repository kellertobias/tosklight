//! The C3/C4 gate on a real `AppState`: nothing starts unless the engine supports the
//! programming contract AND the explicit family-adapter opt-in is set; then the worker starts,
//! installs its Position readout source and follows the desk's Preload lifecycle hooks.
use super::*;
use crate::runtime::tests::{test_state, test_state_with_family_adapters};
use light_core::SessionId;

#[test]
fn with_the_gate_off_nothing_starts_and_every_hook_is_inert() {
    for (case, (state, _directory)) in [
        ("contract without opt-in", test_state()),
        (
            "opt-in without contract",
            test_state_with_family_adapters(ProgrammerRegistry::default(), None, 0),
        ),
    ] {
        assert!(!PendingEpisodeResource::gated(&state), "{case}");
        let pending = state.programming.pending_episodes();
        assert!(!pending.start_if_gated(&state), "{case}");
        for trigger in [
            PendingTrigger::Go,
            PendingTrigger::Clear,
            PendingTrigger::Reload,
        ] {
            pending.trigger(trigger);
        }
        pending.wake();
        assert!(pending.executor().is_none(), "{case}: no worker");
        assert!(pending.status().is_none(), "{case}");
        assert!(
            state.output.pending_position_readouts().source().is_none(),
            "{case}: no Pending readout source"
        );
        assert!(
            state
                .output
                .pending_episode_handles()
                .publication
                .input_capture_cursor()
                .is_none(),
            "{case}: Live never began retaining Pending history"
        );
    }
}

#[test]
fn with_the_gate_on_the_worker_starts_installs_its_source_and_follows_preload() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (state, _directory) =
        test_state_with_family_adapters(programmers.clone(), None, PROGRAMMING_CONTRACT_VERSION);
    assert!(PendingEpisodeResource::gated(&state));
    let pending = state.programming.pending_episodes();
    assert!(pending.start_if_gated(&state));
    assert!(pending.start_if_gated(&state), "idempotent");
    let executor = pending.executor().unwrap();
    assert!(state.output.pending_position_readouts().source().is_some());
    let idle = |status: &PendingEpisodeStatus| *status == PendingEpisodeStatus::Idle;
    assert!(executor.wait_for(PATIENCE, |progress, status| {
        progress.steps > 0 && idle(status)
    }));

    // Arming Preload engages Pending (Running, or Waiting without an active show).
    assert!(programmers.arm_preload(session, true));
    assert!(
        executor.wait_for(PATIENCE, |_, status| !idle(status)),
        "{:?}",
        executor.status()
    );
    // The desk release path ends it through the ProgrammingResource hook.
    assert!(state.programming.release_preload(session));
    assert!(executor.wait_for(PATIENCE, |_, status| idle(status)));
    assert!(executor.latest().is_none());

    drop(executor);
    pending.stop(&state);
    assert!(pending.executor().is_none());
    assert!(state.output.pending_position_readouts().source().is_none());
}
