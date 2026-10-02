//! TL-559 Direct replay through the Color adapter on real captured frames: exact native replay
//! (AC1), identity-only compatibility and fallback fitting (AC2/AC3), per-tick forward
//! evaluation of changing recipes (AC5) and partial visible/UV knowledge (AC8/AC9).
use super::profiles::*;
use super::tests::{intent, program};
use super::*;
use light_core::programming::{
    DirectIncompatibility, NativeColorObservation, NativeColorRecipe, NativeColorSpread,
    PortableUv, PortableVisibleColor,
};
use light_core::{ManualClock, NativeColorBinding, NativeColorValue};
use light_engine::{
    Engine, EngineSnapshot, NativeColorSourceCatalog, NativeColorSourceRevisionKey,
    PreparedOutputFrame, RenderOptions,
};
use light_fixture::{ChannelResolution, FixtureChannel, FixtureProfile, OpticalEmitterBand};
use light_programmer::ProgrammerRegistry;

pub(in crate::runtime) fn catalogue(profiles: &[&FixtureProfile]) -> Arc<NativeColorSourceCatalog> {
    Arc::new(
        NativeColorSourceCatalog::from_revisions(profiles.iter().map(|profile| {
            NativeColorSourceCatalog::compile_revision(
                NativeColorSourceRevisionKey {
                    profile_id: profile.id,
                    revision: u64::from(profile.revision),
                    raw_store_digest: format!("tl559-{}-{}", profile.id.0, profile.revision),
                },
                None,
                || Ok((**profile).clone()),
            )
        }))
        .unwrap(),
    )
}

pub(in crate::runtime) fn identity(profile: &FixtureProfile) -> light_core::NativeColorIdentity {
    let mode = &profile.modes[0];
    profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap()
}

/// The head's native path controls in path order.
pub(in crate::runtime) fn path_channels(profile: &FixtureProfile) -> Vec<&FixtureChannel> {
    let mode = &profile.modes[0];
    let path = &mode.color_physical.as_ref().unwrap().paths[0];
    path.controls
        .iter()
        .map(|id| mode.channels.iter().find(|c| c.id == *id).unwrap())
        .collect()
}

/// Capture a Direct value against the retained original, raws in path order.
pub(in crate::runtime) fn direct(
    catalogue: &NativeColorSourceCatalog,
    profile: &FixtureProfile,
    raws: &[u32],
) -> AttributeValue {
    let channels = path_channels(profile);
    assert_eq!(channels.len(), raws.len());
    let values = channels
        .iter()
        .zip(raws)
        .map(|(channel, raw)| NativeColorValue {
            channel_id: channel.id,
            function_id: channel
                .functions
                .iter()
                .find(|f| (f.dmx_from..=f.dmx_to).contains(raw))
                .unwrap()
                .id,
            raw: *raw,
        })
        .collect();
    catalogue
        .capture_direct(NativeColorObservation {
            source: identity(profile),
            values,
        })
        .unwrap()
        .into_value()
}

pub(in crate::runtime) fn direct_program(value: &AttributeValue) -> &Arc<ColorProgram> {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("Color program")
    };
    program
}

pub(super) struct DirectRig {
    pub engine: Engine,
    clock: Arc<ManualClock>,
    pub target: FixtureId,
    pub adapter: ColorAdapter,
    pub catalogue: RefCell<Arc<NativeColorSourceCatalog>>,
    /// Compiled once per installed generation, as the lane does.
    descriptor: RefCell<Option<std::rc::Rc<ColorDescriptor>>>,
}

pub(super) struct Resolved {
    pub capture: PreparedOutputFrame,
    pub descriptor: std::rc::Rc<ColorDescriptor>,
    pub native: Vec<u32>,
    pub result: PhysicalResolution<ColorAdapter>,
}

