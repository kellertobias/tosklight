//! Lane admission and continuity guards over actual captured Position metadata. This test
//! deliberately does not run the native output finalizer or claim physical-output acceptance.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyProgram;

fn copied_resolution(
    source: &PhysicalResolution<PositionAdapter>,
    parked: bool,
) -> PhysicalResolution<PositionAdapter> {
    let mut writes = source.writes.clone();
    for write in &mut writes {
        write.parked = parked;
    }
    PhysicalResolution {
        writes,
        requested: source.requested.clone(),
        achieved: source.achieved.clone(),
        quality: source.quality.clone(),
        continuity: source.continuity.clone(),
    }
}

#[test]
fn held_admission_rejects_active_foreign_and_duplicate_rows_without_replacing_continuity() {
    let mut desk = EnvelopeDesk::new_configured(false, 1., false, false);
    let (_, published) = desk.tick();
    let owner = ProgrammingOwner::Position;
    let rows = desk.shared.heads.map(|target| {
        published
            .results
            .iter()
            .find(|row| row.target == target)
            .unwrap()
    });
    let previous = desk
        .shared
        .heads
        .map(|target| desk.lane.continuity(target, owner).unwrap());
    let values = desk.shared.targets.clone();
    let resolved = desk.shared.rig.resolve(&[
        (desk.shared.heads[0], values[0].clone()),
        (desk.shared.heads[1], values[1].clone()),
    ]);
    assert_eq!(resolved.results.len(), 2);
    assert!(
        resolved
            .results
            .iter()
            .all(|row| !row.writes.is_empty() && row.writes.iter().all(|write| !write.parked))
    );
    let foreign = desk.shared.rig.capture().frame_token();
    let capture = desk.shared.rig.capture();
    let token = capture.frame_token();
    assert_ne!(token, foreign);
    let mut scalar = desk
        .shared
        .rig
        .engine
        .prepare_static_family_frame(&capture, &[]);
    let geometry = desk
        .shared
        .rig
        .engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let models = DynamicRuntime::default().captured_native_color_models();
    let frame = HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: models.as_ref(),
        token: &token,
        scalar: &scalar,
    };
    desk.lane.begin(&token).unwrap();
    let mut observer = PositionFrameObserver::new(&desk.lane);
    observer.begin_frame(&token).unwrap();
    let programs = desk
        .shared
        .heads
        .iter()
        .zip(&values)
        .map(|(&target, base)| HybridFamilyProgram {
            target,
            owner,
            base,
            samples: &[],
            has_requirements: true,
            frame,
        })
        .collect::<Vec<_>>();
    observer.prepare_programs(frame, &programs).unwrap();
    let stage = |index: usize, staged_token: &CapturedFrameToken, parked: bool, held: bool| {
        let row = rows[index];
        let result = copied_resolution(&resolved.results[index], parked);
        if held {
            desk.lane.stage_held_resolution(
                staged_token,
                row.target,
                owner,
                values[index].clone(),
                row.provenance.clone(),
                row.metadata.clone(),
                result,
            )
        } else {
            desk.lane.stage_resolution(
                staged_token,
                row.target,
                owner,
                values[index].clone(),
                row.provenance.clone(),
                row.metadata.clone(),
                result,
            )
        }
    };
    assert!(
        stage(0, &token, false, true).is_err(),
        "an active native claim is never a held result"
    );
    assert!(
        stage(0, &foreign, true, true).is_err(),
        "all-parked results still need the exact staged token"
    );
    let (_, held) = stage(0, &token, true, true).unwrap();
    assert_eq!(held.token, token);
    assert!(held.writes.iter().all(|write| write.parked));
    assert!(
        stage(0, &token, true, true).is_err(),
        "held ownership is consumed exactly once"
    );
    assert!(
        stage(0, &token, false, false).is_err(),
        "a produced row cannot duplicate a held owner"
    );
    stage(1, &token, true, true).unwrap();
    desk.lane.verify(&token).unwrap();
    assert!(desk.lane.accept(&token));
    assert_eq!(desk.lane.last_accepted(), Some(token));
    assert!(desk.lane.released().is_empty());
    for (index, target) in desk.shared.heads.iter().enumerate() {
        assert_eq!(
            desk.lane.continuity(*target, owner).unwrap(),
            previous[index],
            "holding native claims retains the last actually accepted solver continuity"
        );
    }
}
