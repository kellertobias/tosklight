//! TL-552 colour fallback: modes without an authored physical Color model get a nominal or
//! uncalibrated one at runtime compile time, driven by the existing fitter and forward model.
use super::*;
use crate::forward::*;
use light_core::programming::ColorIntent;
use light_core::srgb_to_xyz;

pub(super) fn package(name: &str) -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(format!("{name}.toskfixture"));
    crate::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

/// The shipped profile reduced to one mode, after the runtime projection.
pub(super) fn runtime(name: &str, mode: &str) -> FixtureProfile {
    let mut profile = package(name);
    profile.modes.retain(|m| m.name == mode);
    assert_eq!(profile.modes.len(), 1, "{name} has mode {mode}");
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    profile
}

pub(super) fn intent(rgb: [f32; 3]) -> ColorIntent {
    let mut value = ColorIntent::default();
    value.recipe.rgb = rgb;
    value.base_xyz = srgb_to_xyz(rgb[0], rgb[1], rgb[2]);
    value
}

pub(super) struct Fit {
    pub profile: FixtureProfile,
    pub fitting: CompiledColorFitting,
    workspace: ColorFitWorkspace,
    pub output: ColorFitResult,
}

impl Fit {
    pub fn new(profile: FixtureProfile) -> Self {
        let fitting = CompiledColorFitting::compile(&profile, profile.modes[0].id, None)
            .unwrap()
            .expect("the derived model compiles into the existing fitter");
        Self {
            workspace: fitting.create_workspace(),
            output: fitting.create_output(0).unwrap(),
            fitting,
            profile,
        }
    }

    pub fn run(&mut self, request: &ColorIntent) -> ColorFitResult {
        let current = vec![0; self.profile.modes[0].channels.len()];
        self.run_from(&current, request)
    }

    /// Fit from these current native values (a parked control may start anywhere).
    pub fn run_from(&mut self, current: &[u32], request: &ColorIntent) -> ColorFitResult {
        self.fitting
            .fit(0, current, request, &mut self.workspace, &mut self.output)
            .unwrap();
        self.output.clone()
    }

    /// Written raw of the channel with this fixture attribute.
    pub fn raw(&self, fixture_attribute: &str) -> Option<u32> {
        let index = self.profile.modes[0]
            .channels
            .iter()
            .position(|c| &*c.fixture_attribute.0 == fixture_attribute)?;
        self.output
            .writes
            .iter()
            .find(|w| w.channel_index as usize == index)
            .map(|w| w.raw)
    }

    pub fn raws(&self, attributes: &[&str]) -> Vec<Option<u32>> {
        attributes.iter().map(|a| self.raw(a)).collect()
    }
}

fn path(profile: &FixtureProfile) -> &HeadOpticalPath {
    &profile.modes[0].color_physical.as_ref().unwrap().paths[0]
}

fn emitters(profile: &FixtureProfile) -> &[OpticalEmitter] {
    match &path(profile).source {
        OpticalSource::Additive { emitters } => emitters,
        other => panic!("expected additive source, got {other:?}"),
    }
}

#[test]
fn rgb_led_without_a_colour_system_is_uncalibrated_srgb_and_outputs_pure_red() {
    let profile = runtime("generic--rgb-led", "DRGB 8-bit dimmer first");
    let path = path(&profile);
    // Every colour channel is a control; the dimmer is not.
    assert_eq!(path.controls.len(), 3);
    for emitter in emitters(&profile) {
        assert_eq!(emitter.provenance.quality, PhysicalDataQuality::Unknown);
        assert_eq!(
            emitter.provenance.source.as_deref(),
            Some(DERIVED_UNCALIBRATED_SOURCE)
        );
        assert!(!emitter.native_reversed);
    }
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(
        fit.raws(&["color.red", "color.green", "color.blue", "intensity"]),
        [Some(255), Some(0), Some(0), None]
    );
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
    // Uncalibrated: the match is computed, but the data is never presented as known.
    assert_eq!(red.visible.data_quality, PhysicalDataQuality::Unknown);
    assert!(red.visible.nominal);
    // No UV emitter: a UV request is unsupported and leaves the visible result alone.
    let mut uv = intent([1., 0., 0.]);
    uv.uv.amount = 1.;
    let with_uv = fit.run(&uv);
    assert_eq!(with_uv.uv.status, UvFitStatus::Unsupported);
    assert_eq!(with_uv.visible.color_match, ColorMatch::Exact);
}