impl DirectRig {
    pub fn new(destination: &FixtureProfile, retained: &[&FixtureProfile]) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let rig = Self {
            engine: Engine::new(ProgrammerRegistry::with_clock(clock.clone())),
            clock,
            target: FixtureId::new(),
            adapter: ColorAdapter::default(),
            catalogue: RefCell::new(catalogue(retained)),
            descriptor: RefCell::new(None),
        };
        rig.install(destination, retained);
        rig
    }

    /// Patch `destination` and retain `retained` as the generation's original catalogue.
    pub fn install(&self, destination: &FixtureProfile, retained: &[&FixtureProfile]) {
        let catalogue = catalogue(retained);
        *self.catalogue.borrow_mut() = Arc::clone(&catalogue);
        *self.descriptor.borrow_mut() = None;
        self.engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![patched(destination, self.target, 1)].into(),
                revision: 1,
                native_color_sources: catalogue,
                ..Default::default()
            })
            .unwrap();
    }

    /// Patch `validated` (the frame) but compile the descriptor once from `unpatched` of the
    /// same mode, as the TL-557 root-owning-two-heads topology does.
    pub fn install_topology(
        &self,
        validated: light_fixture::PatchedFixture,
        unpatched: light_fixture::PatchedFixture,
        retained: &[&FixtureProfile],
    ) {
        let catalogue = catalogue(retained);
        *self.catalogue.borrow_mut() = Arc::clone(&catalogue);
        let compiled = self
            .adapter
            .compile(
                &EngineSnapshot {
                    fixtures: vec![unpatched].into(),
                    revision: 1,
                    ..Default::default()
                },
                self.target,
            )
            .unwrap()
            .expect("physical Color destinations");
        *self.descriptor.borrow_mut() = Some(std::rc::Rc::new(compiled));
        self.engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![validated].into(),
                revision: 1,
                native_color_sources: catalogue,
                ..Default::default()
            })
            .unwrap();
    }

    pub fn resolve(
        &self,
        value: &AttributeValue,
        previous: Option<&ColorContinuity>,
    ) -> Result<Resolved, TransitionError> {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(RenderOptions::default());
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let catalogue = Arc::clone(&self.catalogue.borrow());
        let descriptor = match self.descriptor.borrow().clone() {
            Some(descriptor) => descriptor,
            None => std::rc::Rc::new(
                self.adapter
                    .compile(&capture.snapshot(), self.target)?
                    .expect("physical Color destination"),
            ),
        };
        *self.descriptor.borrow_mut() = Some(descriptor.clone());
        let native = scalar
            .native_raw(&capture, &token, self.target)
            .unwrap()
            .raw()
            .to_vec();
        let result = self.adapter.resolve(PhysicalRequest {
            frame: HybridFrameContext {
                capture: &capture,
                geometry: &geometry,
                native_models: catalogue.as_ref(),
                token: &token,
                scalar: &scalar,
            },
            target: self.target,
            owner: ProgrammingOwner::Color,
            descriptor: &descriptor,
            value,
            previous,
        })?;
        validate_complete_writes(&descriptor.footprint, &result.writes)?;
        Ok(Resolved {
            capture,
            descriptor,
            native,
            result,
        })
    }
}

impl Resolved {
    pub fn raw(&self, channel_id: Uuid) -> u32 {
        self.write(channel_id).raw
    }

    pub fn write(&self, channel_id: Uuid) -> &NativeControlWrite {
        self.result
            .writes
            .iter()
            .find(|w| w.channel_id == channel_id)
            .expect("Color control written")
    }

    pub fn direct(&self) -> &DirectColorStatus {
        self.result.quality.direct.as_ref().expect("Direct status")
    }

    /// The published achieved output is the forward evaluation of exactly the written values.
    pub fn assert_forward(&self) {
        let mut output = self.native.clone();
        for write in &self.result.writes {
            output[write.slot.channel_index as usize] = write.raw;
        }
        let head = self.descriptor.primary();
        let mut forward = head.fitting.forward().create_output();
        head.fitting
            .forward()
            .evaluate(&output, &mut forward)
            .unwrap();
        assert_eq!(self.result.achieved.known_xyz, forward[head.head].known_xyz);
    }
}

