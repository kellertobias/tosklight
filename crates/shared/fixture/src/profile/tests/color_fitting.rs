//! Reference cases for TL-568: semantic Color destination fitting through compiled fixture
//! systems. Emitter XYZ and spectra here are synthetic references, not lamp calibrations.
use super::*;
use crate::forward::*;
use light_core::programming::{
    ColorAllocation, ColorComponent, ColorComponentSpread, ColorIntent, ColorWheelConstraint,
    UvIntent, WhiteTarget,
};
use light_core::srgb_to_xyz;

fn provenance(quality: PhysicalDataQuality) -> OpticalProvenance {
    OpticalProvenance {
        quality,
        source: Some("Synthetic TL-568 reference".into()),
        revision: 1,
    }
}

fn spectrum(f: impl Fn(u32) -> f32) -> Vec<SpectrumSample> {
    (360..=830)
        .map(|nm| SpectrumSample {
            wavelength_nm: nm as f32,
            value: f(nm),
        })
        .collect()
}

/// Flat source normalized so its open output has Y ≈ 1.
fn flat_source() -> OpticalSource {
    OpticalSource::Fixed {
        xyz: None,
        spectrum: spectrum(|_| (1.0 / 106.856_915) as f32),
        provenance: provenance(PhysicalDataQuality::Measured),
    }
}

struct Builder {
    profile: FixtureProfile,
    head: Uuid,
    emitters: Vec<OpticalEmitter>,
    filters: Vec<OpticalFilter>,
    controls: Vec<Uuid>,
    slots: u16,
    source: Option<OpticalSource>,
}

impl Builder {
    /// Starts with an Intensity channel that Color fitting must never touch.
    fn new() -> Self {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "TL-568 reference".into();
        let head = profile.modes[0].heads[0].id;
        let dimmer = channel(head, ChannelResolution::U8, vec![]);
        profile.modes[0].channels = vec![dimmer];
        Self {
            profile,
            head,
            emitters: vec![],
            filters: vec![],
            controls: vec![],
            slots: 1,
            source: None,
        }
    }

    fn channel(&mut self, attribute: &str, resolution: ChannelResolution) -> usize {
        let secondary = if resolution == ChannelResolution::U16 {
            vec![self.slots + 2]
        } else {
            vec![]
        };
        self.slots += resolution.bytes() as u16;
        let mut value = channel(self.head, resolution, secondary);
        value.attribute = AttributeKey(attribute.into());
        value.fixture_attribute = value.attribute.clone();
        value.functions[0].attribute = value.attribute.clone();
        self.controls.push(value.id);
        let channels = &mut self.profile.modes[0].channels;
        channels.push(value);
        channels.len() - 1
    }

    fn emitter(
        &mut self,
        attribute: &str,
        xyz: Option<Xyz>,
        quality: PhysicalDataQuality,
    ) -> usize {
        self.emitter_with(attribute, ChannelResolution::U8, |e| {
            e.xyz = xyz;
            e.provenance = provenance(quality);
        })
    }

    fn emitter_with(
        &mut self,
        attribute: &str,
        resolution: ChannelResolution,
        edit: impl FnOnce(&mut OpticalEmitter),
    ) -> usize {
        let index = self.channel(attribute, resolution);
        let native = &self.profile.modes[0].channels[index];
        let mut emitter = OpticalEmitter {
            id: Uuid::new_v4(),
            name: attribute.into(),
            binding: NativeColorBinding {
                channel_id: native.id,
                function_id: native.functions[0].id,
            },
            xyz: None,
            spectrum: vec![],
            band: if attribute == "color.uv" {
                OpticalEmitterBand::Ultraviolet
            } else {
                OpticalEmitterBand::Visible
            },
            native_reversed: false,
            maximum_level: 1.0,
            response_exponent: 1.0,
            provenance: Default::default(),
        };
        edit(&mut emitter);
        self.emitters.push(emitter);
        index
    }

    /// A wheel/flag channel: the first function carries the steady samples, an optional second
    /// function (e.g. rotation) stays unmodeled.
    fn filter(
        &mut self,
        attribute: &str,
        slots_to: u32,
        samples: Vec<(u32, u32, Vec<SpectrumSample>)>,
        rotation: bool,
    ) -> usize {
        let index = self.channel(attribute, ChannelResolution::U8);
        let native = &mut self.profile.modes[0].channels[index];
        native.functions[0].dmx_to = slots_to;
        if rotation {
            let mut spin = ChannelFunction::continuous(
                "Rotation",
                AttributeKey(format!("{attribute}.rotation").into()),
                255,
            );
            spin.dmx_from = slots_to + 1;
            native.functions.push(spin);
        }
        let binding = NativeColorBinding {
            channel_id: native.id,
            function_id: native.functions[0].id,
        };
        self.filters.push(OpticalFilter {
            id: Uuid::new_v4(),
            name: attribute.into(),
            binding,
            transmission: OpticalTransmission::Spectral {
                samples: samples
                    .into_iter()
                    .map(|(raw_from, raw_to, spectrum)| FilterSpectrum {
                        raw_from,
                        raw_to,
                        spectrum,
                    })
                    .collect(),
            },
            provenance: provenance(PhysicalDataQuality::Measured),
        });
        index
    }

