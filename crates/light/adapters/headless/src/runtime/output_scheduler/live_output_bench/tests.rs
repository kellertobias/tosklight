//! TL-596: the benchmark seam drives the production Live transaction: frames take the hybrid
//! path, an unchanged static Position converges to memo reuse, and readout consumers of the
//! published frames never add physical solves.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position::tests::{
    angles, moving_head, patched,
};
use light_core::programming::{PROGRAMMING_CONTRACT_VERSION, ProgrammingOwner};
use light_core::{FixtureId, ManualClock, SessionId};
use light_programmer::ProgrammerRegistry;

fn seam() -> (LiveOutputBench, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    programmers.start(session);
    let engine = Arc::new(Engine::with_programming_contract_support(
        programmers.clone(),
        PROGRAMMING_CONTRACT_VERSION,
    ));
    let mover = FixtureId::new();
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![patched(&moving_head(), mover, 1)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    programmers.set(
        session,
        mover,
        ProgrammingOwner::Position.key(),
        angles(30., 40.),
    );
    let runtime = light_dynamics::DynamicRuntime::with_programming_contract_support(
        PROGRAMMING_CONTRACT_VERSION,
    );
    let bench = LiveOutputBench::new(engine, runtime, 60, true).unwrap();
    (bench, clock)
}

fn frames(bench: &LiveOutputBench, clock: &ManualClock, count: usize) -> Vec<LiveOutputFrame> {
    (0..count)
        .map(|_| {
            clock.advance_millis(16);
            let frame = bench.render(RenderOptions::default(), &[]).unwrap();
            std::thread::sleep(Duration::from_millis(2));
            frame
        })
        .collect()
}

#[test]
fn the_seam_takes_the_hybrid_path_and_an_unchanged_position_converges_to_reuse() {
    let (bench, clock) = seam();
    assert!(bench.family_engaged());
    let frames = frames(&bench, &clock, 16);
    assert!(frames.iter().all(|frame| frame.hybrid));
    assert_eq!(frames[0].work.position_fits, 1);
    assert_eq!(
        frames[15].work.position_compiles, 1,
        "one descriptor, never recompiled"
    );
    assert_eq!(
        frames[15].work.position_fits, frames[12].work.position_fits,
        "a converged static Position is reused, not re-solved"
    );
    assert!(frames[15].work.position_fit_cache_hits > frames[12].work.position_fit_cache_hits);
    assert!(
        frames
            .iter()
            .all(|frame| frame.rendered.generation == frames[0].rendered.generation)
    );
}

#[test]
fn readout_consumers_read_published_frames_without_adding_solves() {
    let (quiet, quiet_clock) = seam();
    let (read, read_clock) = seam();
    let consumers = read.spawn_readout_consumers(3, Duration::from_millis(1));
    let quiet = frames(&quiet, &quiet_clock, 12);
    let read = frames(&read, &read_clock, 12);
    let report = consumers.finish();
    assert_eq!(report.consumers, 3);
    assert!(report.frames_read > 0, "{report:?}");
    let work = |frames: &[LiveOutputFrame]| {
        let last = frames.last().unwrap().work;
        (
            last.position_fits + last.position_fit_cache_hits,
            last.position_compiles,
        )
    };
    assert_eq!(work(&quiet), work(&read));
}
