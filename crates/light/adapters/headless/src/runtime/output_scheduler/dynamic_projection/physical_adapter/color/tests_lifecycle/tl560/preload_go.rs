//! TL-560 matrix row "Preload GO", columns Position Target, Color Direct, UV and Zoom: the GO
//! commit itself (not a constructed `preload_active` state).
//!
//! On a fresh contract-1 desk the Programmer arms Preload and stages, as one pending batch, a
//! Point Target plus a Beam-convention Zoom on the shipped AURO SPOT Z300, a tagged Direct recipe
//! on an RGBW wash and a magenta-plus-UV Color on a live Group holding an RGBWA+UV wash. While
//! Pending nothing reaches Live. The production headless GO (`preload::commit_preload`, the
//! transaction that activates the Programmer, installs the Playback batch and persists the
//! session) then commits every value unchanged into Live, where the family frame fits and encodes
//! it. Recording that Live Preload through the real Cue writer stores the same intents, and a
//! reopened show plays them back to the same native output.
use super::position_replacement::{fit, shipped};
use super::*;
use crate::runtime::{ControlDesk, Session};
use light_core::OpeningConvention;
use light_core::programming::{PositionIntent, ScalarIntent, TargetReference};
use light_core::programming::{UvIntent, ZoomIntent};
use light_fixture::{FixtureLocation, PositionFitStatus};
use light_programmer::{
    CueRecordingCapturedSource, PreloadProgrammerValueMutation, PreloadProgrammerValueTiming,
};

const CUE: f64 = 1.0;
/// AURO SPOT Z300 17-Channel: U8 Zoom on channel 8, 10–25° Beam.
const ZOOM_CHANNEL: usize = 8;

fn session(id: SessionId) -> Session {
    Session {
        capability: light_core::SurfaceCapability::Programming,
        id,
        token: "tl560-preload".into(),
        connected: true,
        desk: ControlDesk {
            hardware_led_brightness: 100,
            hardware_gooseneck_brightness: 100,
            hardware_gooseneck_color: 100,
            id: Uuid::nil(),
            name: "TL-560 desk".into(),
            columns: 8,
            rows: 1,
            buttons: 3,
            playback_layout: None,
        },
    }
}

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

/// Index of the UV emitter channel of the RGBWA+UV builder profile.
fn uv_channel(profile: &FixtureProfile) -> u32 {
    profile.modes[0]
        .channels
        .iter()
        .position(|c| c.attribute.0.contains("uv"))
        .expect("a UV channel") as u32
}

struct Rig {
    mover: PatchedFixture,
    uv: PatchedFixture,
    direct: PatchedFixture,
}

/// The Live (post-GO or played) output of every committed family on one desk frame.
fn assert_output(desk: &Desk, rig: &Rig) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let frame = desk.frame();
    let (m, w, d) = (
        rig.mover.fixture_id,
        rig.uv.fixture_id,
        rig.direct.fixture_id,
    );
    for (owner, id) in [
        (ProgrammingOwner::Position, m),
        (ProgrammingOwner::Zoom, m),
        (ProgrammingOwner::Color, w),
        (ProgrammingOwner::Color, d),
    ] {
        assert!(
            !frame.family(owner, id).is_empty(),
            "{owner:?} on {id:?} is a family write"
        );
    }
    let mover = frame.assert_encoded(&rig.mover);
    let uv = frame.assert_encoded(&rig.uv);
    let direct = frame.assert_encoded(&rig.direct);
    // Zoom: the authored linear 10–25° Beam curve of the encoded U8 byte.
    let degrees = 10. + 15. * f64::from(mover[ZOOM_CHANNEL]) / 255.;
    assert!(
        (degrees - 17.5).abs() <= 15. / 255. / 2. + 1e-9,
        "Beam Zoom 17.5° -> {degrees}°"
    );
    // UV: the independent UV amount on the UV emitter, magenta on the visible emitters.
    let profile = rig.uv.definition.profile_snapshot.as_deref().unwrap();
    assert_eq!(
        uv[uv_channel(profile) as usize],
        (0.45f64 * 255.).round() as u32
    );
    // Direct: the exact recipe replays on its verified native layout.
    assert_eq!(&direct[1..5], [65535, 90, 0, 0]);
    // Target: fitted from the reopened or committed Point.
    let result = fit(desk, m);
    assert_eq!(
        result.achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted
    );
    (mover, uv, direct)
}