    fn build(self) -> FixtureProfile {
        let mut profile = self.profile;
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = self.slots;
        mode.color_physical = Some(ColorPhysicalModel {
            version: 1,
            revision: 1,
            paths: vec![HeadOpticalPath {
                id: Uuid::new_v4(),
                head_id: self.head,
                controls: self.controls,
                source: self.source.unwrap_or(OpticalSource::Additive {
                    emitters: self.emitters,
                }),
                filters: self.filters,
                measurements: vec![],
            }],
        });
        profile.validate().unwrap();
        profile
    }
}

fn xyz(x: f32, y: f32, z: f32) -> Xyz {
    Xyz { x, y, z }
}

fn rgb_columns() -> [Xyz; 3] {
    [
        srgb_to_xyz(1., 0., 0.),
        srgb_to_xyz(0., 1., 0.),
        srgb_to_xyz(0., 0., 1.),
    ]
}

/// RGB (red on a U16 channel) plus optional extra emitters, all Manufacturer quality.
fn additive(extra: &[(&str, Option<Xyz>)], quality: PhysicalDataQuality) -> FixtureProfile {
    let mut b = Builder::new();
    let [r, g, bl] = rgb_columns();
    b.emitter_with("color.red", ChannelResolution::U16, |e| {
        e.xyz = Some(r);
        e.provenance = provenance(quality);
    });
    b.emitter("color.green", Some(g), quality);
    b.emitter("color.blue", Some(bl), quality);
    for (attribute, value) in extra {
        b.emitter(attribute, *value, quality);
    }
    b.build()
}

fn intent(rgb: [f32; 3]) -> ColorIntent {
    let mut value = ColorIntent::default();
    value.recipe.rgb = rgb;
    value.base_xyz = srgb_to_xyz(rgb[0], rgb[1], rgb[2]);
    value
}

struct Fit {
    profile: FixtureProfile,
    fitting: CompiledColorFitting,
    workspace: ColorFitWorkspace,
    output: ColorFitResult,
}

impl Fit {
    fn new(profile: FixtureProfile) -> Self {
        let fitting = CompiledColorFitting::compile(&profile, profile.modes[0].id, None)
            .unwrap()
            .unwrap();
        Self {
            workspace: fitting.create_workspace(),
            output: fitting.create_output(0).unwrap(),
            fitting,
            profile,
        }
    }

    fn zero(&self) -> Vec<u32> {
        vec![0; self.profile.modes[0].channels.len()]
    }

    fn run(&mut self, current: &[u32], request: &ColorIntent) -> &ColorFitResult {
        let before = format!("{request:?}");
        self.fitting
            .fit(0, current, request, &mut self.workspace, &mut self.output)
            .unwrap();
        assert_eq!(
            before,
            format!("{request:?}"),
            "the stored request is never replaced"
        );
        let controls = self.profile.modes[0].color_physical.as_ref().unwrap().paths[0]
            .controls
            .len();
        if self.output.status == ColorFitStatus::Fitted {
            // Every Color-owned control receives a write or an explicit retain decision.
            assert_eq!(
                self.output.writes.len() + self.output.retained.len(),
                controls
            );
            // Achieved output is exactly what the forward model predicts for the proposed raw.
            let forward =
                CompiledColorForward::compile(&self.profile, self.profile.modes[0].id, None)
                    .unwrap()
                    .unwrap();
            let mut expected = forward.create_output();
            forward
                .evaluate(self.workspace.proposed_raw(), &mut expected)
                .unwrap();
            assert_eq!(self.output.visible.known_xyz, expected[0].known_xyz);
            assert_eq!(
                self.output.visible.achieved.is_some(),
                expected[0].visible_complete
            );
        }
        // Intensity (channel 0) is never a Color write.
        assert!(self.output.writes.iter().all(|w| w.channel_index != 0));
        assert_eq!(self.workspace.proposed_raw()[0], current[0]);
        &self.output
    }

    fn raw(&self, channel: usize) -> Option<u32> {
        self.output
            .writes
            .iter()
            .find(|w| w.channel_index as usize == channel)
            .map(|w| w.raw)
    }

    fn raws(&self, channels: std::ops::RangeInclusive<usize>) -> Vec<Option<u32>> {
        channels.map(|c| self.raw(c)).collect()
    }
}