#[test]
fn rgbw_led_uses_a_d65_white_emitter_and_keeps_pure_blue_on_blue() {
    let profile = runtime("generic--rgbw-led", "DRGBW 8-bit dimmer first");
    let white = emitters(&profile)
        .iter()
        .find(|e| e.name == "White")
        .expect("white emitter");
    assert_eq!(white.xyz, Some(SEMANTIC_WHITE_XYZ));
    let mut fit = Fit::new(profile);
    let blue = fit.run(&intent([0., 0., 1.]));
    assert_eq!(
        fit.raws(&["color.red", "color.green", "color.blue", "color.white"]),
        [Some(0), Some(0), Some(255), Some(0)]
    );
    assert_eq!(blue.visible.color_match, ColorMatch::Exact);
    assert_eq!(blue.visible.data_quality, PhysicalDataQuality::Unknown);
}

#[test]
fn rgbwa_uv_led_keeps_amber_visible_and_uv_controllable_with_unknown_appearance() {
    let profile = runtime("generic--rgbwauv-led", "DRGBWAU 8-bit dimmer first");
    let list = emitters(&profile);
    assert_eq!(list.len(), 6);
    let amber = list.iter().find(|e| e.name == "Amber").unwrap();
    assert_eq!(amber.band, OpticalEmitterBand::Visible);
    assert!(amber.xyz.is_some());
    let uv = list.iter().find(|e| e.name == "UV").unwrap();
    assert_eq!(uv.band, OpticalEmitterBand::Ultraviolet);
    assert_eq!(uv.xyz, None, "UV visible output is unknown, never black");
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(fit.raw("color.uv"), Some(0));
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
    let mut request = intent([1., 0., 0.]);
    request.uv.amount = 1.;
    let with_uv = fit.run(&request);
    assert_eq!(with_uv.uv.status, UvFitStatus::Applied);
    assert_eq!(fit.raw("color.uv"), Some(255));
    // Active UV of unknown appearance: the visible total is not claimed.
    assert_eq!(with_uv.total_quality, PhysicalDataQuality::Unknown);
}

#[test]
fn cmy_flags_are_ideal_srgb_complements_of_a_d65_beam() {
    let profile = runtime("generic--cmy-led", "DCMY 8-bit dimmer first");
    let list = emitters(&profile);
    assert_eq!(
        list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["Cyan", "Magenta", "Yellow"]
    );
    // A flag fully in removes its complement: its native start is full output.
    assert!(list.iter().all(|e| e.native_reversed));
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(
        fit.raws(&["color.cyan", "color.magenta", "color.yellow"]),
        [Some(0), Some(255), Some(255)]
    );
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
    fit.run(&intent([0., 0., 1.]));
    assert_eq!(
        fit.raws(&["color.cyan", "color.magenta", "color.yellow"]),
        [Some(255), Some(255), Some(0)]
    );
    fit.run(&intent([1., 1., 1.]));
    assert_eq!(
        fit.raws(&["color.cyan", "color.magenta", "color.yellow"]),
        [Some(0), Some(0), Some(0)]
    );
}

fn slot(name: &str, from: u32, to: u32) -> ChannelFunction {
    ChannelFunction {
        id: Uuid::new_v4(),
        name: name.into(),
        dmx_from: from,
        dmx_to: to,
        attribute: AttributeKey("color.wheel.1".into()),
        priority: 0,
        physical_mapping: None,
        angular_motion: None,
        behavior: ChannelFunctionBehavior::Fixed {
            semantic_id: name.to_ascii_lowercase().replace(' ', "_"),
            label: name.into(),
            raw_value: from,
        },
    }
}