fn width_emitter(builder: &mut Builder, attribute: &str, resolution: ChannelResolution, xyz: Xyz) {
    builder.emitter_with(attribute, resolution, |e| e.xyz = Some(xyz));
}

/// RGBW with Red U8, Green U16, Blue U24, White U32.
pub(in crate::runtime) fn rgbw_widths() -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new("TL-559 RGBW widths");
    width_emitter(&mut builder, "color.red", ChannelResolution::U8, r);
    width_emitter(&mut builder, "color.green", ChannelResolution::U16, g);
    width_emitter(&mut builder, "color.blue", ChannelResolution::U24, b);
    width_emitter(&mut builder, "color.white", ChannelResolution::U32, white());
    builder.build()
}

/// RGBAL with Red U32, Green U24, Blue U16, Amber and Lime U8.
fn rgbal_widths() -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new("TL-559 RGBAL widths");
    width_emitter(&mut builder, "color.red", ChannelResolution::U32, r);
    width_emitter(&mut builder, "color.green", ChannelResolution::U24, g);
    width_emitter(&mut builder, "color.blue", ChannelResolution::U16, b);
    width_emitter(&mut builder, "color.amber", ChannelResolution::U8, amber());
    width_emitter(
        &mut builder,
        "color.lime",
        ChannelResolution::U8,
        xyz(0.40, 0.80, 0.10),
    );
    builder.build()
}

/// CMY flags at U16/U24/U32 (clear / half / full) and a U8 wheel.
fn cmy_wheel_widths() -> FixtureProfile {
    let mut builder = Builder::new("TL-559 CMY widths").fixed_source();
    for (attribute, resolution, blocked) in [
        ("color.cyan", ChannelResolution::U16, 590..=830),
        ("color.magenta", ChannelResolution::U24, 490..=589),
        ("color.yellow", ChannelResolution::U32, 360..=489),
    ] {
        let max = resolution.max_raw();
        let (half, full) = (blocked.clone(), blocked);
        builder = builder.filter_with(
            attribute,
            resolution,
            max,
            vec![
                (0, 0, spectrum(|_| 1.)),
                (
                    1,
                    max / 2,
                    spectrum(move |nm| if half.contains(&nm) { 0.5 } else { 1. }),
                ),
                (
                    max / 2 + 1,
                    max,
                    spectrum(move |nm| if full.contains(&nm) { 0.02 } else { 1. }),
                ),
            ],
            false,
        );
    }
    builder
        .filter("color.wheel.1", 127, wheel_slots(), false)
        .build()
}

/// A U16 wheel (open, red, blue) with a rotation range.
fn wheel_only_u16() -> FixtureProfile {
    Builder::new("TL-559 U16 wheel")
        .fixed_source()
        .filter_with(
            "color.wheel.1",
            ChannelResolution::U16,
            32767,
            vec![
                (0, 4095, spectrum(|_| 1.)),
                (4096, 8191, pass(590, 830)),
                (8192, 12287, pass(360, 499)),
            ],
            true,
        )
        .build()
}