#[test]
fn rgb_red_black_and_clipping_fit_through_the_encoded_forward_model() {
    let mut fit = Fit::new(additive(&[], PhysicalDataQuality::Manufacturer));
    let current = fit.zero();
    let red = fit.run(&current, &intent([1., 0., 0.])).clone();
    assert_eq!(red.status, ColorFitStatus::Fitted);
    assert_eq!(fit.raws(1..=3), [Some(65535), Some(0), Some(0)]);
    assert_eq!(red.visible.status, VisibleFitStatus::Fitted);
    assert_eq!(red.visible.color_match, ColorMatch::Exact);
    assert!(!red.visible.luminance_limited && !red.visible.nominal);
    assert_eq!(red.visible.data_quality, PhysicalDataQuality::Manufacturer);
    assert_eq!(red.uv.status, UvFitStatus::NotRequested);
    // The U16 write encodes MSB-first into its declared slots.
    let plan = fit.profile.modes[0].compile_encoding_plan().unwrap();
    let mut frame = [0u8; 512];
    let values: Vec<_> = red
        .writes
        .iter()
        .map(|w| (w.channel_index, w.raw))
        .collect();
    plan.encode_split_by_index(&mut frame, 1, 1, &values)
        .unwrap();
    assert_eq!(&frame[1..5], &[0xff, 0xff, 0, 0]);

    let mut black = intent([1., 0., 0.]);
    black.relative_output = 0.;
    let result = fit.run(&current, &black).clone();
    assert_eq!(fit.raws(1..=3), [Some(0), Some(0), Some(0)]);
    assert_eq!(result.requested_visible, Some(xyz(0., 0., 0.)));
    assert_eq!(result.visible.achieved, Some(xyz(0., 0., 0.)));
    assert_eq!(result.visible.color_match, ColorMatch::Exact);

    let mut bright = intent([1., 1., 1.]);
    bright.relative_output = 2.;
    let result = fit.run(&current, &bright).clone();
    assert_eq!(fit.raws(1..=3), [Some(65535), Some(255), Some(255)]);
    assert!((result.requested_visible.unwrap().y - 2.).abs() < 1e-4);
    assert_eq!(result.visible.color_match, ColorMatch::Exact);
    assert!(result.visible.luminance_limited);
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::LUMINANCE_LIMITED)
    );
    assert!((result.visible.luminance_ratio.unwrap() - 0.5).abs() < 1e-3);
}

#[test]
fn rgbw_white_blend_zero_half_full_reproduce_the_reference_allocation() {
    let white = white_target_xyz(WhiteTarget::default());
    let mut fit = Fit::new(additive(
        &[("color.white", Some(white))],
        PhysicalDataQuality::Manufacturer,
    ));
    let current = fit.zero();
    for (blend, expected) in [
        (0.0, [65535, 255, 255, 0]),
        (0.5, [65535, 255, 255, 255]),
        (1.0, [0, 0, 0, 255]),
    ] {
        let mut request = intent([1., 1., 1.]);
        request.white_blend = blend;
        let result = fit.run(&current, &request).clone();
        assert_eq!(fit.raws(1..=4), expected.map(Some), "White Blend {blend}");
        assert_eq!(result.visible.color_match, ColorMatch::Exact);
        assert!(!result.visible.luminance_limited, "White Blend {blend}");
    }
    // Red at 50%: full colored recipe plus full white contribution.
    let mut red = intent([1., 0., 0.]);
    red.white_blend = 0.5;
    fit.run(&current, &red);
    assert_eq!(fit.raws(1..=4), [Some(65535), Some(0), Some(0), Some(255)]);
    // An explicit allocation preference moves an exact white toward colored emitters.
    let mut colored = intent([1., 1., 1.]);
    colored.white_blend = 1.;
    colored.allocation = ColorAllocation::PreferColoredEmitters;
    let result = fit.run(&current, &colored).clone();
    let raws = fit.raws(1..=4);
    // Index 0 is the U16 red; the 8-bit white stays well below the 8-bit green.
    assert!(raws[3].unwrap() < raws[1].unwrap() / 4, "{raws:?}");
    assert_eq!(result.visible.color_match, ColorMatch::Exact);
}

#[test]
fn cct_and_duv_follow_the_planckian_locus_and_normal() {
    let chromaticity = |value: Xyz| {
        let sum = value.x + value.y + value.z;
        (value.x / sum, value.y / sum)
    };
    for (kelvin, expected) in [(3200., (0.4234, 0.3990)), (6500., (0.3135, 0.3237))] {
        let (x, y) = chromaticity(white_target_xyz(WhiteTarget { kelvin, duv: 0. }));
        assert!((x - expected.0).abs() < 1.5e-3 && (y - expected.1).abs() < 1.5e-3);
    }
    let v = |duv: f32| {
        let value = white_target_xyz(WhiteTarget { kelvin: 3200., duv });
        6. * value.y / (value.x + 15. * value.y + 3. * value.z)
    };
    // Positive Duv is above the locus (toward green); CIE 1960 v rises by exactly Duv·cosθ.
    assert!(v(0.01) > v(0.) && v(-0.01) < v(0.));
    assert!((v(0.01) - v(0.) - 0.0098).abs() < 0.001);
}

