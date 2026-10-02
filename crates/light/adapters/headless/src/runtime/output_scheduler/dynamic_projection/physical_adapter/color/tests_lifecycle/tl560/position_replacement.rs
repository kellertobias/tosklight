//! TL-560 matrix row "Fixture replacement (output)", columns Position Angles and Position Target.
//!
//! Angles and a Point Target (Point UUID plus a local offset) are recorded on the shipped AURO
//! SPOT Z300 (TL-637 binding: Pan ±270°, Tilt ±135°) through the real Cue writer, the SQLite show
//! is reopened, and the mover is then replaced by the shipped JBLED A7 (Pan ±215°, Tilt ±150°),
//! by the same JBLED installed with inverted Pan, and the Point is moved, deleted and re-added.
//! Every generation recompiles the reopened file, opens a fresh contract-1 desk and renders the
//! production Live family frame; the encoded universe bytes are decoded through an independently
//! compiled `CompiledPositionForward` for the installed fixture.
//!
//! The stored Cue list body and its Cue count never change. Static Target fitting only: no
//! Dynamics, no native-setup claim, and the shipped Position graphs are TL-637's estimated
//! engineering models, not measured lamp calibration.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position::PositionAdapter;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position::tests::point;
use light_core::programming::{PositionIntent, TargetReference};
use light_fixture::forward::{CompiledPositionForward, PositionInstallation};
use light_fixture::{FixtureLocation, PositionAxisRole, PositionFitRequest, PositionFitStatus};

const ANGLES: f64 = 1.0;
const TARGET: f64 = 2.0;

pub(super) fn shipped(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../../../assets/fixture-library/{name}.toskfixture"
    ));
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

fn angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(-35.5, 72.25)))
}

fn target(point: FixtureId) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id: point.0 },
        [0.5, -1.25, 2.0],
    )))
}

fn mover(profile: &FixtureProfile, id: FixtureId) -> PatchedFixture {
    let mut fixture = super::super::fixture(profile, id, 1, 1);
    fixture.location.z = 6000;
    fixture.name = format!("TL-560 mover {}", profile.name);
    fixture
}

fn aim(id: FixtureId, x: i32) -> PatchedFixture {
    let mut aim = point(id, FixtureLocation { x, y: 4000, z: 0 });
    aim.name = "TL-560 aim".into();
    aim
}

/// The installed fixture's Pan and Tilt channel indices.
fn axes(fixture: &PatchedFixture) -> [usize; 2] {
    let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
    let mode = profile.mode(fixture.definition.mode_id.unwrap()).unwrap();
    let bindings = &mode.position_physical.as_ref().unwrap().bindings;
    [PositionAxisRole::Pan, PositionAxisRole::Tilt].map(|role| {
        let binding = bindings.iter().find(|b| b.role == role).unwrap();
        mode.channels
            .iter()
            .position(|c| c.id == binding.channel_id)
            .unwrap()
    })
}

/// Decode the commanded absolute Pan/Tilt degrees from native output through the installed
/// fixture's own compiled forward model (inversion and calibration included).
fn commanded(fixture: &PatchedFixture, native: &[u32]) -> [f64; 2] {
    let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
    let forward = CompiledPositionForward::compile(
        profile,
        fixture.definition.mode_id.unwrap(),
        PositionInstallation {
            calibration: fixture.position_calibration.as_ref(),
            invert_pan: fixture.invert_pan,
            invert_tilt: fixture.invert_tilt,
            bracket_degrees: f64::from(fixture.bracket_angle),
        },
    )
    .unwrap()
    .expect("a compiled Position forward model");
    let mut commands = forward.create_commands();
    forward.decode_commands(native, &mut commands).unwrap();
    [PositionAxisRole::Pan, PositionAxisRole::Tilt].map(|role| {
        commands
            .iter()
            .find(|c| c.role == Some(role))
            .and_then(|c| c.absolute_degrees())
            .expect("absolute axis command")
    })
}

