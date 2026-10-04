//! TL-552 colour coverage through the real Live family frame: shipped fixtures whose colour
//! controls the derived fallback used to leave dark (CTC, tint, macro wheels, a wheel in front of
//! flags, hue/saturation engines, zoned heads under a shared head) now show a programmed colour
//! in the rendered DMX, with the auxiliary controls parked at their neutral state. (Zoned heads
//! are covered by the fixture-level coverage tests; this bench patches single-head fixtures.)
use super::*;
use light_core::ColorResolutionQuality;
use light_fixture::{FixtureProfile, apply_runtime_profile_compatibility};

fn shipped(name: &str, mode: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    let mut profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    profile.modes.retain(|m| m.name == mode);
    assert_eq!(profile.modes.len(), 1, "{name} {mode}");
    apply_runtime_profile_compatibility(&mut profile);
    profile
}

struct Rigged {
    bench: Bench,
    fixtures: Vec<(FixtureId, FixtureProfile)>,
}

impl Rigged {
    /// Each fixture alone on its own universe (1, 2, …) at address 1.
    fn new(profiles: Vec<FixtureProfile>) -> Self {
        let fixtures: Vec<_> = profiles
            .into_iter()
            .map(|profile| (FixtureId::new(), profile))
            .collect();
        let patched = fixtures
            .iter()
            .zip(1..)
            .map(|((id, profile), universe)| {
                let mut fixture = patched(profile, *id, 1);
                fixture.universe = Some(universe);
                fixture.fixture_number = Some(u32::from(universe));
                fixture
            })
            .collect();
        let bench =
            Bench::with_fixtures(patched, &definition(), PROGRAMMING_CONTRACT_VERSION, true);
        Self { bench, fixtures }
    }

    fn color(&self, index: usize, rgb: [f32; 3]) {
        self.bench.clock.advance_millis(10);
        self.bench.programmers.set(
            self.bench.session,
            self.fixtures[index].0,
            ProgrammingOwner::Color.key(),
            program(&intent(rgb, 0.)),
        );
    }

    /// Rendered native value of the channel with this fixture attribute.
    fn dmx(&self, rendered: &RenderResult, index: usize, fixture_attribute: &str) -> u32 {
        let mode = &self.fixtures[index].1.modes[0];
        let slots = mode.primary_slots().unwrap();
        let channel = mode
            .channels
            .iter()
            .find(|c| &*c.fixture_attribute.0 == fixture_attribute)
            .unwrap_or_else(|| panic!("{fixture_attribute}"));
        let bytes = &rendered.universes[&(index as u16 + 1)];
        let mut raw = u32::from(bytes[usize::from(slots[&channel.id]) - 1]);
        for fine in &channel.secondary_slots {
            raw = (raw << 8) | u32::from(bytes[usize::from(*fine) - 1]);
        }
        raw
    }

    fn quality(&self, index: usize) -> Option<ColorResolutionQuality> {
        let accepted = self.bench.family.latest_accepted_color()?;
        let target = self.fixtures[index].0;
        accepted
            .heads
            .iter()
            .find(|row| row.target == target)
            .map(|row| row.quality)
    }
}

#[test]
fn ctc_tint_and_macro_wheel_fixtures_render_the_programmed_colour_with_neutral_aux() {
    let rig = Rigged::new(vec![
        shipped("jb-lighting--jbled-a7", "Standard RGB 8 Bit (S8)"),
        shipped("robe--robin-600x-ledwash", "Mode 3"),
        shipped("prolights--ecl-fresnel-ct-plus-m", "STANDARD"),
        shipped("martin--mac-300", "Mode 4"),
    ]);
    for index in 0..4 {
        rig.color(index, [1., 0., 0.]);
    }
    let frame = rig.bench.frame();
    assert!(frame.hybrid);
    let dmx = |index, attribute| rig.dmx(&frame.rendered, index, attribute);
    // JBLED A7: RGB red, CTC and the "Colour mixing with RGB" macro range parked.
    assert_eq!(
        [
            dmx(0, "color.red"),
            dmx(0, "color.green"),
            dmx(0, "color.blue")
        ],
        [255, 0, 0]
    );
    assert_eq!(dmx(0, "color.temperature"), 0);
    assert!(dmx(0, "color.wheel.1") <= 1);
    // Robin 600X LEDWash Mode 3: RGBW red, CTC and the macro wheel's "No function".
    assert_eq!(
        ["color.red", "color.green", "color.blue", "color.white"].map(|a| dmx(1, a)),
        [255, 0, 0, 0]
    );
    assert_eq!(
        [dmx(1, "color.temperature"), dmx(1, "color.wheel.1")],
        [0, 0]
    );
    // Prolights ECL: RGB red with CTC and tint parked.
    assert_eq!(
        ["color.red", "color.green", "color.blue"].map(|a| dmx(2, a)),
        [255, 0, 0]
    );
    assert_eq!([dmx(2, "color.temperature"), dmx(2, "color.tint")], [0, 0]);
    // Mac 300 Mode 4: CMY red with the colour wheel open.
    assert_eq!(
        [
            "color.cyan",
            "color.magenta",
            "color.yellow",
            "color.wheel.1"
        ]
        .map(|a| dmx(3, a)),
        [0, 255, 255, 0]
    );
    // Derived from channel names: every result is honestly Uncalibrated.
    for index in 0..4 {
        assert_eq!(
            rig.quality(index),
            Some(ColorResolutionQuality::Uncalibrated),
            "{index}"
        );
    }
}