#[test]
fn rgbwa_and_rgbal_fit_3200k_duv_exactly_with_quantized_output() {
    let white = white_target_xyz(WhiteTarget::default());
    let amber = xyz(
        0.5752 / 0.4242 * 0.4,
        0.4,
        (1. - 0.5752 - 0.4242) / 0.4242 * 0.4,
    );
    let lime = xyz(0.40, 0.80, 0.10);
    for extra in [
        vec![("color.white", Some(white)), ("color.amber", Some(amber))],
        vec![("color.amber", Some(amber)), ("color.lime", Some(lime))],
    ] {
        let mut fit = Fit::new(additive(&extra, PhysicalDataQuality::Manufacturer));
        let current = fit.zero();
        let mut warm = intent([1., 1., 1.]);
        warm.white_blend = 1.;
        warm.white_target = WhiteTarget {
            kelvin: 3200.,
            duv: 0.005,
        };
        let first = fit.run(&current, &warm).clone();
        assert_eq!(first.visible.status, VisibleFitStatus::Fitted);
        assert_eq!(first.visible.color_match, ColorMatch::Exact, "{extra:?}");
        assert!(first.visible.delta_uv.unwrap() < 0.002);
        assert_eq!(
            first.requested_white,
            Some(white_target_xyz(warm.white_target))
        );
        // Deterministic: identical inputs give identical writes and reports.
        let second = fit.run(&current, &warm).clone();
        assert_eq!(first, second);
        // Orange stays in gamut on both amber hybrids.
        let orange = fit.run(&current, &intent([1., 0.5, 0.])).clone();
        assert_eq!(orange.visible.color_match, ColorMatch::Exact, "{extra:?}");
    }
}

fn rgbwauv(uv: impl FnOnce(&mut OpticalEmitter)) -> FixtureProfile {
    let white = white_target_xyz(WhiteTarget::default());
    let mut b = Builder::new();
    let [r, g, bl] = rgb_columns();
    let nominal = PhysicalDataQuality::Estimated;
    b.emitter("color.red", Some(r), nominal);
    b.emitter("color.green", Some(g), nominal);
    b.emitter("color.blue", Some(bl), nominal);
    b.emitter("color.white", Some(white), nominal);
    b.emitter("color.amber", Some(xyz(0.54, 0.4, 0.0)), nominal);
    b.emitter_with("color.uv", ChannelResolution::U8, uv);
    b.build()
}

#[test]
fn uv_is_frozen_independent_drive_and_never_borrowed_for_purple() {
    let mut fit = Fit::new(rgbwauv(|_| {}));
    let current = fit.zero();
    let purple = intent([1., 0., 1.]);
    let off = fit.run(&current, &purple).clone();
    let visible_off = fit.raws(1..=5);
    assert_eq!(fit.raw(6), Some(0), "a zero request actively closes UV");
    assert!(
        off.writes
            .iter()
            .any(|w| w.channel_index == 6 && matches!(w.role, ColorWriteRole::Ultraviolet { .. }))
    );
    assert_eq!(off.visible.status, VisibleFitStatus::Fitted);
    assert!(
        off.visible.nominal,
        "nominal emitters are never presented as measured"
    );
    assert_eq!(off.uv.status, UvFitStatus::Applied);

    let mut with_uv = purple.clone();
    with_uv.uv = UvIntent { amount: 0.5 };
    let on = fit.run(&current, &with_uv).clone();
    assert_eq!(fit.raw(6), Some(128));
    assert_eq!(
        fit.raws(1..=5),
        visible_off,
        "UV never changes the visible solution"
    );
    assert_eq!(on.visible.status, VisibleFitStatus::PredictionIncomplete);
    assert_eq!(on.total_quality, PhysicalDataQuality::Unknown);
    assert!(!on.uv.appearance_known);
    assert!(
        on.limitations
            .contains(ColorFitLimitations::UNKNOWN_UV_APPEARANCE)
    );
    assert!((on.uv.achieved_drive.unwrap() - 128. / 255.).abs() < 1e-12);

    // UV-only black is valid and relativeOutput never scales UV.
    let mut uv_only = purple.clone();
    uv_only.relative_output = 0.;
    uv_only.uv.amount = 1.;
    let result = fit.run(&current, &uv_only).clone();
    assert_eq!(result.status, ColorFitStatus::Fitted);
    assert_eq!(fit.raws(1..=6), [0, 0, 0, 0, 0, 255].map(Some));
    uv_only.relative_output = 0.25;
    uv_only.uv.amount = 0.5;
    fit.run(&current, &uv_only);
    assert_eq!(fit.raw(6), Some(128));
}