/// Resolve the played Position owner of `target` once through the Position adapter on a
/// captured desk frame (diagnostics: fit status, requested world, achieved axes).
pub(super) fn fit(desk: &Desk, target: FixtureId) -> PhysicalResolution<PositionAdapter> {
    let engine = desk.engine();
    desk.clock.advance_millis(25);
    let capture = engine.prepare_output_frame(RenderOptions::default());
    let token = capture.frame_token();
    let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let snapshot = capture.snapshot();
    let adapter = PositionAdapter::default();
    let descriptor = adapter
        .compile(&snapshot, target)
        .unwrap()
        .expect("Position destination");
    let value = desk
        .played(target, ProgrammingOwner::Position)
        .expect("played Position");
    let request = PhysicalRequest {
        frame: HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: snapshot.native_color_sources.as_ref(),
            token: &token,
            scalar: &scalar,
        },
        target,
        owner: ProgrammingOwner::Position,
        descriptor: &descriptor,
        value: &value,
        previous: None,
    };
    let mut results = adapter.resolve_cohort(&[request], &[]).unwrap();
    validate_complete_writes(&descriptor.footprint, &results[0].writes).unwrap();
    results.remove(0)
}

struct Generation {
    /// Native output and decoded commanded axes of the Angles Cue.
    angles: (Vec<u32>, [f64; 2]),
    /// Fit of the Target Cue and its native output.
    target: (PhysicalResolution<PositionAdapter>, Vec<u32>),
}

/// Reopen, compile, open a fresh desk, play both Cues and verify the bytes.
fn generation(show: &Show, fixture: &PatchedFixture, recorded: &Value) -> Generation {
    assert_eq!(
        show.cue_list().1,
        *recorded,
        "the stored Cue list is unchanged"
    );
    let snapshot = show.compile();
    assert_eq!(snapshot.cue_lists[0].cues.len(), 2, "no Cue reprogramming");
    let desk = Desk::open(snapshot);
    let id = fixture.fixture_id;
    let owner = ProgrammingOwner::Position;

    desk.go(ANGLES);
    assert_eq!(desk.played(id, owner), Some(angles()), "{}", fixture.name);
    let frame = desk.frame();
    assert!(
        !frame.family(owner, id).is_empty(),
        "{}: Position is a family write",
        fixture.name
    );
    let native = frame.assert_encoded(fixture);
    let axes_degrees = commanded(fixture, &native);
    let requested = [-35.5, 72.25];
    for (axis, (achieved, requested)) in axes_degrees.iter().zip(requested).enumerate() {
        assert!(
            (achieved - requested).abs() < 0.05,
            "{}: axis {axis} {requested} -> {achieved}",
            fixture.name
        );
    }
    let angles_fit = fit(&desk, id);
    assert_eq!(angles_fit.requested, *intent(&angles()));
    assert_eq!(
        angles_fit.achieved.outcomes[0].result.status,
        PositionFitStatus::Fitted
    );

    desk.go(TARGET);
    let frame = desk.frame();
    let native_target = frame.assert_encoded(fixture);
    let target_fit = fit(&desk, id);
    Generation {
        angles: (native, axes_degrees),
        target: (target_fit, native_target),
    }
}

fn intent(value: &AttributeValue) -> &PositionIntent {
    let AttributeValue::Position(intent) = value else {
        panic!("Position intent")
    };
    intent
}

fn fitted(generation: &Generation, fixture: &PatchedFixture, stored: &AttributeValue) {
    let (result, native) = &generation.target;
    assert_eq!(result.requested, *intent(stored), "{}", fixture.name);
    let outcome = &result.achieved.outcomes[0].result;
    assert_eq!(
        outcome.status,
        PositionFitStatus::Fitted,
        "{}",
        fixture.name
    );
    assert!(!result.quality.held);
    assert!(
        outcome.angular_error_degrees.unwrap() < 0.5,
        "{}: {:?}",
        fixture.name,
        outcome.angular_error_degrees
    );
    // The adapter's fitted axes are the bytes on the wire.
    let achieved = outcome.achieved.expect("achieved axes");
    let decoded = commanded(fixture, native);
    for (a, d) in achieved.iter().zip(decoded) {
        assert!(
            (a - d).abs() < 1e-6,
            "{}: {achieved:?} vs {decoded:?}",
            fixture.name
        );
    }
}