/// AC1: every participating control is reset from the recipe, overriding a previous semantic
/// solution, at every width, and the published achieved output is its forward evaluation.
#[test]
fn exact_replay_resets_every_color_control_for_rgbw_rgbal_cmy_wheel_and_wheel_only_at_all_widths() {
    let cases: Vec<(FixtureProfile, Vec<Vec<u32>>)> = vec![
        (
            rgbw_widths(),
            vec![
                vec![255, 65535, 16_777_215, u32::MAX],
                vec![0; 4],
                vec![1, 2, 3, 4],
            ],
        ),
        (
            rgbal_widths(),
            vec![vec![u32::MAX, 0, 65535, 17, 255], vec![0; 5]],
        ),
        (
            cmy_wheel_widths(),
            vec![vec![300, 16_777_215, 0, 20], vec![65535, 0, u32::MAX, 40]],
        ),
        (wheel_only_u16(), vec![vec![5000], vec![0], vec![20000]]),
        (rgbw(), vec![vec![65535, 0, 255, 255]]),
        (rgbal(), vec![vec![0, 255, 0, 255, 128]]),
        (cmy_wheel(), vec![vec![200, 0, 90, 20]]),
        (wheel_only(), vec![vec![36]]),
    ];
    for (profile, recipes) in &cases {
        let rig = DirectRig::new(profile, &[profile]);
        let seeded = rig
            .resolve(&program(&intent([1., 0.5, 0.2], 0.)), None)
            .unwrap();
        for raws in recipes {
            let value = direct(&rig.catalogue.borrow(), profile, raws);
            let resolved = rig
                .resolve(&value, Some(&seeded.result.continuity))
                .unwrap_or_else(|e| panic!("{}: {e:?}", profile.name));
            assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
            assert_eq!(resolved.direct().origin, DirectEstimateOrigin::Forward);
            assert_eq!(
                resolved.result.requested,
                ColorRequest::Direct(Arc::clone(direct_program(&value)))
            );
            for (channel, raw) in path_channels(profile).iter().zip(raws) {
                let write = resolved.write(channel.id);
                assert_eq!(write.raw, *raw, "{}: {}", profile.name, channel.attribute.0);
                assert!(!write.parked && write.function_id.is_some());
            }
            assert_eq!(
                resolved.result.writes.len(),
                raws.len(),
                "{}: every participating control once",
                profile.name
            );
            resolved.assert_forward();
        }
    }
    assert!(DirectRig::new(&rgbw(), &[]).adapter.counters().direct_exact == 0);
}

/// Revision/calibration changes keep native replay eligibility; the saved source estimate is
/// never recomputed from the destination's newer optical model.
#[test]
fn recalibrated_compatible_destination_replays_exactly_with_the_original_estimate() {
    let source = rgbw_widths();
    let mut recalibrated = source.clone();
    recalibrated.revision = 2;
    let path = &mut recalibrated.modes[0].color_physical.as_mut().unwrap().paths[0];
    if let light_fixture::OpticalSource::Additive { emitters } = &mut path.source {
        emitters[0].xyz = Some(xyz(0.30, 0.20, 0.02));
    }
    recalibrated.validate().unwrap();
    assert_ne!(identity(&source), identity(&recalibrated));
    let rig = DirectRig::new(&recalibrated, &[&source, &recalibrated]);
    let value = direct(&rig.catalogue.borrow(), &source, &[255, 0, 0, 0]);
    let ColorProgram::Direct { portable, .. } = direct_program(&value).as_ref() else {
        unreachable!()
    };
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    assert_eq!(
        &resolved.direct().estimate,
        portable,
        "source estimate kept"
    );
    assert_eq!(resolved.raw(path_channels(&source)[0].id), 255);
    assert_ne!(
        resolved.result.achieved.visible.map(|v| v.x),
        portable.visible.map(|v| v.xyz.x),
        "the destination's own forward model reports what it actually emits"
    );
}

fn fallback(resolved: &Resolved) -> (DirectCompatibility, VisibleFallback, UvFallback) {
    match &resolved.direct().replay {
        DirectReplayOutcome::Fallback {
            compatibility,
            visible,
            uv,
        } => (compatibility.clone(), *visible, *uv),
        DirectReplayOutcome::Exact => panic!("expected fallback"),
    }
}

fn close(a: Xyz, b: Xyz, tolerance: f32) -> bool {
    (a.x - b.x).abs() <= tolerance
        && (a.y - b.y).abs() <= tolerance
        && (a.z - b.z).abs() <= tolerance
}

