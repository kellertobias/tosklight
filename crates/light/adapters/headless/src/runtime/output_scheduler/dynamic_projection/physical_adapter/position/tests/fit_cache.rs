//! Accepted cohort identity is stronger than numerical equality of independently fitted poses.
use super::current_cohort::shared_rig;
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::{
    HybridFamilyProgram, HybridFamilyRequirementReason,
};

#[test]
fn shared_axis_fit_reuse_requires_every_peers_same_accepted_memo() {
    let shared = shared_rig();
    let rig = &shared.rig;
    let values = shared.angles.map(|pair| angles(pair[0], pair[1]));
    for (head, value) in shared.heads.iter().zip(&values) {
        rig.programmers.set(
            rig.session,
            *head,
            ProgrammingOwner::Position.key(),
            value.clone(),
        );
    }
    let lanes = [
        PhysicalAdapterLane::live(PositionAdapter::default()),
        PhysicalAdapterLane::live(PositionAdapter::default()),
    ];
    let mut runtimes = std::array::from_fn::<_, 2, _>(|_| {
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION)
    });
    let mut origins = [
        DynamicSourceOrigins::default(),
        DynamicSourceOrigins::default(),
    ];
    let mut scratches = [HybridFrameScratch::default(), HybridFrameScratch::default()];
    let mut warmed = false;
    for _ in 0..8 {
        let mut hits = 0;
        for index in 0..2 {
            // Each finalization owns a fresh frame; identical solver inputs do not
            // authorize finalizing one captured engine frame twice.
            let capture = rig.capture();
            let output = prepare_live(
                rig,
                &capture,
                &capture,
                &lanes[index],
                &mut runtimes[index],
                &mut origins[index],
                &mut scratches[index],
            )
            .unwrap();
            assert!(output.requirements.is_empty());
            assert_eq!(output.results.len(), 2);
            for row in &output.results {
                assert!(!row.quality.held);
                let value = &values[shared
                    .heads
                    .iter()
                    .position(|head| *head == row.target)
                    .unwrap()];
                assert_eq!(&row.value, value);
                assert_eq!(row.requested, *intent(value).unwrap());
                let physical = output
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == rig.root.0)
                    .unwrap();
                for write in &row.writes {
                    assert_eq!(
                        physical.native_raw[write.slot.channel_index as usize],
                        write.raw
                    );
                }
                hits += row.quality.reused_fits;
            }
        }
        if hits == 4 {
            warmed = true;
            break;
        }
    }
    assert!(
        warmed,
        "both independent lanes must reach an accepted stable shared fit"
    );
    let previous = shared.heads.map(|head| {
        lanes[0]
            .continuity(head, ProgrammingOwner::Position)
            .unwrap()
    });
    let foreign = lanes[1]
        .continuity(shared.heads[1], ProgrammingOwner::Position)
        .unwrap();
    let common = previous[0].instances[0].fit_memo.as_ref().unwrap();
    assert!(Arc::ptr_eq(
        common,
        previous[1].instances[0].fit_memo.as_ref().unwrap()
    ));
    let independently_accepted = foreign.instances[0].fit_memo.as_ref().unwrap();
    assert_eq!(
        common, independently_accepted,
        "same real solver inputs and answers"
    );
    assert!(
        !Arc::ptr_eq(common, independently_accepted),
        "independent lanes cannot confer shared acceptance"
    );

    let capture = rig.capture();
    let token = capture.frame_token();
    let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
    let geometry = rig
        .engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let models = DynamicRuntime::default().captured_native_color_models();
    let frame = HybridFrameContext {
        capture: &capture,
        token: &token,
        scalar: &scalar,
        geometry: &geometry,
        native_models: models.as_ref(),
    };
    let descriptors = shared.heads.map(|head| {
        lanes[0]
            .adapter()
            .compile(&capture.snapshot(), head)
            .unwrap()
            .unwrap()
    });
    let mut expected_writes = None;
    for mode in 0..3 {
        let mut candidate = previous.clone();
        match mode {
            0 => {}
            1 => candidate[1].instances[0].fit_memo = None,
            2 => candidate[1].instances[0].fit_memo = Some(Arc::clone(independently_accepted)),
            _ => unreachable!(),
        }
        let requests = (0..2)
            .map(|index| PhysicalRequest {
                frame,
                target: shared.heads[index],
                owner: ProgrammingOwner::Position,
                descriptor: &descriptors[index],
                value: &values[index],
                previous: Some(&candidate[index]),
            })
            .collect::<Vec<_>>();
        let before = lanes[0].adapter().counters();
        let resolved = lanes[0].adapter().resolve_cohort(&requests, &[]).unwrap();
        let after = lanes[0].adapter().counters();
        assert_eq!(
            after.fit_cache_hits - before.fit_cache_hits,
            u64::from(mode == 0)
        );
        assert_eq!(after.fits - before.fits, u64::from(mode != 0));
        let writes = resolved
            .iter()
            .map(|result| result.writes.clone())
            .collect::<Vec<_>>();
        if let Some(expected) = &expected_writes {
            assert_eq!(&writes, expected);
        } else {
            expected_writes = Some(writes);
        }
        for (index, result) in resolved.iter().enumerate() {
            assert_eq!(result.requested, *intent(&values[index]).unwrap());
            assert!(!result.quality.held);
            assert_eq!(result.quality.reused_fits, usize::from(mode == 0));
        }
        assert_eq!(
            lanes[0]
                .continuity(shared.heads[0], ProgrammingOwner::Position)
                .unwrap(),
            previous[0],
            "speculative resolve cannot publish new continuity"
        );
        assert_eq!(
            lanes[0]
                .continuity(shared.heads[1], ProgrammingOwner::Position)
                .unwrap(),
            previous[1]
        );
    }
}

