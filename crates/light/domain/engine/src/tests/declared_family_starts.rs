//! TL-544 G2: a Cue that fades a semantic Color, a Zoom or a Focus in over a fixture with no prior
//! value starts from the fixture's declared default, decoded through its compiled Color and optics
//! forward models (the Position precedent is `position_adoption/cue_fade_start.rs`). An owner
//! whose default cannot be decoded keeps the previous behaviour.
use super::*;
use chrono::TimeZone;
use light_core::OpeningConvention;
use light_core::programming::{
    ColorIntent, ColorProgram, ProgrammingOwner, ScalarIntent, ZoomIntent,
};
use light_fixture::{
    ChannelFunctionBehavior, ColorPhysicalModel, HeadOpticalPath, NativeColorBinding,
    OpticalEmitter, OpticalEmitterBand, OpticalProvenance, OpticalSource, PhysicalDataQuality,
    PhysicalMappingCalibration,
};
use uuid::Uuid;

#[path = "declared_family_starts/solo_native.rs"]
mod solo_native;

/// Channel order: intensity, red, green, blue, zoom, focus.
const RED: usize = 1;
const ZOOM: usize = 4;
const FOCUS: usize = 5;
/// Declared defaults: red emitter at full, zoom raw 0 = 50° Field, focus raw 51 = 20 %.
const DEFAULT_ZOOM: f32 = 50.;
const DEFAULT_FOCUS: f32 = 0.2;

fn redefine(fixture: &mut PatchedFixture, edit: impl FnOnce(&mut FixtureProfile)) {
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = fixture.definition.mode_id.unwrap();
    edit(&mut profile);
    fixture.definition = profile.resolved_definition(mode).unwrap();
}

fn continuous(
    function: &mut light_fixture::ChannelFunction,
    physical: (f32, f32),
    unit: &str,
    convention: Option<OpeningConvention>,
) {
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: physical.0,
        physical_max: physical.1,
        unit: Some(unit.into()),
    };
    function.physical_mapping = Some(PhysicalMappingCalibration {
        quality: PhysicalDataQuality::Measured,
        source: Some("Synthetic TL-544 G2 reference".into()),
        opening_convention: convention,
        ..Default::default()
    });
}

/// An RGB wash with a compiled Color path, a measured Field Zoom 50° → 10° and a measured
/// Focus 0 → 100 %. `modeled` false leaves Zoom without a convention and Color without a model.
fn wash(modeled: bool) -> PatchedFixture {
    let (mut fixture, _) = schema_v2_fixture(&[
        ("intensity", false, false),
        ("color.red", false, false),
        ("color.green", false, false),
        ("color.blue", false, false),
        ("zoom", false, false),
        ("focus", false, false),
    ]);
    redefine(&mut fixture, |profile| {
        let mode = &mut profile.modes[0];
        mode.channels[RED].default_raw = 255;
        mode.channels[FOCUS].default_raw = 51;
        continuous(
            &mut mode.channels[ZOOM].functions[0],
            (50., 10.),
            "deg",
            modeled.then_some(OpeningConvention::Field),
        );
        continuous(
            &mut mode.channels[FOCUS].functions[0],
            (0., 100.),
            "%",
            None,
        );
        if !modeled {
            return;
        }
        let emitters = [RED, RED + 1, RED + 2]
            .into_iter()
            .enumerate()
            .map(|(i, index)| {
                let channel = &mode.channels[index];
                let mut xyz = [0.; 3];
                xyz[i] = 1.;
                OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: channel.attribute.0.to_string(),
                    binding: NativeColorBinding {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                    },
                    xyz: Some(Xyz {
                        x: xyz[0],
                        y: xyz[1],
                        z: xyz[2],
                    }),
                    spectrum: vec![],
                    band: OpticalEmitterBand::Visible,
                    native_reversed: false,
                    maximum_level: 1.,
                    response_exponent: 1.,
                    provenance: OpticalProvenance {
                        quality: PhysicalDataQuality::Estimated,
                        ..Default::default()
                    },
                }
            })
            .collect();
        let controls = [RED, RED + 1, RED + 2].map(|index| mode.channels[index].id);
        mode.color_physical = Some(ColorPhysicalModel {
            version: 1,
            revision: 1,
            paths: vec![HeadOpticalPath {
                id: Uuid::new_v4(),
                head_id: mode.heads[0].id,
                controls: controls.into(),
                source: OpticalSource::Additive { emitters },
                filters: vec![],
                measurements: vec![],
            }],
        });
    });
    fixture
}

fn blue() -> AttributeValue {
    let intent = ColorIntent {
        base_xyz: Xyz {
            x: 0.,
            y: 0.,
            z: 1.,
        },
        ..Default::default()
    };
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}

fn field(degrees: f32) -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention: OpeningConvention::Field,
    }))
}