/// AC2/AC3: compatibility is identity-only. A lookalike with identical names, attributes,
/// widths and slots, a changed native layout, another fixture type and an unverified
/// destination all fall back to fitting the source estimate; none replays raw values.
#[test]
fn identity_only_compatibility_falls_back_for_lookalike_changed_layout_other_type_and_unverified() {
    let source = rgbw();
    let mut lookalike = source.clone();
    lookalike.id = FixtureId::new();
    let mut changed = source.clone();
    changed.revision = 2;
    changed.modes[0].channels[3].default_raw = 7;
    changed.validate().unwrap();
    let other = rgbal();
    let raws = [40000, 30, 200, 0];
    let cases = [
        (
            &lookalike,
            vec![&source, &lookalike],
            DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource),
        ),
        (
            &changed,
            vec![&source, &changed],
            DirectCompatibility::Incompatible(DirectIncompatibility::ChangedLayout),
        ),
        (
            &other,
            vec![&source, &other],
            DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource),
        ),
    ];
    for (destination, retained, expected) in cases {
        let rig = DirectRig::new(destination, &retained);
        let value = direct(&rig.catalogue.borrow(), &source, &raws);
        let ColorProgram::Direct { portable, .. } = direct_program(&value).as_ref() else {
            unreachable!()
        };
        let resolved = rig.resolve(&value, None).unwrap();
        let (compatibility, visible, uv) = fallback(&resolved);
        assert_eq!(compatibility, expected, "{}", destination.name);
        assert!(matches!(visible, VisibleFallback::Fit(_)));
        assert!(matches!(uv, UvFallback::Apply(uv) if uv.amount == 0.));
        let target = portable.visible.unwrap().xyz;
        let achieved = resolved.result.achieved.visible.unwrap();
        assert!(
            close(achieved, target, 0.01),
            "{}: {achieved:?} vs {target:?}",
            destination.name
        );
        assert_eq!(rig.adapter.counters().direct_exact, 0);
        resolved.assert_forward();
    }
    // The destination model is not retained: compatibility is unknown, never assumed.
    let rig = DirectRig::new(&lookalike, &[&source]);
    let value = direct(&rig.catalogue.borrow(), &source, &raws);
    let resolved = rig.resolve(&value, None).unwrap();
    assert!(matches!(
        fallback(&resolved).0,
        DirectCompatibility::Unknown(_)
    ));
}

/// AC2/AC3: known black and reduced output are not normalized away on another fixture type.
#[test]
fn black_and_reduced_output_survive_incompatible_replay() {
    let source = rgbw();
    let destination = rgbal();
    let rig = DirectRig::new(&destination, &[&source, &destination]);
    let black = direct(&rig.catalogue.borrow(), &source, &[0, 0, 0, 0]);
    let resolved = rig.resolve(&black, None).unwrap();
    assert!(matches!(
        fallback(&resolved).1,
        VisibleFallback::Fit(PortableVisibleColor { xyz, .. }) if xyz.y == 0.
    ));
    assert!(resolved.result.writes.iter().all(|w| w.raw == 0), "black");
    assert_eq!(resolved.result.achieved.visible.map(|v| v.y), Some(0.));

    let full = direct(&rig.catalogue.borrow(), &source, &[65535, 255, 255, 0]);
    let dim = direct(&rig.catalogue.borrow(), &source, &[16384, 64, 64, 0]);
    let full = rig
        .resolve(&full, None)
        .unwrap()
        .result
        .achieved
        .visible
        .unwrap();
    let dim_resolved = rig.resolve(&dim, None).unwrap();
    let dim_achieved = dim_resolved.result.achieved.visible.unwrap();
    let ColorProgram::Direct { portable, .. } = direct_program(&dim).as_ref() else {
        unreachable!()
    };
    let expected = portable.visible.unwrap().xyz;
    assert!(
        dim_achieved.y < full.y * 0.4,
        "reduced output stays reduced"
    );
    assert!((dim_achieved.y - expected.y).abs() <= expected.y * 0.02);
}