#[test]
fn uv_limits_direction_and_known_leakage_are_honored() {
    let mut fit = Fit::new(rgbwauv(|e| {
        e.native_reversed = true;
        e.maximum_level = 0.5;
    }));
    let current = fit.zero();
    let mut request = intent([1., 0., 1.]);
    request.uv.amount = 1.;
    let result = fit.run(&current, &request).clone();
    // The declared maximum is never exceeded by rounding: 127/255, reversed.
    assert_eq!(fit.raw(6), Some(255 - 127));
    assert!(result.uv.clipped);
    assert!(result.limitations.contains(ColorFitLimitations::UV_CLIPPED));

    // Known violet leakage is part of the total forward XYZ and is compensated, not added twice.
    let leak = xyz(0.02, 0.005, 0.1);
    let mut fit = Fit::new(rgbwauv(|e| {
        e.xyz = Some(leak);
        e.provenance = provenance(PhysicalDataQuality::Estimated);
    }));
    let result = fit.run(&current, &request).clone();
    assert_eq!(fit.raw(6), Some(255));
    assert_eq!(result.visible.status, VisibleFitStatus::Fitted);
    assert_eq!(result.visible.color_match, ColorMatch::Exact);
    assert!(!result.visible.luminance_limited);
    assert!(result.visible.delta_uv.unwrap() < 0.002);

    // A head without UV keeps the request and reports it as unsupported.
    let mut fit = Fit::new(additive(&[], PhysicalDataQuality::Manufacturer));
    let result = fit.run(&fit.zero(), &request).clone();
    assert_eq!(result.uv.status, UvFitStatus::Unsupported);
    assert_eq!(result.uv.requested, 1.);
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::UV_UNSUPPORTED)
    );
}

/// TL-573: one linear U16 visible emitter and an independent linear U16 UV emitter whose known
/// visible leakage is a fixed offset once UV is frozen.
fn visible_plus_known_uv_leakage() -> FixtureProfile {
    let mut b = Builder::new();
    for (attribute, value) in [
        ("color.white", xyz(0.4, 0.5, 0.1)),
        ("color.uv", xyz(0.1, 0.05, 0.4)),
    ] {
        b.emitter_with(attribute, ChannelResolution::U16, |e| {
            e.xyz = Some(value);
            e.provenance = provenance(PhysicalDataQuality::Measured);
        });
    }
    b.build()
}

#[test]
fn fixed_uv_leakage_is_fitted_as_part_of_the_final_chromaticity() {
    let mut fit = Fit::new(visible_plus_known_uv_leakage());
    let current = fit.zero();
    let mut request = ColorIntent {
        base_xyz: xyz(0.6, 0.6, 0.9),
        white_blend: 0.,
        relative_output: 1.,
        uv: UvIntent { amount: 1. },
        ..Default::default()
    };
    request.recipe.approximate = true;
    request.validate().unwrap();
    let result = fit.run(&current, &request).clone();

    // UV is exactly the requested drive and never a visible fitting variable.
    assert_eq!(fit.raw(2), Some(65535));
    assert!(
        result
            .writes
            .iter()
            .any(|w| w.channel_index == 2 && matches!(w.role, ColorWriteRole::Ultraviolet { .. }))
    );
    assert_eq!(result.uv.status, UvFitStatus::Applied);
    assert!(result.uv.appearance_known);
    assert_eq!(result.uv.achieved_drive, Some(1.));
    // The requested visible target is reported unchanged, never rewritten to the achievable one.
    assert_eq!(result.requested_visible, Some(xyz(0.6, 0.6, 0.9)));

    // Total output (visible + leakage) reaches the requested chromaticity at half drive:
    // 0.5·(0.4, 0.5, 0.1) + (0.1, 0.05, 0.4) = 0.5·(0.6, 0.6, 0.9). Full drive would leave
    // Δu'v' ≈ 0.0439; the reachable optimum is limited only by U16 quantization.
    let visible = fit.raw(1).unwrap();
    assert!(visible.abs_diff(32768) <= 2, "visible raw {visible}");
    assert_eq!(result.visible.status, VisibleFitStatus::Fitted);
    let delta = result.visible.delta_uv.unwrap();
    assert!(delta < 1e-5, "final Δu'v' {delta}");
    assert_eq!(result.visible.color_match, ColorMatch::Exact);
    // Chromaticity comes before luminance: the exact hue is only reachable at half luminance.
    assert!((result.visible.luminance_ratio.unwrap() - 0.5).abs() < 1e-3);
    assert!(result.visible.luminance_limited);

    // The frozen UV drive does not depend on the visible request.
    let mut other = request.clone();
    other.base_xyz = xyz(0.2, 0.3, 0.1);
    other.validate().unwrap();
    fit.run(&current, &other);
    assert_eq!(fit.raw(2), Some(65535));
}