#[tokio::test]
async fn recorded_angles_and_point_target_survive_reopen_shipped_replacement_inversion_and_point_edits()
 {
    let show = Show::new();
    let (id, aim_id) = (FixtureId::new(), FixtureId::new());
    let auro = shipped("cameo--auro-spot-z300");
    let jbled = shipped("jb-lighting--jbled-a7");
    let original = mover(&auro, id);
    show.patch(&original);
    show.patch(&aim(aim_id, 0));
    let owner = ProgrammingOwner::Position.key();
    show.programmers
        .set(show.session, id, owner.clone(), angles());
    show.record(ANGLES);
    show.programmers
        .set(show.session, id, owner.clone(), target(aim_id));
    show.record(TARGET);
    let (_, recorded) = show.cue_list();
    assert_eq!(stored_fixture_value(&recorded, 0, id, "position"), angles());
    assert_eq!(
        stored_fixture_value(&recorded, 1, id, "position"),
        target(aim_id),
        "the Point UUID and the local offset are stored, never a resolved world point"
    );
    let stored = target(aim_id);

    // Generation 0: the recording fixture after reopen.
    let g0 = generation(&show, &original, &recorded);
    fitted(&g0, &original, &stored);
    let PositionFitRequest::Target { world: world0 } = g0.target.0.achieved.outcomes[0]
        .result
        .requested
        .clone()
        .unwrap()
    else {
        panic!("Target request")
    };

    // Generation 1: replaced by another shipped profile with a different travel range.
    let replaced = mover(&jbled, id);
    show.patch(&replaced);
    let g1 = generation(&show, &replaced, &recorded);
    fitted(&g1, &replaced, &stored);
    let [pan0, _] = axes(&original);
    let [pan1, tilt1] = axes(&replaced);
    assert_ne!(
        g0.angles.0[pan0], g1.angles.0[pan1],
        "the same angle is a different wire word on a different travel range"
    );

    // Generation 2: the same lamp installed with inverted Pan mirrors only the wire word.
    let mut inverted = replaced.clone();
    inverted.invert_pan = true;
    show.patch(&inverted);
    let g2 = generation(&show, &inverted, &recorded);
    fitted(&g2, &inverted, &stored);
    assert_eq!(g2.angles.1, g1.angles.1, "commanded angles are unchanged");
    assert_ne!(
        g2.angles.0[pan1], g1.angles.0[pan1],
        "Pan wire word mirrored"
    );
    assert_eq!(g2.angles.0[tilt1], g1.angles.0[tilt1], "Tilt untouched");

    // Generation 3: the Point moves; the stored Target follows it without a rewrite.
    show.patch(&aim(aim_id, 2500));
    let g3 = generation(&show, &inverted, &recorded);
    fitted(&g3, &inverted, &stored);
    let PositionFitRequest::Target { world: world3 } = g3.target.0.achieved.outcomes[0]
        .result
        .requested
        .clone()
        .unwrap()
    else {
        panic!("Target request")
    };
    assert_ne!(world3, world0, "the resolved world point follows the Point");
    assert_ne!(g3.target.1, g2.target.1, "the moved Point is a new aim");

    // Generation 4: the Point is deleted. The intent is retained and diagnosed; the mover holds.
    assert!(
        show.store()
            .delete_object("patched_fixture", &aim_id.0.to_string())
            .unwrap()
    );
    let g4 = generation(&show, &inverted, &recorded);
    let (result, _) = &g4.target;
    assert_eq!(result.requested, *intent(&stored), "stored intent retained");
    assert_eq!(
        result.achieved.outcomes[0].result.status,
        PositionFitStatus::MissingTarget,
        "a visible diagnostic, never a silent rebind"
    );
    assert!(result.quality.held);
    assert!(result.writes.iter().all(|w| w.parked));

    // Generation 5: re-adding the same Point identity resumes the original aim.
    show.patch(&aim(aim_id, 2500));
    let g5 = generation(&show, &inverted, &recorded);
    fitted(&g5, &inverted, &stored);
    assert_eq!(
        g5.target.1, g3.target.1,
        "the same Point resolves the same aim"
    );
}