/// A UV-leaking additive lamp with two independent UV banks (known violet leakage).
fn two_uv_banks(leak: Xyz) -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new("TL-559 two UV banks")
        .emitter("color.red", Some(r))
        .emitter("color.green", Some(g))
        .emitter("color.blue", Some(b));
    for attribute in ["color.uv", "color.uv.2"] {
        builder.emitter_with(attribute, ChannelResolution::U8, |e| {
            e.xyz = Some(leak);
            e.band = OpticalEmitterBand::Ultraviolet;
            e.provenance = provenance(light_fixture::PhysicalDataQuality::Estimated);
        });
    }
    builder.build()
}

/// RGB plus White of unknown appearance and a UV emitter with known leakage.
pub(in crate::runtime) fn unknown_white_uv(leak: Xyz) -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new("TL-559 unknown white")
        .emitter("color.red", Some(r))
        .emitter("color.green", Some(g))
        .emitter("color.blue", Some(b))
        .emitter("color.white", None);
    builder.emitter_with("color.uv", ChannelResolution::U8, |e| {
        e.xyz = Some(leak);
        e.provenance = provenance(light_fixture::PhysicalDataQuality::Estimated);
    });
    builder.build()
}

const LEAK: Xyz = Xyz {
    x: 0.02,
    y: 0.01,
    z: 0.08,
};

fn uv_channel(profile: &FixtureProfile) -> Uuid {
    profile.modes[0].channels.last().unwrap().id
}

/// AC9: unknown visible with known UV (0.5 and 0) holds only the visible solution (the last
/// accepted one, or safe defaults before a first solution) and applies the known UV request.
#[test]
fn unknown_visible_holds_visible_and_still_applies_known_uv_including_zero() {
    let leak_unknown = rgbwauv(None);
    let destination = rgbwauv(Some(LEAK));
    let rig = DirectRig::new(&destination, &[&leak_unknown, &destination]);
    let visible = path_channels(&destination)[..5]
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();

    // Visible null because the source's UV leakage is unknown; UV amount 128/255 is known.
    let uv_half = direct(
        &rig.catalogue.borrow(),
        &leak_unknown,
        &[0, 0, 0, 0, 0, 128],
    );
    let first = rig.resolve(&uv_half, None).unwrap();
    assert_eq!(
        fallback(&first).1,
        VisibleFallback::Hold,
        "unknown appearance is held, never white"
    );
    assert!(
        visible.iter().all(|id| first.raw(*id) == 0),
        "safe defaults"
    );
    assert_eq!(first.raw(uv_channel(&destination)), 128);
    assert_eq!(
        first.result.quality.visible,
        VisibleFitStatus::UnknownAppearance
    );

    // After a valid solution, the held visible output is that solution.
    let orange = rig
        .resolve(&program(&intent([1., 0.5, 0.], 0.)), None)
        .unwrap();
    let held = rig
        .resolve(&uv_half, Some(&orange.result.continuity))
        .unwrap();
    for id in &visible {
        assert_eq!(held.raw(*id), orange.raw(*id), "visible solution held");
    }
    assert_eq!(held.raw(uv_channel(&destination)), 128, "UV still applied");

    // Visible null from an active unknown White emitter; known UV zero is applied explicitly.
    let white_unknown = unknown_white_uv(LEAK);
    let rig = DirectRig::new(&destination, &[&white_unknown, &destination]);
    let mut uv_on = program(&intent([1., 0., 1.], 0.));
    if let AttributeValue::ColorProgram(p) = &mut uv_on
        && let ColorProgram::Semantic { intent } = Arc::make_mut(p)
    {
        intent.uv.amount = 0.9;
    }
    let before = rig.resolve(&uv_on, None).unwrap();
    assert!(before.raw(uv_channel(&destination)) > 200);
    let zero = direct(&rig.catalogue.borrow(), &white_unknown, &[0, 0, 0, 255, 0]);
    let resolved = rig.resolve(&zero, Some(&before.result.continuity)).unwrap();
    assert_eq!(
        fallback(&resolved),
        (
            DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource),
            VisibleFallback::Hold,
            UvFallback::Apply(PortableUv {
                amount: 0.,
                quality: light_fixture::PhysicalDataQuality::Estimated
            })
        )
    );
    let uv = resolved.write(uv_channel(&destination));
    assert_eq!((uv.raw, uv.parked), (0, true), "previous UV never leaks");
    for id in &visible {
        assert_eq!(resolved.raw(*id), before.raw(*id));
    }
    resolved.assert_forward();
}

