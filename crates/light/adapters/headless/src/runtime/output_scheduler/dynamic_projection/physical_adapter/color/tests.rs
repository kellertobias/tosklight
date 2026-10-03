//! TL-592 reference cases: the Color adapter resolves against a real captured frame (engine
//! capture, scalar static token, final geometry, token-bound native raw values), its writes are
//! encoded into DMX, decoded and re-simulated through the same compiled forward model.
use super::profiles::*;
use super::*;
use light_core::programming::{
    ColorAllocation, UvIntent, VirtualColorAuthoringV1, VirtualColorRecipe, WhiteTarget,
};
use light_core::{AttributeKey, ManualClock, SessionId};
use light_dynamics::DynamicRuntime;
use light_engine::{Engine, RenderOptions};
use light_fixture::FixtureProfile;
use light_programmer::ProgrammerRegistry;

mod actual_quality;
mod adoption;
mod derived;
mod destinations;
pub(in crate::runtime) mod direct;
mod fitting_bench;
mod lifecycle;
mod reference;
mod result_memo;
mod review;

pub(in crate::runtime) fn intent(rgb: [f32; 3], amber: f32) -> ColorIntent {
    let recipe = VirtualColorRecipe {
        version: 1,
        rgb,
        amber,
        approximate: false,
    };
    ColorIntent {
        base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe).unwrap(),
        recipe,
        ..ColorIntent::default()
    }
}

/// The TL-557 persistence magenta: White Blend 0.25, 5600 K, Duv −0.004, relativeOutput 0.8.
pub(in crate::runtime) fn magenta() -> ColorIntent {
    ColorIntent {
        white_blend: 0.25,
        white_target: WhiteTarget {
            kelvin: 5600.0,
            duv: -0.004,
        },
        relative_output: 0.8,
        ..intent([1.0, 0.0, 1.0], 0.0)
    }
}

/// The TL-557 3200 K warm white (White Blend 0.85, Duv 0.0035, relativeOutput 0.6).
pub(in crate::runtime) fn warm_white() -> ColorIntent {
    ColorIntent {
        white_blend: 0.85,
        white_target: WhiteTarget {
            kelvin: 3200.0,
            duv: 0.0035,
        },
        relative_output: 0.6,
        allocation: ColorAllocation::PreferWhite,
        ..intent([1.0, 0.72, 0.42], 0.3)
    }
}

pub(in crate::runtime) fn uv_only_black() -> ColorIntent {
    ColorIntent {
        relative_output: 0.0,
        uv: UvIntent { amount: 0.9 },
        ..intent([0.0, 0.0, 0.0], 0.0)
    }
}

pub(in crate::runtime) fn program(intent: &ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: intent.clone(),
    }))
}

pub(super) struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    adapter: ColorAdapter,
    clock: Arc<ManualClock>,
    profile: RefCell<FixtureProfile>,
}

/// One resolved head plus the evidence needed to re-simulate it.
pub(super) struct Resolved {
    descriptor: ColorDescriptor,
    native: Vec<u32>,
    token: light_engine::CapturedFrameToken,
    pub(super) result: PhysicalResolution<ColorAdapter>,
}

impl Rig {
    pub(super) fn new(profile: &FixtureProfile) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let target = FixtureId::new();
        let engine = Engine::new(programmers.clone());
        let rig = Self {
            engine,
            programmers,
            session,
            target,
            adapter: ColorAdapter::default(),
            clock,
            profile: RefCell::new(profile.clone()),
        };
        rig.install(profile);
        rig
    }

    fn install(&self, profile: &FixtureProfile) {
        *self.profile.borrow_mut() = profile.clone();
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(profile, self.target, 1)].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
    }

    fn set(&self, attribute: &str, value: f32) {
        self.programmers.set(
            self.session,
            self.target,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        );
    }

    fn resolve_with(
        &self,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
        options: RenderOptions,
    ) -> Result<Resolved, TransitionError> {
        self.resolve_on(None, intent, previous, options)
    }

    /// Resolve through `descriptor` (a retained descriptor keeps its per-head scratch across
    /// frames, as the lane's does) or through a freshly compiled one.
    fn resolve_on(
        &self,
        retained: Option<&ColorDescriptor>,
        intent: &ColorIntent,
        previous: Option<&ColorContinuity>,
        options: RenderOptions,
    ) -> Result<Resolved, TransitionError> {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(options);
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let frame = HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: models.as_ref(),
            token: &token,
            scalar: &scalar,
        };
        let descriptor = self
            .adapter
            .compile(&capture.snapshot(), self.target)?
            .expect("physical Color destination");
        let native = scalar
            .native_raw(&capture, &token, self.target)
            .unwrap()
            .raw()
            .to_vec();
        let value = program(intent);
        let result = self.adapter.resolve(PhysicalRequest {
            frame,
            target: self.target,
            owner: ProgrammingOwner::Color,
            descriptor: retained.unwrap_or(&descriptor),
            value: &value,
            previous,
        })?;
        Ok(Resolved {
            descriptor,
            native,
            token,
            result,
        })
    }

    pub(super) fn resolve(&self, intent: &ColorIntent) -> Resolved {
        let resolved = self
            .resolve_with(intent, None, RenderOptions::default())
            .unwrap();
        resolved.verify(intent, &self.profile.borrow());
        resolved
    }
}