/// Generic Dimmer plus a colour wheel whose slots carry the given names.
fn wheel_profile(names: &[&str]) -> FixtureProfile {
    let mut profile = package("generic--dimmer");
    profile.modes.retain(|m| m.name == "8-bit");
    let mode = &mut profile.modes[0];
    let mut wheel = mode.channels[0].clone();
    wheel.id = Uuid::new_v4();
    wheel.fixture_attribute = AttributeKey("color.wheel.1".into());
    wheel.attribute = wheel.fixture_attribute.clone();
    wheel.secondary_slots.clear();
    wheel.default_raw = 0;
    wheel.functions = names
        .iter()
        .enumerate()
        .map(|(i, name)| slot(name, i as u32 * 16, i as u32 * 16 + 15))
        .collect();
    let mut rotation =
        ChannelFunction::continuous("Rainbow scroll", AttributeKey("color.wheel.1".into()), 255);
    rotation.dmx_from = names.len() as u32 * 16;
    wheel.functions.push(rotation);
    mode.channels.push(wheel);
    mode.splits[0].footprint = 2;
    profile.validate().unwrap();
    profile
}

#[test]
fn a_wheel_with_named_slots_is_observed_per_steady_slot_and_never_parks_on_a_scroll() {
    let mut profile = wheel_profile(&["Open", "Red", "Blue"]);
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    let path = path(&profile).clone();
    assert!(matches!(path.source, OpticalSource::Fixed { .. }));
    assert_eq!(
        path.filters.len(),
        4,
        "one unknown filter per wheel function"
    );
    assert!(
        path.filters
            .iter()
            .all(|f| matches!(f.transmission, OpticalTransmission::Unknown))
    );
    // Three steady named slots; the scroll is never observed.
    assert_eq!(path.measurements.len(), 3);
    assert!(
        path.measurements
            .iter()
            .all(|m| m.provenance.quality == PhysicalDataQuality::Unknown)
    );
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(fit.raw("color.wheel.1"), Some(16 + 7));
    assert_eq!(red.visible.data_quality, PhysicalDataQuality::Unknown);
    let orange = fit.run(&intent([1., 0.5, 0.]));
    let raw = fit.raw("color.wheel.1").unwrap();
    assert!(raw < 48, "parked on a steady slot, never the scroll: {raw}");
    assert_ne!(orange.visible.color_match, ColorMatch::Exact);
}

#[test]
fn a_measured_wheel_system_keeps_measured_slots_and_shows_a_slot_colour_exactly() {
    let mut profile = wheel_profile(&["Open", "Deep Red", "Medium Blue"]);
    let mode = &mut profile.modes[0];
    let wheel = mode.channels[1].id;
    let head = mode.heads[0].id;
    mode.color_systems = vec![HeadColorSystem {
        head_id: head,
        correction_matrix: identity_color_correction(),
        calibration: ColorSystemCalibration {
            status: ColorCalibrationStatus::Measured,
            revision: 3,
            source: Some("colorimeter".into()),
        },
        system: ColorSystem::DiscreteWheel {
            channel_id: wheel,
            slots: [
                ("open", [1., 1., 1.]),
                ("deep_red", [1., 0., 0.]),
                ("medium_blue", [0., 0., 1.]),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (id, rgb))| ColorWheelSlot {
                semantic_id: id.into(),
                label: id.into(),
                dmx_from: i as u32 * 16,
                dmx_to: i as u32 * 16 + 15,
                measured_xyz: Some(srgb_to_xyz(rgb[0], rgb[1], rgb[2])),
                steady: Some(true),
            })
            .collect(),
        },
    }];
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    let model = profile.modes[0].color_physical.as_ref().unwrap();
    assert_eq!(model.revision, 3);
    assert!(
        model.paths[0]
            .measurements
            .iter()
            .all(|m| m.provenance.quality == PhysicalDataQuality::Measured
                && m.provenance.source.as_deref() == Some("colorimeter"))
    );
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(fit.raw("color.wheel.1"), Some(23));
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
    assert_eq!(red.visible.data_quality, PhysicalDataQuality::Measured);
    let orange = fit.run(&intent([1., 0.5, 0.]));
    assert_ne!(orange.visible.color_match, ColorMatch::Exact);
    assert_eq!(orange.visible.data_quality, PhysicalDataQuality::Measured);
}