/// TL-592: the solver publishes its measured work; frozen UV leakage runs the bounded level
/// search (1 + 16 grid + 2 + 24 golden-section level solves), a zero offset one level solve.
#[test]
fn solver_work_counters_measure_the_fixed_uv_level_search() {
    let mut fit = Fit::new(visible_plus_known_uv_leakage());
    let current = fit.zero();
    let mut request = ColorIntent {
        base_xyz: xyz(0.6, 0.6, 0.9),
        uv: UvIntent { amount: 1. },
        ..Default::default()
    };
    request.recipe.approximate = true;
    let fixed = fit.run(&current, &request).work;
    assert_eq!(
        fixed,
        ColorFitWork {
            visible_solves: 1,
            fixed_offset_solves: 1,
            level_solves: 43,
            forward_evaluations: 2,
        }
    );
    request.uv.amount = 0.;
    let free = fit.run(&current, &request).work;
    assert_eq!((free.fixed_offset_solves, free.level_solves), (0, 1));
    // Filter-only paths rank combinations without any visible solve.
    let mut fit = Fit::new(cmy());
    let result = fit.run(&fit.zero(), &intent([1., 0., 0.])).clone();
    assert_eq!(result.candidates_ranked, 27);
    assert_eq!(result.work.visible_solves, 0);
    assert!(result.work.forward_evaluations <= COLOR_FIT_REEVALUATED_CANDIDATES as u32 + 1);
}

fn cmy() -> FixtureProfile {
    let mut b = Builder::new();
    b.source = Some(flat_source());
    for (attribute, blocked) in [
        ("color.cyan", 590..=830),
        ("color.magenta", 490..=589),
        ("color.yellow", 360..=489),
    ] {
        let half = blocked.clone();
        let full = blocked.clone();
        b.filter(
            attribute,
            255,
            vec![
                (0, 0, spectrum(|_| 1.)),
                (
                    1,
                    127,
                    spectrum(move |nm| if half.contains(&nm) { 0.5 } else { 1. }),
                ),
                (
                    128,
                    255,
                    spectrum(move |nm| if full.contains(&nm) { 0.02 } else { 1. }),
                ),
            ],
            false,
        );
    }
    b.build()
}

#[test]
fn cmy_selects_measured_filter_states_and_reports_unattainable_black() {
    let mut fit = Fit::new(cmy());
    let mut current = fit.zero();
    let white = fit.run(&current, &intent([1., 1., 1.])).clone();
    assert_eq!(fit.raws(1..=3), [Some(0), Some(0), Some(0)]);
    assert!(
        white
            .writes
            .iter()
            .all(|w| matches!(w.role, ColorWriteRole::Filter { .. }))
    );
    let red = fit.run(&current, &intent([1., 0., 0.])).clone();
    assert_eq!(fit.raws(1..=3), [Some(0), Some(191), Some(191)]);
    assert_eq!(red.visible.status, VisibleFitStatus::Fitted);
    assert!(red.visible.delta_uv.is_some());
    // A current value already inside the chosen steady range is kept.
    current[2] = 200;
    fit.run(&current, &intent([1., 0., 0.]));
    assert_eq!(fit.raw(2), Some(200));

    let mut black = intent([1., 1., 1.]);
    black.relative_output = 0.;
    let result = fit.run(&fit.zero(), &black).clone();
    assert_eq!(fit.raws(1..=3), [Some(191), Some(191), Some(191)]);
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::BLACK_UNATTAINABLE)
    );
    assert_eq!(result.visible.color_match, ColorMatch::OutOfGamut);
}

fn pass(from: u32, to: u32) -> Vec<SpectrumSample> {
    spectrum(move |nm| if (from..=to).contains(&nm) { 1. } else { 0.02 })
}

fn wheel(rotation: bool) -> Builder {
    let mut b = Builder::new();
    b.filter(
        "color.wheel.1",
        127,
        vec![
            (0, 15, spectrum(|_| 1.)),
            (16, 31, pass(590, 830)),
            (32, 47, pass(360, 499)),
        ],
        rotation,
    );
    b
}

fn identity(fit: &Fit) -> NativeColorIdentity {
    fit.profile
        .native_color_identity(fit.profile.modes[0].id, fit.profile.modes[0].heads[0].id)
        .unwrap()
}

fn pin(fit: &Fit, raw: u32, source: NativeColorIdentity) -> ColorWheelConstraint {
    let channel = &fit.profile.modes[0].channels[1];
    let function = channel
        .functions
        .iter()
        .find(|f| (f.dmx_from..=f.dmx_to).contains(&raw))
        .unwrap();
    ColorWheelConstraint {
        source,
        value: NativeColorValue {
            channel_id: channel.id,
            function_id: function.id,
            raw,
        },
    }
}