/// Decode one fixture's DMX bytes with the builder's sequential layout (fine bytes follow).
fn decode(mode: &light_fixture::FixtureMode, bytes: &[u8; 512]) -> Vec<u32> {
    let mut slot = 1usize;
    mode.channels
        .iter()
        .map(|channel| {
            let mut raw = u32::from(bytes[slot - 1]);
            for secondary in &channel.secondary_slots {
                raw = (raw << 8) | u32::from(bytes[usize::from(*secondary) - 1]);
            }
            slot += channel.resolution.bytes();
            raw
        })
        .collect()
}

impl Resolved {
    pub(super) fn raw(&self, channel: u32) -> u32 {
        self.result
            .writes
            .iter()
            .find(|w| w.slot.channel_index == channel)
            .expect("owned Color control")
            .raw
    }

    pub(super) fn raws(&self) -> Vec<u32> {
        self.result.writes.iter().map(|w| w.raw).collect()
    }

    /// Complete sidecar checks, then encode the full native output into DMX, decode the bytes
    /// and re-run the forward simulation: it must reproduce the published achieved output.
    fn verify(&self, intent: &ColorIntent, profile: &FixtureProfile) {
        let (result, descriptor) = (&self.result, &self.descriptor);
        validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
        assert_eq!(&result.requested, intent, "the request is never rewritten");
        assert!(
            result.writes.iter().all(|w| w.slot.channel_index != 0),
            "Intensity is never a Color write"
        );
        let head = descriptor.primary();
        assert!(
            result
                .writes
                .iter()
                .all(|w| w.slot.destination == head.destination)
        );
        let mut output = self.native.clone();
        for write in &result.writes {
            output[write.slot.channel_index as usize] = write.raw;
        }
        let mode = &profile.modes[0];
        let plan = mode.compile_encoding_plan().unwrap();
        let mut bytes = [0u8; 512];
        let values: Vec<_> = (0u32..).zip(output.iter().copied()).collect();
        plan.encode_split_by_index(&mut bytes, 1, 1, &values)
            .unwrap();
        let decoded = decode(mode, &bytes);
        assert_eq!(
            decoded, output,
            "encoded bytes decode to the written values"
        );
        let model = head.fitting.forward();
        let mut forward = model.create_output();
        model.evaluate(&decoded, &mut forward).unwrap();
        let forward = &forward[head.head];
        assert_eq!(result.achieved.known_xyz, forward.known_xyz);
        assert_eq!(
            result.achieved.visible,
            forward.visible_complete.then_some(forward.known_xyz)
        );
        let uv = forward.portable_uv.map(|uv| uv.amount);
        let applied = result.quality.uv == UvFitStatus::Applied;
        assert_eq!(result.achieved.uv_drive, uv.filter(|_| applied));
    }
}

fn uv_raw(amount: f32) -> u32 {
    (f64::from(amount) * 255.).round() as u32
}