fn cue_engine(fixture: PatchedFixture) -> (Engine, FixtureId, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    registry.start(SessionId::new());
    let root = fixture.fixture_id;
    let mut list = test_cue_list("Fade in", vec![]);
    let mut cue = Cue::new(1_u16.into());
    cue.changes = vec![
        CueChange::set(root, ProgrammingOwner::Color.key(), blue()),
        CueChange::set(root, ProgrammingOwner::Zoom.key(), field(20.)),
        CueChange::set(
            root,
            ProgrammingOwner::Focus.key(),
            AttributeValue::Normalized(0.8),
        ),
    ];
    cue.fade_millis = 1000;
    list.cues = vec![cue];
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![list.clone()].into(),
            playbacks: vec![test_playback(1, list.id)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, root, clock)
}

fn value(engine: &Engine, target: FixtureId, owner: ProgrammingOwner) -> Option<AttributeValue> {
    engine
        .observe_source_frame(&[])
        .values()
        .value(target, &owner.key())
        .cloned()
}

fn opening(value: Option<AttributeValue>) -> Option<f32> {
    match value? {
        AttributeValue::Zoom(zoom) => match zoom.opening_degrees {
            ScalarIntent::Value(degrees) => Some(degrees),
            ScalarIntent::Spread(_) => None,
        },
        _ => None,
    }
}

fn base_xyz(value: Option<AttributeValue>) -> Option<Xyz> {
    match value? {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Semantic { intent } => Some(intent.base_xyz),
            ColorProgram::Direct { .. } => None,
        },
        _ => None,
    }
}

#[test]
fn cue_color_zoom_and_focus_fade_in_from_the_declared_defaults() {
    let (engine, root, clock) = cue_engine(wash(true));
    let snapshot = engine.snapshot();
    let declared_color =
        base_xyz(engine.declared_default_family(&snapshot, root, ProgrammingOwner::Color))
            .expect("the red default output is a known visible appearance");
    assert!(
        declared_color.x > 0.5 && declared_color.z == 0.,
        "{declared_color:?}"
    );
    assert_eq!(
        opening(engine.declared_default_family(&snapshot, root, ProgrammingOwner::Zoom)),
        Some(DEFAULT_ZOOM)
    );
    let Some(AttributeValue::Normalized(focus)) =
        engine.declared_default_family(&snapshot, root, ProgrammingOwner::Focus)
    else {
        panic!("Focus default decodes to normalized travel")
    };
    assert!((focus - DEFAULT_FOCUS).abs() < 1e-3, "{focus}");

    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(250);
    let lerp = |from: f32, to: f32| from + (to - from) * 0.25;
    let zoom = opening(value(&engine, root, ProgrammingOwner::Zoom))
        .expect("the Zoom fade renders from its first frame");
    assert!((zoom - lerp(DEFAULT_ZOOM, 20.)).abs() < 1e-3, "Zoom {zoom}");
    let Some(AttributeValue::Normalized(focus)) = value(&engine, root, ProgrammingOwner::Focus)
    else {
        panic!("Focus renders")
    };
    assert!(
        (focus - lerp(DEFAULT_FOCUS, 0.8)).abs() < 1e-3,
        "Focus {focus}: from the declared 20 %, never from 0"
    );
    let color = base_xyz(value(&engine, root, ProgrammingOwner::Color))
        .expect("the Color fade renders from its first frame");
    assert!(
        (color.x - lerp(declared_color.x, 0.)).abs() < 1e-3,
        "{color:?}"
    );
    assert!(
        (color.z - lerp(declared_color.z, 1.)).abs() < 1e-3,
        "{color:?}"
    );

    clock.advance_millis(750);
    assert_eq!(
        opening(value(&engine, root, ProgrammingOwner::Zoom)),
        Some(20.)
    );
    assert_eq!(
        value(&engine, root, ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.8))
    );
    assert_eq!(value(&engine, root, ProgrammingOwner::Color), Some(blue()));
}

#[test]
fn an_undecodable_default_keeps_the_previous_fade_in() {
    let (engine, root, clock) = cue_engine(wash(false));
    let snapshot = engine.snapshot();
    assert!(
        engine
            .declared_default_family(&snapshot, root, ProgrammingOwner::Color)
            .is_none(),
        "no Color model: unknown stays unknown"
    );
    assert!(
        engine
            .declared_default_family(&snapshot, root, ProgrammingOwner::Zoom)
            .is_none(),
        "no declared convention: a degree default is never guessed into one"
    );
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.advance_millis(250);
    assert_eq!(value(&engine, root, ProgrammingOwner::Color), None);
    assert_eq!(value(&engine, root, ProgrammingOwner::Zoom), None);
    clock.advance_millis(750);
    assert_eq!(
        opening(value(&engine, root, ProgrammingOwner::Zoom)),
        Some(20.)
    );
    assert_eq!(value(&engine, root, ProgrammingOwner::Color), Some(blue()));
}