#[test]
fn wheel_only_uses_stable_steady_slots_and_pinned_constraints() {
    let mut b = wheel(true);
    b.source = Some(flat_source());
    let mut fit = Fit::new(b.build());
    let mut current = fit.zero();
    let blue = fit.run(&current, &intent([0., 0., 1.])).clone();
    assert_eq!(fit.raw(1), Some(39), "slot center");
    assert_eq!(blue.visible.status, VisibleFitStatus::Fitted);
    current[1] = 33;
    fit.run(&current, &intent([0., 0., 1.]));
    assert_eq!(
        fit.raw(1),
        Some(33),
        "no wheel movement inside the chosen slot"
    );
    // Rotation is never a steady candidate.
    current[1] = 200;
    fit.run(&current, &intent([1., 1., 1.]));
    assert_eq!(fit.raw(1), Some(7));

    let own = identity(&fit);
    let mut request = intent([0., 0., 1.]);
    request.wheel_constraints = vec![pin(&fit, 20, own.clone())];
    let pinned = fit.run(&fit.zero(), &request).clone();
    assert_eq!(fit.raw(1), Some(20));
    assert_eq!(pinned.writes[0].role, ColorWriteRole::Constrained);
    assert_eq!(pinned.constraints[0].status, ColorConstraintStatus::Applied);
    assert_eq!(pinned.visible.status, VisibleFitStatus::Fitted);
    assert_eq!(pinned.visible.color_match, ColorMatch::OutOfGamut);

    let mut foreign = own.clone();
    foreign.head_id = Uuid::new_v4();
    request.wheel_constraints = vec![pin(&fit, 20, foreign)];
    let result = fit.run(&fit.zero(), &request).clone();
    assert_eq!(
        fit.raw(1),
        Some(39),
        "a foreign constraint is reported, not applied"
    );
    assert_eq!(
        result.constraints[0].status,
        ColorConstraintStatus::SourceMismatch
    );
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::CONSTRAINT_REJECTED)
    );

    request.wheel_constraints = vec![pin(&fit, 200, own)];
    let result = fit.run(&fit.zero(), &request).clone();
    assert_eq!(fit.raw(1), Some(200));
    assert_eq!(
        result.constraints[0].status,
        ColorConstraintStatus::AppliedUnknownState
    );
    assert_eq!(result.visible.status, VisibleFitStatus::UnknownAppearance);
}

#[test]
fn unknown_wheel_calibration_is_retained_rather_than_fabricated() {
    let mut b = wheel(false);
    b.source = Some(flat_source());
    b.filters[0].transmission = OpticalTransmission::Unknown;
    let mut fit = Fit::new(b.build());
    let current = fit.zero();
    let result = fit.run(&current, &intent([1., 0., 0.])).clone();
    assert_eq!(result.status, ColorFitStatus::Fitted);
    assert_eq!(result.visible.status, VisibleFitStatus::UnknownAppearance);
    assert!(result.writes.is_empty());
    assert_eq!(
        result.retained[0].reason,
        ColorRetainReason::UnknownAppearance
    );
    assert!(result.visible.achieved.is_none());
    assert_eq!(result.total_quality, PhysicalDataQuality::Unknown);
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::UNKNOWN_FILTER)
    );
}

#[test]
fn unknown_source_stays_unknown_without_fabricated_output() {
    let mut b = wheel(false);
    b.source = Some(OpticalSource::Unknown);
    let mut fit = Fit::new(b.build());
    let result = fit.run(&fit.zero(), &intent([0., 0., 1.])).clone();
    assert_eq!(result.visible.status, VisibleFitStatus::UnknownAppearance);
    assert!(result.writes.is_empty());
    assert_eq!(result.visible.color_match, ColorMatch::Unknown);
}

#[test]
fn measured_whole_path_recipe_competes_as_an_exact_candidate() {
    let mut b = wheel(false);
    b.source = Some(flat_source());
    let mut profile = b.build();
    let target = srgb_to_xyz(1., 0., 0.);
    let channel = &profile.modes[0].channels[1];
    let recipe = vec![NativeColorValue {
        channel_id: channel.id,
        function_id: channel.functions[0].id,
        raw: 24,
    }];
    let path = &mut profile.modes[0].color_physical.as_mut().unwrap().paths[0];
    path.measurements.push(ColorRecipeMeasurement {
        recipe,
        xyz: target,
        provenance: provenance(PhysicalDataQuality::Measured),
    });
    let mut fit = Fit::new(profile);
    let result = fit.run(&fit.zero(), &intent([1., 0., 0.])).clone();
    assert_eq!(fit.raw(1), Some(24));
    assert_eq!(result.writes[0].role, ColorWriteRole::MeasuredRecipe);
    assert_eq!(result.visible.achieved, Some(target));
    assert_eq!(result.visible.color_match, ColorMatch::Exact);
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::MEASURED_RECIPE)
    );
}