#[test]
fn rgb_and_rgbw_fit_magenta_warm_white_and_white_blend_through_encoded_forward_output() {
    let rig = Rig::new(&rgb());
    let plain = rig.resolve(&intent([1., 0., 1.], 0.));
    // Channel 1 is the U16 red of the TL-568 reference layout.
    assert_eq!(plain.raws(), [65535, 0, 255]);
    assert_eq!(plain.result.quality.color_match, ColorMatch::Exact);
    assert_eq!(plain.result.quality.visible, VisibleFitStatus::Fitted);
    assert!(!plain.result.quality.nominal);
    for request in [magenta(), warm_white()] {
        let resolved = rig.resolve(&request);
        assert_eq!(resolved.result.quality.visible, VisibleFitStatus::Fitted);
        assert!(resolved.result.achieved.visible.is_some());
    }
    assert_eq!(rig.adapter.counters().fitting_compiles, 1);
    assert_eq!(rig.adapter.counters().fitting_cache_hits, 2);

    rig.install(&rgbw());
    for (blend, expected) in [
        (0.0, [65535, 255, 255, 0]),
        (0.5, [65535, 255, 255, 255]),
        (1.0, [0, 0, 0, 255]),
    ] {
        let mut request = intent([1., 1., 1.], 0.);
        request.white_blend = blend;
        let resolved = rig.resolve(&request);
        assert_eq!(resolved.raws(), expected, "White Blend {blend}");
        assert_eq!(resolved.result.quality.color_match, ColorMatch::Exact);
        assert!(
            !resolved.result.quality.luminance_limited,
            "White Blend {blend}"
        );
    }
    assert_eq!(
        rig.adapter.counters().fitting_compiles,
        2,
        "a replaced fixture list recompiles the destination fitter"
    );
}

#[test]
fn rgbwauv_black_dim_uv_only_and_uv0_keep_uv_frozen_and_independent() {
    let rig = Rig::new(&rgbwauv(None));
    let purple = intent([1., 0., 1.], 0.);
    let off = rig.resolve(&purple);
    let uv = off.result.writes.last().unwrap();
    assert_eq!((uv.slot.channel_index, uv.raw, uv.parked), (6, 0, true));
    assert_eq!(off.result.quality.uv, UvFitStatus::Applied);
    let visible_off = off.raws()[..5].to_vec();

    let mut with_uv = purple.clone();
    with_uv.uv = UvIntent { amount: 0.45 };
    let on = rig.resolve(&with_uv);
    assert_eq!(on.raw(6), uv_raw(0.45));
    assert!(!on.result.writes[5].parked);
    assert_eq!(
        on.raws()[..5],
        visible_off,
        "UV is never borrowed for purple"
    );
    assert_eq!(
        on.result.quality.visible,
        VisibleFitStatus::PredictionIncomplete
    );
    assert_eq!(
        on.result.quality.total_quality,
        PhysicalDataQuality::Unknown
    );
    assert!(!on.result.quality.uv_appearance_known);
    assert!(on.result.achieved.visible.is_none(), "unknown is not black");

    let mut dim = with_uv.clone();
    dim.relative_output = 0.25;
    let dim = rig.resolve(&dim);
    assert_eq!(dim.raw(6), uv_raw(0.45), "relativeOutput never scales UV");
    assert!(dim.raw(1) < on.raw(1) && dim.raw(3) < on.raw(3));

    let mut black = purple.clone();
    black.relative_output = 0.;
    let black = rig.resolve(&black);
    assert_eq!(black.raws(), [0; 6]);
    assert_eq!(black.result.quality.color_match, ColorMatch::Exact);

    let uv_only = rig.resolve(&uv_only_black());
    assert_eq!(uv_only.raws(), [0, 0, 0, 0, 0, uv_raw(0.9)]);
    assert_eq!(uv_only.result.quality.uv, UvFitStatus::Applied);

    // A head without UV retains the request and reports unsupported UV, never violet.
    let rig = Rig::new(&rgb());
    let unsupported = rig.resolve(&uv_only_black());
    assert_eq!(unsupported.raws(), [0, 0, 0]);
    assert_eq!(unsupported.result.quality.uv, UvFitStatus::Unsupported);
    assert!(
        unsupported
            .result
            .quality
            .limitations
            .contains(ColorFitLimitations::UV_UNSUPPORTED)
    );
    assert_eq!(
        unsupported.result.requested.semantic().unwrap().uv.amount,
        0.9
    );
}

