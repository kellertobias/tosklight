//! TL-560 matrix row "Unpatched fixture", columns Position Target, Color Direct, UV, Focus, Zoom
//! and Media color (the Angles and semantic Color cells are `unpatched`).
//!
//! AGENTS operator semantics: an unpatched fixture remains part of the show. It is programmed,
//! stored in Cues and displayed; only DMX output is suppressed until it is patched again, and
//! re-patching restores output from the same stored intent.
//!
//! Every fixture of the rig is unpatched (no universe/address): the shipped AURO SPOT Z300 gets a
//! Point Target plus a Beam Zoom, an RGBW wash a tagged Direct recipe, an RGBWA+UV wash
//! magenta with independent UV, Wash A a Focus, and layer 1 of the shipped Media Server a Media
//! colour. One Cue is recorded through the real Cue writer and the SQLite show is reopened. On a
//! fresh contract-1 desk the played owners equal the stored intent and every fixture keeps a
//! physical forward instance with its fitted native command, but no universe is output. After
//! re-patching every fixture without a Cue rewrite, the same native command is encoded on the
//! wire.
use super::position_replacement::{fit, shipped};
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::media_color::tests::{
    media_fixture, shipped_media_server,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::wash_a;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::focus;
use light_core::OpeningConvention;
use light_core::programming::{
    PositionIntent, ScalarIntent, TargetReference, UvIntent, ZoomIntent,
};
use light_fixture::{FixtureLocation, PositionFitStatus};

const CUE: f64 = 1.0;

fn target(point: FixtureId) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id: point.0 },
        [0.5, -1.25, 2.0],
    )))
}

fn zoom() -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(17.5),
        convention: OpeningConvention::Beam,
    }))
}

fn magenta_with_uv() -> AttributeValue {
    program(&ColorIntent {
        uv: UvIntent { amount: 0.45 },
        ..magenta()
    })
}

fn media_amber() -> AttributeValue {
    program(&ColorIntent {
        white_blend: 0.25,
        ..super::super::super::super::tests::intent([1., 0.735, 0.], 0.)
    })
}

fn unpatch(mut fixture: PatchedFixture) -> PatchedFixture {
    fixture.universe = None;
    fixture.address = None;
    fixture
}

#[tokio::test]
async fn unpatched_target_direct_uv_focus_zoom_and_media_keep_their_intent_and_only_lose_dmx() {
    let show = Show::new();
    let ids: [FixtureId; 7] = std::array::from_fn(|_| FixtureId::new());
    let [m, p, d, w, o, root, layer] = ids;
    let mut mover = fixture(&shipped("cameo--auro-spot-z300"), m, 1, 1);
    mover.location.z = 6000;
    let aim = super::super::super::super::super::position::tests::point(
        p,
        FixtureLocation {
            x: 0,
            y: 4000,
            z: 0,
        },
    );
    let rgbw_profile = rgbw();
    let mut media = media_fixture(&shipped_media_server(), root, &[layer, FixtureId::new()]);
    media.fixture_number = Some(5);
    media.address = Some(200);
    media.universe = Some(2);
    let patched = [
        mover,
        fixture(&rgbw_profile, d, 2, 40),
        fixture(&rgbwauv(None), w, 3, 60),
        fixture(&wash_a(), o, 4, 80),
        media,
    ];
    show.patch(&aim);
    for fixture in &patched {
        show.patch(&unpatch(fixture.clone()));
    }
    let catalogue = Arc::clone(&show.compile().native_color_sources);
    let direct_value =
        super::super::super::direct::direct(&catalogue, &rgbw_profile, &[65535, 90, 0, 0]);
    let expected = [
        (m, ProgrammingOwner::Position, target(p)),
        (m, ProgrammingOwner::Zoom, zoom()),
        (d, ProgrammingOwner::Color, direct_value),
        (w, ProgrammingOwner::Color, magenta_with_uv()),
        (o, ProgrammingOwner::Focus, focus(0.37)),
        (layer, ProgrammingOwner::Color, media_amber()),
    ];
    for (id, owner, value) in &expected {
        show.programmers
            .set(show.session, *id, owner.key(), value.clone());
    }
    show.record(CUE);

    // Stored like any patched fixture.
    let (list_id, recorded) = show.cue_list();
    for (id, owner, value) in &expected {
        assert_eq!(
            &stored_fixture_value(&recorded, 0, *id, &owner.key().0),
            value,
            "{owner:?} on an unpatched fixture is recorded exactly"
        );
    }
    let snapshot = show.compile();
    for fixture in &patched {
        let compiled = snapshot
            .fixtures
            .iter()
            .find(|f| f.fixture_id == fixture.fixture_id)
            .expect("an unpatched fixture is still a show fixture");
        assert_eq!((compiled.universe, compiled.address), (None, None));
    }

    // Reopened desk: every owner plays and is fitted for display; nothing reaches a universe.
    let desk = Desk::open(snapshot);
    desk.go(CUE);
    for (id, owner, value) in &expected {
        assert_eq!(
            desk.played(*id, *owner).as_ref(),
            Some(value),
            "{owner:?} plays on the unpatched fixture"
        );
    }
    assert_eq!(
        fit(&desk, m).achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted,
        "the unpatched mover's Point Target is still fitted"
    );
    let frame = desk.frame();
    assert!(
        frame
            .rendered
            .universes
            .values()
            .all(|universe| universe.iter().all(|byte| *byte == 0)),
        "no byte of an unpatched fixture is output"
    );
    let dormant: Vec<Vec<u32>> = patched
        .iter()
        .map(|fixture| frame.native(fixture.fixture_id).to_vec())
        .collect();
    drop(desk);

    // Re-patch every fixture: the same stored Cue now reaches the wire.
    for fixture in &patched {
        show.patch(fixture);
    }
    assert_eq!(show.cue_list(), (list_id, recorded), "no Cue rewrite");
    let desk = Desk::open(show.compile());
    desk.go(CUE);
    for (id, owner, value) in &expected {
        assert_eq!(desk.played(*id, *owner).as_ref(), Some(value), "{owner:?}");
    }
    let frame = desk.frame();
    for (fixture, dormant) in patched.iter().zip(&dormant) {
        let native = frame.assert_encoded(fixture);
        assert_eq!(
            &native, dormant,
            "{}: the re-patched output is the native command fitted while unpatched",
            fixture.name
        );
    }
    // Focus stays on the scalar path (see `focus`); every other owner is a family write.
    for (id, owner, _) in expected
        .iter()
        .filter(|(_, o, _)| *o != ProgrammingOwner::Focus)
    {
        let destination = if *id == layer { root } else { *id };
        assert!(
            !frame.family(*owner, *id).is_empty() || !frame.family(*owner, destination).is_empty(),
            "{owner:?} on {id:?} is a family write after re-patching"
        );
    }
}