/// AC9: known visible with unknown UV aggregation (unequal banks) fits visible and parks UV off
/// explicitly, without leaking the previous UV.
#[test]
fn known_visible_with_unknown_uv_fits_visible_and_parks_uv_off() {
    let source = two_uv_banks(LEAK);
    let destination = rgbwauv(Some(LEAK));
    let rig = DirectRig::new(&destination, &[&source, &destination]);
    let mut uv_on = program(&intent([0., 0., 1.], 0.));
    if let AttributeValue::ColorProgram(p) = &mut uv_on
        && let ColorProgram::Semantic { intent } = Arc::make_mut(p)
    {
        intent.uv.amount = 1.;
    }
    let before = rig.resolve(&uv_on, None).unwrap();
    assert_eq!(before.raw(uv_channel(&destination)), 255);
    let unequal = direct(&rig.catalogue.borrow(), &source, &[255, 0, 0, 255, 0]);
    let ColorProgram::Direct { portable, .. } = direct_program(&unequal).as_ref() else {
        unreachable!()
    };
    assert!(portable.uv.is_none() && portable.visible.is_some());
    let resolved = rig
        .resolve(&unequal, Some(&before.result.continuity))
        .unwrap();
    let (_, visible, uv) = fallback(&resolved);
    assert!(matches!(visible, VisibleFallback::Fit(_)));
    assert_eq!(uv, UvFallback::ParkOff);
    let write = resolved.write(uv_channel(&destination));
    assert_eq!((write.raw, write.parked), (0, true), "UV parked off");
    assert!(
        resolved
            .direct()
            .limitations
            .iter()
            .any(|l| l.contains("UV amount is unknown"))
    );
    // Visible fits the complete source total (its UV leakage included once).
    let target = portable.visible.unwrap().xyz;
    assert!(close(
        resolved.result.achieved.visible.unwrap(),
        target,
        0.01
    ));
}

/// AC8: a UV-only Direct recipe's visible estimate already includes the UV leakage. The
/// destination applies the same UV and fits the remaining visible light: it must not add the
/// leakage a second time with its visible emitters.
#[test]
fn uv_only_direct_never_double_counts_known_leakage() {
    let source = rgbwauv(Some(LEAK));
    let mut destination = source.clone();
    destination.id = FixtureId::new();
    let rig = DirectRig::new(&destination, &[&source, &destination]);
    let uv_only = direct(&rig.catalogue.borrow(), &source, &[0, 0, 0, 0, 0, 255]);
    let ColorProgram::Direct { portable, .. } = direct_program(&uv_only).as_ref() else {
        unreachable!()
    };
    assert!(
        close(portable.visible.unwrap().xyz, LEAK, 1e-4),
        "total includes leakage"
    );
    assert_eq!(portable.uv.map(|uv| uv.amount), Some(1.));
    let resolved = rig.resolve(&uv_only, None).unwrap();
    assert_eq!(resolved.raw(uv_channel(&destination)), 255);
    let visible_drive: u32 = path_channels(&destination)[..5]
        .iter()
        .map(|c| resolved.raw(c.id))
        .sum();
    assert!(
        visible_drive <= 2,
        "no second leakage from visible emitters"
    );
    let achieved = resolved.result.achieved.visible.unwrap();
    assert!(
        close(achieved, LEAK, 0.002),
        "{achieved:?} is not 2 × leakage"
    );
}