#[test]
fn rgbal_and_rgbwauv_fit_warm_white_through_the_fitter() {
    for profile in [rgbal(), rgbwauv(None)] {
        let rig = Rig::new(&profile);
        let mut warm = intent([1., 1., 1.], 0.);
        warm.white_blend = 1.;
        warm.white_target = WhiteTarget {
            kelvin: 3200.,
            duv: 0.005,
        };
        let exact = rig.resolve(&warm);
        assert_eq!(
            exact.result.quality.color_match,
            ColorMatch::Exact,
            "{}",
            profile.name
        );
        assert!(exact.result.quality.delta_uv.unwrap() < 0.002);
        let tl557 = rig.resolve(&warm_white());
        assert_eq!(tl557.result.quality.visible, VisibleFitStatus::Fitted);
        // Deterministic: identical inputs give identical writes and reports.
        let again = rig.resolve(&warm_white());
        assert_eq!(again.raws(), tl557.raws());
        assert_eq!(again.result.quality, tl557.result.quality);
    }
}

#[test]
fn fixed_uv_leakage_cost_and_candidate_counts_are_measured() {
    let rig = Rig::new(&rgbwauv(Some(xyz(0.02, 0.005, 0.1))));
    let mut request = intent([1., 0., 1.], 0.);
    let zero = rig.resolve(&request).result.quality;
    request.uv.amount = 1.;
    let fixed = rig.resolve(&request).result.quality;
    assert_eq!(fixed.visible, VisibleFitStatus::Fitted);
    assert!(fixed.uv_appearance_known);
    assert_eq!(fixed.color_match, ColorMatch::Exact);
    for (name, quality) in [("UV0", &zero), ("fixed UV", &fixed)] {
        let work = quality.work;
        eprintln!(
            "TL-592 work {name}: fits={} candidates={} visible_solves={} fixed_offset={} \
             level_solves={} forward_evaluations={}",
            work.fits,
            work.candidates_ranked,
            work.fit.visible_solves,
            work.fit.fixed_offset_solves,
            work.fit.level_solves,
            work.fit.forward_evaluations
        );
        assert_eq!(work.fits, 1);
        assert_eq!(
            work.candidates_ranked, 1,
            "{name}: continuous emitters only"
        );
        assert_eq!(work.fit.visible_solves, 1);
    }
    // Without an offset one level solve; frozen leakage runs 1 + 16 grid + 2 + 24 refinements.
    assert_eq!(
        (
            zero.work.fit.fixed_offset_solves,
            zero.work.fit.level_solves
        ),
        (0, 1)
    );
    assert_eq!(
        (
            fixed.work.fit.fixed_offset_solves,
            fixed.work.fit.level_solves
        ),
        (1, 43)
    );
    let counters = rig.adapter.counters();
    assert_eq!(
        (counters.resolves, counters.fits, counters.refits),
        (2, 2, 0)
    );
    assert_eq!(counters.level_solves, 44);
}

#[test]
fn wheel_only_dual_wheel_and_cmy_wheel_select_steady_filters_and_keep_the_accepted_slot() {
    let rig = Rig::new(&wheel_only());
    let blue = intent([0., 0., 1.], 0.);
    let first = rig.resolve(&blue);
    assert_eq!(first.raws(), [39], "center of the blue slot");
    assert_eq!(first.result.quality.visible, VisibleFitStatus::Fitted);
    let channel = first.descriptor.primary().controls[0].channel_id;
    // The last accepted raw inside the chosen slot is kept: no wheel movement.
    let head_id = first.descriptor.primary().head_id;
    let continuity = |destination, head_id, raw| ColorContinuity {
        heads: vec![ColorHeadContinuity {
            destination,
            head_id,
            controls: vec![(1, channel, raw)],
        }],
    };
    let previous = continuity(rig.target, head_id, 33);
    let kept = rig
        .resolve_with(&blue, Some(&previous), RenderOptions::default())
        .unwrap();
    kept.verify(&blue, &rig.profile.borrow());
    assert_eq!(kept.raws(), [33]);
    // Continuity of another destination or head, or out of range, is ignored.
    for foreign in [
        continuity(FixtureId::new(), head_id, 33),
        continuity(rig.target, Uuid::new_v4(), 33),
        continuity(rig.target, head_id, 256),
    ] {
        let resolved = rig
            .resolve_with(&blue, Some(&foreign), RenderOptions::default())
            .unwrap();
        assert_eq!(resolved.raws(), [39]);
    }

    let rig = Rig::new(&dual_wheel());
    let red = intent([1., 0., 0.], 0.);
    let resolved = rig.resolve(&red);
    assert_eq!(resolved.result.writes.len(), 2);
    assert!(
        (16..=31).contains(&resolved.raw(1)),
        "{:?}",
        resolved.raws()
    );
    assert_eq!(resolved.result.quality.work.candidates_ranked, 9);

    let rig = Rig::new(&cmy_wheel());
    let resolved = rig.resolve(&red);
    assert_eq!(resolved.result.writes.len(), 4);
    assert_eq!(resolved.result.quality.visible, VisibleFitStatus::Fitted);
    assert_eq!(resolved.result.quality.work.candidates_ranked, 81);
    assert_eq!(resolved.result.quality.work.fit.visible_solves, 0);
    let mut black = intent([1., 1., 1.], 0.);
    black.relative_output = 0.;
    let black = rig.resolve(&black);
    assert!(
        black
            .result
            .quality
            .limitations
            .contains(ColorFitLimitations::BLACK_UNATTAINABLE)
    );
}

