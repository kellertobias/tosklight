//! TL-641: where a Live frame spends the Dynamic start path.
//!
//! The Live transaction records, per thread, how long reconciliation of the captured Dynamic
//! sources took and how much of that was controller construction (starting a runtime instance,
//! selecting its lanes and binding its target scope). Only `LiveOutputBench` reads these; the
//! production scheduler never takes them, so they are overwritten by the next measured frame.
//! Recording is two clock reads per reconciliation and per reconciled controller.
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

/// Cumulative start-path cost of the frames rendered on this thread since the last `take`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StartPathPhases {
    /// Programmer, Cue and Playback reconciliation, including controller construction.
    pub reconcile: Duration,
    /// Starting, re-scoping and lane selection of reconciled controllers.
    pub controller_construction: Duration,
}

thread_local! {
    static PHASES: Cell<StartPathPhases> = const {
        Cell::new(StartPathPhases {
            reconcile: Duration::ZERO,
            controller_construction: Duration::ZERO,
        })
    };
}

fn record(update: impl FnOnce(&mut StartPathPhases)) {
    PHASES.with(|phases| {
        let mut current = phases.get();
        update(&mut current);
        phases.set(current);
    });
}

/// Add one reconciliation pass that began at `started`.
pub(in crate::runtime) fn reconciled_since(started: Instant) {
    let elapsed = started.elapsed();
    record(|phases| phases.reconcile += elapsed);
}

pub(in crate::runtime) fn controller_construction<T>(work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let output = work();
    let elapsed = started.elapsed();
    record(|phases| phases.controller_construction += elapsed);
    output
}

/// Return and reset this thread's accumulated phases.
pub(in crate::runtime) fn take() -> StartPathPhases {
    PHASES.with(|phases| phases.replace(StartPathPhases::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_accumulate_per_thread_and_reset_on_take() {
        let _ = take();
        let started = Instant::now();
        controller_construction(|| std::thread::sleep(Duration::from_millis(2)));
        reconciled_since(started);
        let phases = take();
        assert!(phases.controller_construction >= Duration::from_millis(2));
        assert!(phases.reconcile >= phases.controller_construction);
        assert_eq!(take(), StartPathPhases::default());
        std::thread::spawn(|| reconciled_since(Instant::now()))
            .join()
            .unwrap();
        assert_eq!(take(), StartPathPhases::default());
    }
}