/// AC5: a Direct Dynamic sends a new recipe object every tick. Each changed recipe is
/// forward-evaluated by the original model before fallback fitting, even when the carried
/// estimate is stale; an unchanged object is evaluated once. Without the original, the recorded
/// estimate is used with a limitation.
#[test]
fn changing_direct_recipes_are_forward_evaluated_every_tick_before_fallback_fitting() {
    let source = rgbw();
    let destination = rgbal();
    let rig = DirectRig::new(&destination, &[&source, &destination]);
    let recorded = direct(&rig.catalogue.borrow(), &source, &[65535, 0, 0, 0]);
    let ColorProgram::Direct {
        recipe,
        portable: stale,
    } = direct_program(&recorded).as_ref()
    else {
        unreachable!()
    };
    let mut previous_y = None;
    for (tick, red) in [65535u32, 49152, 32768, 16384].into_iter().enumerate() {
        let mut changed = recipe.clone();
        changed.channels.iter_mut().for_each(|value| {
            if value.channel_id == path_channels(&source)[0].id {
                value.raw = red;
            }
        });
        // The Dynamic value keeps the stale record-time estimate on purpose.
        let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: changed.clone(),
            portable: stale.clone(),
        }));
        let resolved = rig.resolve(&value, None).unwrap();
        let expected = rig
            .catalogue
            .borrow()
            .resolve(&changed.source)
            .unwrap()
            .predict(&changed)
            .unwrap();
        assert_eq!(resolved.direct().estimate, expected, "tick {tick}");
        assert_eq!(resolved.direct().origin, DirectEstimateOrigin::Forward);
        let y = resolved.result.achieved.visible.unwrap().y;
        if let Some(previous) = previous_y {
            assert!(
                y < previous,
                "tick {tick}: the fitted output follows the recipe"
            );
        }
        previous_y = Some(y);
        assert_eq!(
            rig.adapter.counters().direct_forward_evaluations,
            tick as u64 + 1
        );
        rig.resolve(&value, None).unwrap();
        assert_eq!(
            rig.adapter.counters().direct_forward_evaluations,
            tick as u64 + 1,
            "an unchanged value object is evaluated once"
        );
    }
    // Original unavailable: the recorded estimate is valid fallback data, reported passively.
    rig.install(&destination, &[&destination]);
    let resolved = rig.resolve(&recorded, None).unwrap();
    assert_eq!(resolved.direct().origin, DirectEstimateOrigin::Recorded);
    assert_eq!(&resolved.direct().estimate, stale);
    assert!(
        resolved
            .direct()
            .limitations
            .iter()
            .any(|l| l.contains("recorded estimate is used"))
    );
}

/// Invalid data is not a fallback: an available original rejecting the recipe fails the frame;
/// an unresolved native spread is a passive materialization requirement.
#[test]
fn rejected_recipes_fail_and_unresolved_spreads_stay_passive() {
    let source = wheel_only();
    let rig = DirectRig::new(&source, &[&source]);
    let value = direct(&rig.catalogue.borrow(), &source, &[20]);
    let ColorProgram::Direct { recipe, portable } = direct_program(&value).as_ref() else {
        unreachable!()
    };
    let mut foreign = recipe.clone();
    foreign.channels[0].function_id = Uuid::new_v4();
    let invalid = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: foreign,
        portable: portable.clone(),
    }));
    assert!(matches!(
        rig.resolve(&invalid, None),
        Err(TransitionError::Invalid(_))
    ));
    let channel = &path_channels(&source)[0];
    let spread = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            spreads: vec![NativeColorSpread {
                binding: NativeColorBinding {
                    channel_id: channel.id,
                    function_id: channel.functions[0].id,
                },
                points: vec![0, 40],
            }],
            ..recipe.clone()
        },
        portable: portable.clone(),
    }));
    assert!(matches!(
        rig.resolve(&spread, None),
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints
        ))
    ));
}