#[test]
fn retained_controls_are_parked_before_the_final_fit() {
    let mut profile = wheel_only();
    profile.modes[0].color_physical.as_mut().unwrap().paths[0].filters[0].transmission =
        light_fixture::OpticalTransmission::Unknown;
    profile.validate().unwrap();
    let rig = Rig::new(&profile);
    rig.set("color.wheel.1", 0.1);
    let request = intent([1., 0., 0.], 0.);
    let resolved = rig.resolve(&request);
    assert_ne!(
        resolved.native[1], 0,
        "a stale scalar wheel value is present"
    );
    let write = resolved.result.writes[0];
    assert_eq!((write.raw, write.parked), (0, true));
    let quality = &resolved.result.quality;
    assert_eq!(quality.visible, VisibleFitStatus::UnknownAppearance);
    assert_eq!(
        quality.parked,
        [(write.channel_id, ColorRetainReason::UnknownAppearance)]
    );
    assert_eq!(quality.work.fits, 2, "refit after parking");
    assert!(resolved.result.achieved.visible.is_none());
    assert_eq!(rig.adapter.counters().refits, 1);
}

#[test]
fn intensity_masters_and_blackout_are_left_to_the_single_final_render() {
    let rig = Rig::new(&rgbw());
    let base = rig.resolve(&magenta());
    rig.set("intensity", 0.3);
    for options in [
        RenderOptions::default(),
        RenderOptions {
            grand_master: 0.25,
            ..Default::default()
        },
        RenderOptions {
            blackout: true,
            ..Default::default()
        },
    ] {
        let resolved = rig.resolve_with(&magenta(), None, options).unwrap();
        resolved.verify(&magenta(), &rig.profile.borrow());
        assert!(resolved.native[0] > 0, "{options:?}: pre-master Intensity");
        assert_eq!(resolved.raws(), base.raws(), "{options:?}");
        assert_eq!(resolved.result.achieved, base.result.achieved);
    }
}

#[test]
fn rgb_to_rgbw_replacement_with_white_seeded_decides_every_new_control() {
    let rig = Rig::new(&rgb());
    let request = intent([1., 0., 1.], 0.);
    let first = rig.resolve(&request);
    assert_eq!(first.result.writes.len(), 3);
    let replacement = rgbw();
    rig.install(&replacement);
    rig.set("color.white", 0.8);
    let second = rig
        .resolve_with(
            &request,
            Some(&first.result.continuity),
            RenderOptions::default(),
        )
        .unwrap();
    second.verify(&request, &replacement);
    assert_eq!(
        second.native[4], 204,
        "White is seeded nonzero on the new destination"
    );
    assert_eq!(
        second.result.writes.len(),
        4,
        "every new Color control is written"
    );
    assert_eq!(second.raws(), [65535, 0, 255, 0]);
    assert_eq!(second.result.requested, request);
    assert_eq!(second.result.continuity.heads[0].controls.len(), 4);
    for tl557 in [magenta(), warm_white()] {
        let resolved = rig.resolve(&tl557);
        assert_ne!(
            resolved.raw(4),
            204,
            "White is fitted, never left at the seed"
        );
        assert_eq!(resolved.result.writes.len(), 4);
    }
}