fn hybrid() -> FixtureProfile {
    let mut b = wheel(false);
    for (attribute, from, to) in [
        ("color.red", 610, 650),
        ("color.green", 510, 550),
        ("color.blue", 440, 480),
    ] {
        b.emitter_with(attribute, ChannelResolution::U8, |e| {
            e.spectrum = spectrum(move |nm| if (from..=to).contains(&nm) { 0.03 } else { 0. });
            e.provenance = provenance(PhysicalDataQuality::Measured);
        });
    }
    b.build()
}

#[test]
fn hybrid_prefers_a_clear_wheel_and_fits_emitters_through_a_pinned_filter() {
    let mut fit = Fit::new(hybrid());
    let current = fit.zero();
    let magenta = fit.run(&current, &intent([1., 0., 1.])).clone();
    // The open slot already holds the current raw 0, so the wheel does not move.
    assert_eq!(
        fit.raw(1),
        Some(0),
        "open slot when the emitters reach the color"
    );
    assert_eq!(magenta.visible.status, VisibleFitStatus::Fitted);
    assert!(fit.raw(2).unwrap() > 0 && fit.raw(4).unwrap() > 0);
    assert!(fit.raw(3).unwrap() < fit.raw(2).unwrap());
    assert!(magenta.candidates_ranked >= 3);
    // From the red slot the wheel returns to the open slot's center.
    let mut from_red = current.clone();
    from_red[1] = 20;
    fit.run(&from_red, &intent([1., 0., 1.]));
    assert_eq!(fit.raw(1), Some(7));

    let mut request = intent([1., 0., 1.]);
    request.wheel_constraints = vec![pin(&fit, 20, identity(&fit))];
    let result = fit.run(&current, &request).clone();
    assert_eq!(fit.raw(1), Some(20));
    assert_eq!(result.visible.status, VisibleFitStatus::Fitted);
    assert!(fit.raw(2).unwrap() > 0, "red LEDs pass the red filter");
}

#[test]
fn candidate_tables_and_scratch_are_bounded_for_repeated_solves() {
    let mut b = Builder::new();
    b.source = Some(flat_source());
    for wheel in 1..=3 {
        let samples = (0..20)
            .map(|slot| {
                (
                    slot * 10,
                    slot * 10 + 9,
                    spectrum(move |nm| if nm % 20 == slot { 0.5 } else { 1. }),
                )
            })
            .collect();
        b.filter(&format!("color.wheel.{wheel}"), 255, samples, false);
    }
    let mut fit = Fit::new(b.build());
    assert_eq!(fit.fitting.candidate_combinations(0), 0);
    let result = fit.run(&fit.zero(), &intent([1., 0., 0.])).clone();
    assert_eq!(result.visible.status, VisibleFitStatus::CandidateLimit);
    assert!(
        result
            .limitations
            .contains(ColorFitLimitations::CANDIDATE_LIMIT)
    );
    assert!(
        result
            .retained
            .iter()
            .all(|r| r.reason == ColorRetainReason::CandidateLimit)
    );

    let mut fit = Fit::new(hybrid());
    assert!(fit.fitting.candidate_combinations(0) <= COLOR_FIT_MAX_CONTINUOUS_COMBINATIONS);
    let capacity = (
        fit.output.writes.capacity(),
        fit.output.constraints.capacity(),
    );
    let current = fit.zero();
    for step in 0..50 {
        let mut request = intent([1., step as f32 / 49., 0.]);
        request.uv.amount = 0.;
        fit.run(&current, &request);
    }
    assert_eq!(
        (
            fit.output.writes.capacity(),
            fit.output.constraints.capacity()
        ),
        capacity
    );
}

#[test]
fn invalid_or_unresolved_requests_write_nothing_and_inputs_are_checked() {
    let mut fit = Fit::new(additive(&[], PhysicalDataQuality::Manufacturer));
    let current = fit.zero();
    let mut spread = intent([1., 0., 0.]);
    spread.spreads = vec![ColorComponentSpread {
        component: ColorComponent::Red,
        points: vec![0., 1.],
    }];
    let result = fit.run(&current, &spread).clone();
    assert_eq!(result.status, ColorFitStatus::InvalidRequest);
    assert!(result.writes.is_empty() && result.retained.is_empty());
    let mut bad = intent([1., 0., 0.]);
    bad.relative_output = f32::NAN;
    assert_eq!(
        fit.run(&current, &bad).status,
        ColorFitStatus::InvalidRequest
    );

    let request = intent([1., 0., 0.]);
    let Fit {
        fitting,
        workspace,
        output,
        ..
    } = &mut fit;
    assert_eq!(
        fitting.fit(0, &current[1..], &request, workspace, output),
        Err(ColorFitInputError::ChannelCount)
    );
    assert_eq!(
        fitting.fit(1, &current, &request, workspace, output),
        Err(ColorFitInputError::HeadIndex)
    );
    let mut foreign = output.clone();
    foreign.head_id = Uuid::new_v4();
    assert_eq!(
        fitting.fit(0, &current, &request, workspace, &mut foreign),
        Err(ColorFitInputError::OutputLayout)
    );
}