#[test]
fn hue_saturation_fixtures_render_and_a_colour_macro_is_actively_parked() {
    let rig = Rigged::new(vec![
        shipped("etc--source-four-led-series-2-lustr", "HSI"),
        shipped("chauvet-professional--colorado-1-solo", "HSIC"),
    ]);
    rig.color(0, [0., 1., 1.]);
    rig.color(1, [0., 1., 0.]);
    let frame = rig.bench.frame();
    let dmx = |index, attribute| rig.dmx(&frame.rendered, index, attribute);
    // Lustr HSI: cyan is hue 180° at full saturation (16-bit hue).
    assert_eq!(
        [dmx(0, "color.hue"), dmx(0, "color.saturation")],
        [32768, 255]
    );
    // Colorado HSIC: green, and the white macro channel leaves its 255 default for its
    // "No function" range, so the macro cannot tint the engine.
    assert_eq!(dmx(1, "color.saturation"), 255);
    assert!(dmx(1, "color.white") <= 10, "{}", dmx(1, "color.white"));
    // Hue/saturation grids are estimated data: an exact grid point reads Approximate.
    assert_eq!(rig.quality(0), Some(ColorResolutionQuality::Approximate));
}

/// G8d: Lustr HSI(C) Plus 7 render through their seven direct emitters, with the Plus Seven gate
/// held activated and the layered hue/saturation engine parked at saturation 0 (hue at 0).
#[test]
fn lustr_plus_seven_renders_the_direct_emitters_with_the_hsi_engine_parked() {
    let rig = Rigged::new(vec![
        shipped("etc--source-four-led-series-2-lustr", "HSI Plus 7"),
        shipped("etc--source-four-led-series-2-lustr", "HSIC Plus 7"),
    ]);
    rig.color(0, [1., 0., 0.]);
    rig.color(1, [0., 0., 1.]);
    let frame = rig.bench.frame();
    assert!(frame.hybrid);
    let dmx = |index, attribute| rig.dmx(&frame.rendered, index, attribute);
    let direct = [
        "color.red",
        "color.lime",
        "color.amber",
        "color.green",
        "color.cyan",
        "color.blue",
        "color.indigo",
    ];
    assert_eq!(direct.map(|a| dmx(0, a)), [255, 0, 0, 0, 0, 0, 0]);
    assert_eq!(direct.map(|a| dmx(1, a)), [0, 0, 0, 0, 0, 255, 0]);
    for index in 0..2 {
        assert_eq!(
            [
                dmx(index, "color.hue"),
                dmx(index, "color.saturation"),
                dmx(index, "fixture.plus_7_control")
            ],
            // 192: the middle of Plus Seven's "activated" range (130-255).
            [0, 0, 192],
            "{index}: HSI parked at saturation 0, Plus Seven activated"
        );
        // Brightness stays Intensity's; nothing programmed it.
        assert_eq!(dmx(index, "intensity"), 0, "{index}");
        assert_eq!(
            rig.quality(index),
            Some(ColorResolutionQuality::Uncalibrated),
            "{index}"
        );
    }
    // HSIC Plus 7: the colour point is parked too.
    assert_eq!(dmx(1, "color.temperature"), 0);
}

#[test]
fn a_layered_engine_without_an_activation_gate_writes_no_colour_and_has_no_colour_result() {
    // Negative control: without the described Plus Seven gate, whether the direct emitters act
    // over the hue/saturation engine is unknown, so the mode stays excluded.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/etc--source-four-led-series-2-lustr.toskfixture");
    let mut ungated = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    ungated.modes.retain(|m| m.name == "HSI Plus 7");
    for channel in &mut ungated.modes[0].channels {
        channel.functions.retain(|f| {
            !matches!(
                &f.behavior,
                light_fixture::ChannelFunctionBehavior::Fixed { semantic_id, .. }
                    if semantic_id == "plus_seven_on"
            )
        });
    }
    ungated.id = FixtureId::new();
    apply_runtime_profile_compatibility(&mut ungated);
    let rig = Rigged::new(vec![
        ungated,
        shipped("jb-lighting--jbled-a7", "Compressed RGB 8 Bit (C8)"),
    ]);
    assert!(rig.fixtures[0].1.modes[0].color_physical.is_none());
    rig.color(0, [1., 0., 0.]);
    rig.color(1, [1., 0., 0.]);
    rig.bench.frame();
    let published = rig.bench.family.take_published().unwrap();
    assert!(
        published
            .writes
            .iter()
            .all(|(_, target, _)| *target != rig.fixtures[0].0),
        "no Color write for a fixture without a colour model"
    );
    assert_eq!(rig.quality(0), None);
    assert_eq!(rig.quality(1), Some(ColorResolutionQuality::Uncalibrated));
}