#[test]
fn unheaded_root_requirement_protects_root_and_copy_through_real_observer_finish() {
    let (rig, _, copy) = super::root_emitter::root_emitter_rig(false);
    let initial = angles(15., 25.);
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        initial,
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let capture = rig.capture();
    let mut published = prepare_live(
        &rig,
        &capture,
        &capture,
        &lane,
        &mut runtime,
        &mut origins,
        &mut scratch,
    )
    .unwrap();
    let previous = lane
        .continuity(rig.root, ProgrammingOwner::Position)
        .unwrap();
    let mut sidecar = published.results.remove(0);
    let value = angles(90., 60.);
    let capture = rig.capture();
    let token = capture.frame_token();
    let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
    let geometry = rig
        .engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let models = DynamicRuntime::default().captured_native_color_models();
    let frame = HybridFrameContext {
        capture: &capture,
        token: &token,
        scalar: &scalar,
        geometry: &geometry,
        native_models: models.as_ref(),
    };
    assert!(profile_head_destinations(&capture.snapshot(), rig.root).is_empty());
    lane.begin(&token).unwrap();
    let mut observer = PositionFrameObserver::new(&lane);
    observer.begin_frame(&token).unwrap();
    observer
        .prepare_programs(
            frame,
            &[HybridFamilyProgram {
                target: rig.root,
                owner: ProgrammingOwner::Position,
                base: &value,
                samples: &[],
                has_requirements: true,
                frame,
            }],
        )
        .unwrap();
    let descriptor = lane
        .descriptor(frame, rig.root, ProgrammingOwner::Position)
        .unwrap();
    observer.pending.push(PendingPosition {
        target: rig.root,
        descriptor,
        previous: Some(previous.clone()),
        program: None,
        destinations: Vec::new(),
    });
    sidecar.token = token.clone();
    sidecar.value = value.clone();
    let mut rows = vec![OwnedHybridProjection {
        target: rig.root,
        owner: ProgrammingOwner::Position,
        value: value.clone(),
        metadata: sidecar.metadata.clone(),
        sidecar,
    }];
    let requirements = [HybridFamilyRequirement {
        target: rig.root,
        owner: ProgrammingOwner::Position,
        reason: HybridFamilyRequirementReason::Composition(TransitionRequirement::LiveTargetPoints),
    }];
    observer.finish(frame, &mut rows, &requirements).unwrap();
    let row = rows.remove(0).sidecar;
    assert_eq!(row.value, value);
    assert_eq!(row.requested, *intent(&value).unwrap());
    assert!(
        row.quality.held,
        "root requirements must not disappear merely because channel heads are logical"
    );
    assert_eq!(row.quality.reused_fits, 0);
    assert_eq!(row.achieved.outcomes.len(), 2);
    assert!(row.writes.iter().all(|write| write.parked));
    for destination in [rig.root, copy] {
        assert_eq!(
            row.writes
                .iter()
                .filter(|write| write.slot.destination == destination)
                .count(),
            2
        );
    }
    observer
        .project_native(&capture, &token, &mut scalar, &[row])
        .unwrap();
    lane.verify(&token).unwrap();
    let rendered = rig
        .engine
        .render_static_family_frame(&capture, scalar)
        .unwrap();
    for instance in &rendered.physical.instances {
        let saved = previous
            .instances
            .iter()
            .find(|saved| saved.destination.0 == instance.instance_id)
            .unwrap();
        for &(index, _, _, raw) in &saved.controls {
            assert_eq!(
                instance.native_raw[index as usize], raw,
                "protected root and copied body keep accepted physical pose"
            );
        }
    }
    assert!(lane.accept(&token));
}
