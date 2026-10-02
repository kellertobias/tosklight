//! Shared-control agreement between owners of one family, and the Preload ownership mask of
//! Dynamic-only Pending native writes (TL-610 finding, fixed in TL-548 C2).
use super::*;
use crate::{PreloadBranch, PreloadFrameState};
use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};
use light_fixture::FixtureHead;

/// Two heads; the first is master-shared and carries the only Zoom control, which the second
/// (logical) head inherits. The root and the logical owner therefore own that one control.
fn zoom_pair() -> (PatchedFixture, [FixtureId; 2]) {
    let (mut fixture, _) = schema_v2_fixture(&[
        ("zoom", false, false, false, false, false),
        ("intensity", false, false, false, false, false),
    ]);
    fixture.address = Some(60);
    redefine(&mut fixture, |profile| {
        let mode = &mut profile.modes[0];
        mode.heads[0].master_shared = true;
        let cell = FixtureHead {
            id: Uuid::new_v4(),
            name: "Cell".into(),
            master_shared: false,
        };
        mode.channels[1].head_id = cell.id;
        mode.heads.push(cell);
    });
    // A master-shared head stays with the physical root; the cell head is a logical owner.
    let cell = FixtureId::new();
    let head = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0].heads[1];
    fixture.logical_heads = vec![PatchedHead {
        profile_head_id: Some(head.id),
        fixture_id: cell,
        head_index: fixture.definition.heads[1].index,
    }];
    let owners = [fixture.fixture_id, cell];
    (fixture, owners)
}

#[test]
fn same_family_owners_may_share_an_agreeing_control_and_reject_a_disagreeing_one() {
    let (fixture, owners) = zoom_pair();
    let (engine, programmers, session) = engine(fixture.clone());
    for owner in owners {
        set(&programmers, session, owner, "zoom", 0.5);
    }
    let writes = |first: u32, second: u32| -> Vec<FamilyNativeWrite> {
        owners
            .iter()
            .zip([first, second])
            .flat_map(|(&owner, raw)| {
                family_writes(&fixture, owner, ProgrammingOwner::Zoom, &[(0, raw)])
            })
            .collect()
    };
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut frame = engine.prepare_static_family_frame(&capture, &[]);
    assert!(
        frame
            .project_family_native(&capture, &token, &writes(90, 91))
            .is_err(),
        "owners disagree on the shared Zoom control"
    );
    frame
        .project_family_native(&capture, &token, &writes(90, 90))
        .unwrap();
    let output = engine.render_static_family_frame(&capture, frame).unwrap();
    assert_eq!(output.universes[&1][59], 90);
    assert_eq!(native_raw(&output, fixture.fixture_id.0)[0], 90);
}

/// Live static Position on a mover with a copy, then an armed Preload whose only Pending value
/// is Dynamic-sourced Position on that mover (when `pending`).
fn pending_mover(pending: bool) -> (Engine, PatchedFixture) {
    let mut fixture = mover();
    copy_at(&mut fixture, 10);
    let (engine, programmers, session) = engine(fixture.clone());
    let position =
        |pan, tilt| AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)));
    programmers.set(
        session,
        fixture.fixture_id,
        ProgrammingOwner::Position.key(),
        position(0., 0.),
    );
    programmers.arm_preload(session, true);
    if pending {
        assert!(programmers.apply_dynamic_values(
            session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: fixture.fixture_id,
                attribute: ProgrammingOwner::Position.key(),
                value: DynamicSemanticValue::Static {
                    value: position(30., 40.),
                    timing: DynamicValueTiming::default(),
                },
            }],
            None,
        ));
    }
    (engine, fixture)
}

fn pending_mask(pending: bool) -> Option<Box<[bool]>> {
    let (engine, fixture) = pending_mover(pending);
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let mut state = PreloadFrameState::default();
    let mut after = engine.prepare_preload_static_family_frame(
        &input,
        &[],
        &state,
        PreloadBranch::AfterRelease,
    );
    let writes = family_writes(
        &fixture,
        fixture.fixture_id,
        ProgrammingOwner::Position,
        &[(0, 0x1234), (1, 0xabcd)],
    );
    after
        .project_family_native(
            &frame,
            &input.frame_token(&state, PreloadBranch::AfterRelease),
            &writes,
        )
        .unwrap();
    let rendered = engine
        .render_prepared_preload_families(&input, None, after, &mut state)
        .unwrap();
    for instance in instances(&fixture) {
        let row = rendered
            .projection
            .physical
            .instances
            .iter()
            .find(|row| row.instance_id == instance)
            .unwrap();
        assert_eq!(&row.native_raw[..2], &[0x1234, 0xabcd]);
    }
    rendered
        .projection
        .native_ownership
        .get(&fixture.fixture_id)
        .cloned()
}

#[test]
fn dynamic_only_pending_position_owns_its_fitted_native_controls() {
    assert_eq!(
        pending_mask(true).as_deref(),
        Some(&[true, true][..]),
        "TL-610: a Dynamic-only Pending Position must carry an ownership mask"
    );
    assert_eq!(
        pending_mask(false),
        None,
        "a Live-only native write is not Pending output"
    );
}