#[tokio::test]
async fn preload_go_commits_target_direct_uv_and_zoom_unchanged_into_live_and_the_recorded_show() {
    let show = Show::new();
    let (m, p, w, d) = (
        FixtureId::new(),
        FixtureId::new(),
        FixtureId::new(),
        FixtureId::new(),
    );
    let mut mover = fixture(&shipped("cameo--auro-spot-z300"), m, 1, 1);
    mover.location.z = 6000;
    let aim = super::super::super::super::position::tests::point(
        p,
        FixtureLocation {
            x: 0,
            y: 4000,
            z: 0,
        },
    );
    let uv = fixture(&rgbwauv(None), w, 2, 40);
    let rgbw_profile = rgbw();
    let direct_fixture = fixture(&rgbw_profile, d, 3, 60);
    for fixture in [&mover, &aim, &uv, &direct_fixture] {
        show.patch(fixture);
    }
    show.group(&[w]);
    let catalogue = Arc::clone(&show.compile().native_color_sources);
    let direct_value =
        super::super::super::tests_direct::direct(&catalogue, &rgbw_profile, &[65535, 90, 0, 0]);
    let rig = Rig {
        mover,
        uv,
        direct: direct_fixture,
    };

    let desk = Desk::open(show.compile());
    let operator = session(SessionId::new());
    desk.state.programming.start(operator.id);
    let active = desk
        .state
        .installation
        .upsert_show(
            "TL-560 Preload",
            &show.ports.path.display().to_string(),
            false,
        )
        .unwrap();
    desk.state.active_show.replace_current(Some(active));
    assert!(desk.state.programming.arm_preload(operator.id, true));
    let timing = PreloadProgrammerValueTiming::default();
    let staged = [
        PreloadProgrammerValueMutation::SetFixture {
            fixture_id: m,
            attribute: ProgrammingOwner::Position.key(),
            value: target(p),
            timing,
        },
        PreloadProgrammerValueMutation::SetFixture {
            fixture_id: m,
            attribute: ProgrammingOwner::Zoom.key(),
            value: zoom(),
            timing,
        },
        PreloadProgrammerValueMutation::SetFixture {
            fixture_id: d,
            attribute: ProgrammingOwner::Color.key(),
            value: direct_value.clone(),
            timing,
        },
        PreloadProgrammerValueMutation::SetGroup {
            group_id: GROUP.into(),
            attribute: ProgrammingOwner::Color.key(),
            value: magenta_with_uv(),
            timing,
        },
    ];
    assert!(desk.programmers.apply_preload_values(operator.id, &staged));
    let expected = [
        (m, ProgrammingOwner::Position, target(p)),
        (m, ProgrammingOwner::Zoom, zoom()),
        (d, ProgrammingOwner::Color, direct_value.clone()),
        (w, ProgrammingOwner::Color, magenta_with_uv()),
    ];

    // Pending: blind. Nothing is Live and no family frame writes these owners.
    let frame = desk.frame();
    for (id, owner, _) in &expected {
        assert_eq!(
            desk.played(*id, *owner),
            None,
            "{owner:?} is not Live while Pending"
        );
        assert!(frame.family(*owner, *id).is_empty());
    }

    // GO through the production headless Preload transaction.
    crate::runtime::preload::commit_preload(&desk.state, &operator).unwrap();
    let committed = desk.programmers.get(operator.id).unwrap();
    assert!(committed.preload_pending.is_empty());
    for (id, owner, value) in &expected[..3] {
        let row = committed
            .preload_active
            .iter()
            .find(|row| row.fixture_id == *id && row.attribute == owner.key())
            .unwrap_or_else(|| panic!("{owner:?} committed"));
        assert_eq!(&row.value, value, "{owner:?}: committed unchanged");
    }
    assert_eq!(
        committed.preload_group_active[GROUP][&ProgrammingOwner::Color.key()].value,
        magenta_with_uv()
    );
    desk.clock.advance_millis(600_000);
    for (id, owner, value) in &expected {
        assert_eq!(
            desk.played(*id, *owner).as_ref(),
            Some(value),
            "{owner:?} is Live after GO"
        );
    }
    let live = assert_output(&desk, &rig);

    // Record the committed Live Preload into the show through the real Cue writer.
    let programming = ProgrammingService::new(
        desk.programmers.clone(),
        EventBus::new(16),
        Arc::new(HighlightRegistry::default()),
    );
    assert_eq!(
        record_from(
            &show,
            &programming,
            operator.id,
            CUE,
            ProgrammingCueCapturePolicy::PendingOrActivePreload,
        ),
        CueRecordingCapturedSource::ActivePreload
    );
    let (_, recorded) = show.cue_list();
    assert_eq!(stored_fixture_value(&recorded, 0, m, "position"), target(p));
    assert_eq!(stored_fixture_value(&recorded, 0, m, "zoom"), zoom());
    assert_eq!(stored_fixture_value(&recorded, 0, d, "color"), direct_value);
    assert_eq!(stored_group_value(&recorded, 0, "color"), magenta_with_uv());

    // Reopen: the recorded Cue plays the same intents to the same native output.
    drop(desk);
    let reopened = Desk::open(show.compile());
    reopened.go(CUE);
    for (id, owner, value) in &expected {
        assert_eq!(
            reopened.played(*id, *owner).as_ref(),
            Some(value),
            "{owner:?}"
        );
    }
    let played = assert_output(&reopened, &rig);
    assert_eq!(played.1, live.1, "UV wash: same native output");
    assert_eq!(played.2, live.2, "Direct wash: same native output");
    assert_eq!(
        played.0[ZOOM_CHANNEL], live.0[ZOOM_CHANNEL],
        "same Zoom output"
    );
}
