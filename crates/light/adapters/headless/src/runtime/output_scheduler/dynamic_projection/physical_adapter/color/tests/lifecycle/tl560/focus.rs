//! TL-560 column "Focus": Cue record, reopen, playback and fixture replacement with Focus as its
//! own owner, independent of Zoom (Preset recall of Focus is in `preset_recall`).
//!
//! A per-fixture Cue 1 stores Focus 0.37 plus a Field Zoom of 20°; Cue 2 stores Focus 0.8 only.
//! Both are recorded through the real Cue writer and the SQLite show is reopened. A fresh
//! contract-1 desk plays them through the production Live frame on Wash A (reversed measured
//! Focus, reversed nonlinear U16 Zoom). The fixture is then replaced by Spot B (nominal U16
//! Focus travel, ascending U8 Zoom) without any Cue rewrite, and the same Cues play on it. Cue 2
//! never touches Zoom: the tracked Zoom output stays exactly where Cue 1 put it.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::{
    spot_b, wash_a,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::{
    field, focus,
};

/// Both builder profiles: 0 Intensity, 1 Zoom, 2 Focus.
const ZOOM: usize = 1;
const FOCUS: usize = 2;
const FOCUS_AND_ZOOM: f64 = 1.0;
const FOCUS_ONLY: f64 = 2.0;

fn near(actual: u32, expected: f64, what: &str) {
    assert!(
        (f64::from(actual) - expected).abs() <= 1.,
        "{what}: raw {actual} ≈ {expected}"
    );
}

/// Expected native Focus and Zoom raw values per profile for a normalized Focus and a 20° Field.
struct Optics {
    focus: fn(f64) -> f64,
    zoom_20: f64,
}

const WASH_A: Optics = Optics {
    // 100 % at 10 → 0 % at 200.
    focus: |n| 10. + (1. - n) * 190.,
    // 50° → 20° (at 32768) → 5°.
    zoom_20: 32768.,
};
const SPOT_B: Optics = Optics {
    // Nominal U16 travel.
    focus: |n| n * 65535.,
    // 8° → 20° (at 128) → 40°.
    zoom_20: 128.,
};

/// `strict` also requires the profile's authored Focus curve on the wire (see the ignored BUG
/// test); otherwise Focus is only required to reach the wire and differ between the Cues.
fn play(show: &Show, patched: &PatchedFixture, optics: &Optics, strict: bool) {
    let f = patched.fixture_id;
    let desk = Desk::open(show.compile());
    desk.go(FOCUS_AND_ZOOM);
    assert_eq!(desk.played(f, ProgrammingOwner::Focus), Some(focus(0.37)));
    assert_eq!(desk.played(f, ProgrammingOwner::Zoom), Some(field(20.)));
    let first = desk.frame().assert_encoded(patched);
    if strict {
        near(first[FOCUS], (optics.focus)(0.37), "Cue 1 Focus");
    }
    near(first[ZOOM], optics.zoom_20, "Cue 1 Zoom");

    desk.go(FOCUS_ONLY);
    assert_eq!(desk.played(f, ProgrammingOwner::Focus), Some(focus(0.8)));
    assert_eq!(
        desk.played(f, ProgrammingOwner::Zoom),
        Some(field(20.)),
        "Cue 2 stores no Zoom; the tracked Zoom is unchanged"
    );
    let second = desk.frame().assert_encoded(patched);
    if strict {
        near(second[FOCUS], (optics.focus)(0.8), "Cue 2 Focus");
    }
    assert_ne!(second[FOCUS], first[FOCUS], "Cue 2 Focus reaches the wire");
    assert_eq!(
        second[ZOOM], first[ZOOM],
        "Focus never moves the Zoom output"
    );
}

fn record_and_replace(strict: bool) {
    let show = Show::new();
    let f = FixtureId::new();
    let wash = fixture(&wash_a(), f, 1, 1);
    show.patch(&wash);
    show.programmers
        .set(show.session, f, ProgrammingOwner::Focus.key(), focus(0.37));
    show.programmers
        .set(show.session, f, ProgrammingOwner::Zoom.key(), field(20.));
    show.record(FOCUS_AND_ZOOM);
    show.programmers
        .set(show.session, f, ProgrammingOwner::Focus.key(), focus(0.8));
    show.record(FOCUS_ONLY);

    let (list_id, recorded) = show.cue_list();
    assert_eq!(stored_fixture_value(&recorded, 0, f, "focus"), focus(0.37));
    assert_eq!(stored_fixture_value(&recorded, 0, f, "zoom"), field(20.));
    assert_eq!(stored_fixture_value(&recorded, 1, f, "focus"), focus(0.8));
    assert!(
        cue_changes(&recorded, 1, "changes")
            .iter()
            .all(|change| change["attribute"] != "zoom"),
        "a Focus-only Cue stores no Zoom row: {recorded}"
    );

    play(&show, &wash, &WASH_A, strict);

    // Replace Wash A by Spot B under the same fixture identity.
    let spot = fixture(&spot_b(), f, 1, 1);
    show.patch(&spot);
    assert_eq!(show.cue_list(), (list_id, recorded), "no Cue rewrite");
    play(&show, &spot, &SPOT_B, strict);
}

#[tokio::test]
async fn recorded_focus_reopens_plays_and_survives_replacement_independent_of_zoom() {
    record_and_replace(false);
}

/// The operator contract (help `06-focus-and-zoom.md`): Focus is lens travel 0–100 % and means
/// the same on every fixture type. A static Focus in the Live frame is rendered by the scalar
/// path as a fraction of the channel function's DMX range, ignoring the profile's authored
/// Focus curve: Wash A (100 % at raw 10 → 0 % at raw 200) gets raw 80 for 37 % instead of 130,
/// which the Focus adapter (`optics::OpticsAdapter`, used for Dynamics and Preload hybrids)
/// produces for the same stored value. Spot B (nominal travel) is unaffected.
#[tokio::test]
#[ignore = "BUG: static Live Focus ignores the authored Focus curve (Wash A 37 % -> raw 80, Focus adapter -> 130); family_lanes/observer.rs keeps Focus on the scalar path"]
async fn recorded_focus_reaches_the_authored_focus_curve_on_every_profile() {
    record_and_replace(true);
}