#[test]
fn a_wheel_without_slot_colours_is_modelled_as_unknown_not_guessed() {
    let mut profile = wheel_profile(&["Slot 1", "Slot 2"]);
    apply_runtime_profile_compatibility(&mut profile);
    profile.validate().unwrap();
    let path = path(&profile).clone();
    assert!(path.measurements.is_empty());
    assert!(!path.filters.is_empty());
    let mut fit = Fit::new(profile);
    let red = fit.run(&intent([1., 0., 0.]));
    assert_eq!(red.visible.color_match, ColorMatch::Unknown);
    assert_eq!(red.visible.data_quality, PhysicalDataQuality::Unknown);
}

#[test]
fn modes_without_colour_and_authored_models_are_unchanged() {
    let mut dimmer = package("generic--dimmer");
    let before = serde_json::to_value(&dimmer).unwrap();
    apply_runtime_profile_compatibility(&mut dimmer);
    assert_eq!(serde_json::to_value(&dimmer).unwrap(), before);
    assert!(dimmer.modes.iter().all(|m| m.color_physical.is_none()));

    let mut par = package("cameo--root-par-6");
    let authored = par
        .modes
        .iter()
        .map(|m| serde_json::to_value(&m.color_physical).unwrap())
        .collect::<Vec<_>>();
    apply_derived_color_physical(&mut par);
    for (mode, before) in par.modes.iter().zip(authored) {
        if !before.is_null() {
            assert_eq!(serde_json::to_value(&mode.color_physical).unwrap(), before);
        }
    }
}

#[test]
fn media_stays_on_its_own_path_and_layered_engines_are_excluded_with_a_reason() {
    // Media layers keep their own Media colour route.
    let media = package("tosklight--media-server");
    for mode in &media.modes {
        assert!(matches!(
            mode.derived_color_outcome(),
            DerivedColorOutcome::MediaColor
        ));
        assert!(mode.derived_color_physical().is_none());
    }
    // A hue/saturation engine with direct emitters on top and no described activation gate: how
    // they combine is not described, so the outcome is an explicit reason, never a silent
    // nothing. (With ETC's Plus Seven gate the direct emitters are the engine; see
    // `derived_color_coverage::lustr_plus_seven_*`.)
    let lustr = package("etc--source-four-led-series-2-lustr");
    let mut plus = lustr
        .modes
        .iter()
        .find(|m| m.name == "HSI Plus 7")
        .unwrap()
        .clone();
    assert!(matches!(
        plus.derived_color_outcome(),
        DerivedColorOutcome::Derived(_)
    ));
    for channel in &mut plus.channels {
        channel.functions.retain(|f| {
            !matches!(
                &f.behavior,
                ChannelFunctionBehavior::Fixed { semantic_id, .. } if semantic_id == "plus_seven_on"
            )
        });
    }
    let DerivedColorOutcome::Excluded(reason) = plus.derived_color_outcome() else {
        panic!("an ungated layered engine is excluded with a reason");
    };
    assert!(reason.contains("hue/saturation"), "{reason}");
    let head = plus.heads[0].id;
    assert_eq!(plus.color_model_exclusion(head), Some(reason));
}

#[test]
fn every_shipped_profile_stays_valid_after_the_runtime_projection() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library");
    let mut derived = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let mut profile = crate::read_fixture_package(&std::fs::read(&path).unwrap()).unwrap();
        apply_runtime_profile_compatibility(&mut profile);
        profile
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for mode in &profile.modes {
            if mode.color_physical.is_some() {
                CompiledColorFitting::compile(&profile, mode.id, None)
                    .unwrap_or_else(|e| panic!("{} {}: {e}", path.display(), mode.name));
            }
        }
        derived += profile
            .modes
            .iter()
            .filter(|m| m.color_physical.is_some())
            .count();
    }
    assert!(
        derived > 50,
        "the Generic LED rig derives models: {derived}"
    );
}
