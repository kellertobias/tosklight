//! TL-624 captured Color and fixed-UV fitting work.
//!
//! Drives the actual Color path for one patched fixture per capture: engine capture, the scalar
//! static-family frame, the captured geometry, then `ColorAdapter::compile`/`resolve` against
//! the token-bound native raw values, complete owned writes, and an encode → decode → forward
//! re-simulation of the written output. It reuses `tests_direct::DirectRig` (the descriptor is
//! compiled once per installed generation and retained) and `Resolved::assert_forward`.
//!
//! Cold samples reinstall the fixture (a new fixture list and generation) and resolve once
//! through `DirectRig::resolve`, which compiles. Warm samples resolve the *retained* descriptor
//! of that generation again and again, so fitting is never confused with recompilation.
//!
//! The deterministic smoke runs in normal test runs and asserts functional behavior and exact
//! counters only. The manual benchmark is `#[ignore]`, release-mode, writes an immutable report
//! and asserts no timing. This is a synthetic, sequential one-fixture-per-capture fit workload:
//! it is not a simultaneous output frame, a render deadline, a physical colour match, native
//! Stage evidence or TL-596 acceptance. The production programming contract is not touched.
use super::super::profiles::*;
use super::super::tests::{intent, magenta, program, warm_white};
use super::super::*;
use super::direct::{DirectRig, Resolved};
use light_core::NativeColorValue;
use light_core::programming::{ColorWheelConstraint, UvIntent};
use light_engine::{CapturedFrameToken, RenderOptions};
use light_fixture::FixtureProfile;
use light_fixture::forward::ColorConstraintStatus;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Fixed seed of the per-sample request order (xorshift64*). Recorded in every report.
const SEED: u64 = 0x7162_4000_0000_0001;
/// Known visible leakage of the UV emitter (the TL-592 fixed-UV reference value).
const UV_LEAK: Xyz = Xyz {
    x: 0.02,
    y: 0.005,
    z: 0.1,
};

// ---------------------------------------------------------------------------------------------
// Workload: fixture capabilities and request scenarios.

struct BenchFixture {
    name: &'static str,
    capability: &'static str,
    uv_model: &'static str,
    builder: &'static str,
    profile: FixtureProfile,
}

fn bench_fixtures() -> Vec<BenchFixture> {
    vec![
        BenchFixture {
            name: "additive-rgb",
            capability: "additive",
            uv_model: "no UV emitter",
            builder: "profiles::rgb",
            profile: rgb(),
        },
        BenchFixture {
            name: "additive-rgbwauv-known-leakage",
            capability: "additive with UV",
            uv_model: "UV emitter with known visible leakage",
            builder: "profiles::rgbwauv(Some(0.02, 0.005, 0.1))",
            profile: rgbwauv(Some(UV_LEAK)),
        },
        BenchFixture {
            name: "additive-rgbwauv-unknown-leakage",
            capability: "additive with UV",
            uv_model: "UV emitter with unknown visible leakage",
            builder: "profiles::rgbwauv(None)",
            profile: rgbwauv(None),
        },
        BenchFixture {
            name: "subtractive-cmy-wheel",
            capability: "subtractive CMY flags plus colour wheel",
            uv_model: "no UV emitter",
            builder: "profiles::cmy_wheel",
            profile: cmy_wheel(),
        },
        BenchFixture {
            name: "hybrid-rgbw-wheel",
            capability: "hybrid: XYZ-only additive RGBW behind a spectral colour wheel",
            uv_model: "no UV emitter",
            builder: "profiles::hybrid",
            profile: hybrid(),
        },
        BenchFixture {
            name: "hybrid-spectral-rgbw-wheel",
            capability: "hybrid: spectral additive RGBW behind a spectral colour wheel",
            uv_model: "no UV emitter",
            builder: "tests_fitting_bench::spectral_hybrid",
            profile: spectral_hybrid(),
        },
        BenchFixture {
            name: "wheel-only",
            capability: "fixed source through one wheel with an unmodeled rotation range",
            uv_model: "no UV emitter",
            builder: "profiles::wheel_only",
            profile: wheel_only(),
        },
    ]
}

/// The TL-557 hybrid topology with spectral emitters, so a filtered emitter has a known
/// appearance (`profiles::hybrid` carries XYZ-only emitters, whose filtered appearance is
/// unknown). Built here with the shared `Builder`; no profile helper is changed.
fn spectral_hybrid() -> FixtureProfile {
    let band = |from: u32, to: u32, level: f32| {
        spectrum(move |nm| if (from..=to).contains(&nm) { level } else { 0. })
    };
    let mut builder = Builder::new("TL-624 spectral hybrid RGBW wheel");
    for (attribute, samples) in [
        ("color.red", band(600, 700, 0.02)),
        ("color.green", band(500, 580, 0.02)),
        ("color.blue", band(430, 490, 0.02)),
        ("color.white", spectrum(|_| (1.0 / 106.856_915) as f32)),
    ] {
        builder.emitter_with(attribute, light_fixture::ChannelResolution::U8, |e| {
            e.spectrum = samples;
        });
    }
    builder
        .filter("color.wheel.1", 127, wheel_slots(), false)
        .build()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sweep {
    Unchanged,
    WhiteBlend,
    Cct,
    Duv,
    Uv,
    Hue,
    WheelConstraint,
}

struct Scenario {
    name: &'static str,
    description: &'static str,
    sweep: Sweep,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "warm-unchanged",
            description: "the TL-557 warm white, one unchanged value object every frame",
            sweep: Sweep::Unchanged,
        },
        Scenario {
            name: "white-blend-sweep",
            description: "warm white with White Blend changing every frame",
            sweep: Sweep::WhiteBlend,
        },
        Scenario {
            name: "cct-sweep",
            description: "warm white with the White Target CCT changing every frame",
            sweep: Sweep::Cct,
        },
        Scenario {
            name: "duv-sweep",
            description: "warm white with the White Target Duv changing every frame",
            sweep: Sweep::Duv,
        },
        Scenario {
            name: "uv-sweep",
            description: "TL-557 magenta with the UV amount changing every frame (includes 0)",
            sweep: Sweep::Uv,
        },
        Scenario {
            name: "hue-candidate-sweep",
            description: "saturated and white recipes changing every frame: wheel/flag candidates re-rank",
            sweep: Sweep::Hue,
        },
        Scenario {
            name: "wheel-constraint-sweep",
            description: "blue with no, own red-slot, own blue-slot or foreign wheel constraint, changing every frame",
            sweep: Sweep::WheelConstraint,
        },
    ]
}

const WHITE_BLENDS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const KELVINS: [f32; 5] = [2700.0, 3200.0, 4300.0, 5600.0, 6500.0];
const DUVS: [f32; 5] = [-0.006, -0.003, 0.0, 0.003, 0.006];
const UV_AMOUNTS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const HUES: [(&str, [f32; 3]); 7] = [
    ("red", [1., 0., 0.]),
    ("green", [0., 1., 0.]),
    ("blue", [0., 0., 1.]),
    ("magenta", [1., 0., 1.]),
    ("cyan", [0., 1., 1.]),
    ("yellow", [1., 1., 0.]),
    ("white", [1., 1., 1.]),
];

/// One request of a scenario: its label, the immutable composed intent and value object.
struct Request {
    label: String,
    intent: ColorIntent,
    value: AttributeValue,
    uv: f32,
}

fn request(label: String, intent: ColorIntent) -> Request {
    Request {
        label,
        uv: intent.uv.amount,
        value: program(&intent),
        intent,
    }
}

/// The scenario's requests for this fixture, or why it does not apply.
fn requests(sweep: Sweep, fixture: &BenchFixture) -> Result<Vec<Request>, &'static str> {
    let with = |edit: &dyn Fn(&mut ColorIntent)| {
        let mut value = warm_white();
        edit(&mut value);
        value
    };
    Ok(match sweep {
        Sweep::Unchanged => vec![request("warm-white".into(), warm_white())],
        Sweep::WhiteBlend => WHITE_BLENDS
            .iter()
            .map(|b| {
                request(
                    format!("whiteBlend={b}"),
                    with(&|i: &mut ColorIntent| i.white_blend = *b),
                )
            })
            .collect(),
        Sweep::Cct => KELVINS
            .iter()
            .map(|k| {
                request(
                    format!("kelvin={k}"),
                    with(&|i: &mut ColorIntent| i.white_target.kelvin = *k),
                )
            })
            .collect(),
        Sweep::Duv => DUVS
            .iter()
            .map(|d| {
                request(
                    format!("duv={d}"),
                    with(&|i: &mut ColorIntent| i.white_target.duv = *d),
                )
            })
            .collect(),
        Sweep::Uv => UV_AMOUNTS
            .iter()
            .map(|amount| {
                let mut value = magenta();
                value.uv = UvIntent { amount: *amount };
                request(format!("uv={amount}"), value)
            })
            .collect(),
        Sweep::Hue => HUES
            .iter()
            .map(|(name, rgb)| request((*name).into(), intent(*rgb, 0.)))
            .collect(),
        Sweep::WheelConstraint => {
            let mode = &fixture.profile.modes[0];
            let Some(channel) = mode
                .channels
                .iter()
                .find(|channel| &*channel.attribute.0 == "color.wheel.1")
            else {
                return Err("no colour wheel: wheel constraints do not apply");
            };
            let own = fixture
                .profile
                .native_color_identity(mode.id, mode.heads[0].id)
                .expect("native Color identity");
            let other = wheel_only();
            let foreign = other
                .native_color_identity(other.modes[0].id, other.modes[0].heads[0].id)
                .expect("native Color identity");
            let pin = |source, raw| ColorWheelConstraint {
                source,
                value: NativeColorValue {
                    channel_id: channel.id,
                    function_id: channel.functions[0].id,
                    raw,
                },
            };
            let blue = || intent([0., 0., 1.], 0.);
            let constrained = |label: &str, constraint| {
                let mut value = blue();
                value.wheel_constraints = vec![constraint];
                request(label.into(), value)
            };
            vec![
                request("none".into(), blue()),
                constrained("own-red-slot", pin(own.clone(), 20)),
                constrained("own-blue-slot", pin(own, 40)),
                constrained("foreign-red-slot", pin(foreign, 20)),
            ]
        }
    })
}

/// Deterministic request order: xorshift64* from `SEED`, never the same request twice in a
/// row when the scenario has more than one, so every frame of a sweep is a changed request.
struct Order {
    state: u64,
    previous: Option<usize>,
}
impl Order {
    fn new(scenario: usize, fixture: usize) -> Self {
        Self {
            state: SEED ^ (((scenario as u64) << 32) | fixture as u64).wrapping_mul(0x9e37_79b9),
            previous: None,
        }
    }
    fn next(&mut self, count: usize) -> usize {
        if count == 1 {
            return 0;
        }
        loop {
            self.state ^= self.state >> 12;
            self.state ^= self.state << 25;
            self.state ^= self.state >> 27;
            let index = (self.state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % count;
            if Some(index) != self.previous {
                self.previous = Some(index);
                return index;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Observation.

macro_rules! counter_delta {
    ($after:expr, $before:expr, $($field:ident),* $(,)?) => {
        ColorAdapterCounters { $($field: $after.$field - $before.$field),* }
    };
}

fn delta(after: ColorAdapterCounters, before: ColorAdapterCounters) -> ColorAdapterCounters {
    counter_delta!(
        after,
        before,
        descriptor_compiles,
        fitting_compiles,
        fitting_cache_hits,
        fitting_failures,
        multi_head_targets,
        copy_destinations,
        resolves,
        fits,
        refits,
        result_reuses,
        shared_conflicts,
        direct_exact,
        direct_fallbacks,
        direct_visible_holds,
        direct_forward_evaluations,
        representation_transitions,
        representation_holds,
        representation_adoptions,
        candidates_ranked,
        visible_solves,
        fixed_offset_solves,
        level_solves,
        forward_evaluations,
        fitting_shared,
    )
}

#[derive(Clone, Copy, Debug, Default)]
struct Phases {
    capture: Duration,
    scalar: Duration,
    geometry: Duration,
    resolve: Duration,
    validate: Duration,
}
impl Phases {
    fn total(&self) -> Duration {
        self.capture + self.scalar + self.geometry + self.resolve + self.validate
    }
}

struct ColdObservation {
    install: Duration,
    /// `DirectRig::resolve` of a new generation: capture, scalar, geometry, compile, resolve.
    first_resolve: Duration,
    /// `ColorAdapter::compile` of the same captured snapshot on a fresh adapter.
    isolated_compile: Duration,
    counters: ColorAdapterCounters,
    isolated_counters: ColorAdapterCounters,
}

#[derive(Clone)]
struct WarmObservation {
    phases: Phases,
    counters: ColorAdapterCounters,
    request: usize,
    class: &'static str,
    color_match: ColorMatch,
    visible: VisibleFitStatus,
    uv: UvFitStatus,
    constraint: Option<ColorConstraintStatus>,
    writes: Vec<u32>,
}

/// Separates unsupported UV and unknown leakage from numerical matching.
fn classify(quality: &ColorQuality) -> &'static str {
    match (quality.uv, quality.visible) {
        (UvFitStatus::Unsupported, _) => "uv-unsupported",
        (_, VisibleFitStatus::PredictionIncomplete) if !quality.uv_appearance_known => {
            "unknown-uv-leakage"
        }
        (_, VisibleFitStatus::PredictionIncomplete) => "prediction-incomplete",
        (_, VisibleFitStatus::UnknownAppearance) => "unknown-appearance",
        (_, VisibleFitStatus::CandidateLimit) => "candidate-limit",
        (_, VisibleFitStatus::Fitted) => "numerical-fit",
        _ => "other",
    }
}

/// Decode one fixture's DMX bytes with the builder's sequential layout (fine bytes follow).
/// A test-side copy of the private `tests::decode`; no helper visibility was widened.
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

/// Encode the complete native output (scalar baseline plus Color writes) into DMX, decode it
/// and re-run the head's compiled forward model: it must reproduce the published achieved
/// output. Mirrors `tests::Resolved::verify`, which is private to that module.
fn encoded_forward(resolved: &Resolved, profile: &FixtureProfile) -> Result<(), String> {
    let result = &resolved.result;
    let head = resolved.descriptor.primary();
    let mut output = resolved.native.clone();
    for write in &result.writes {
        output[write.slot.channel_index as usize] = write.raw;
    }
    let mode = &profile.modes[0];
    let plan = mode
        .compile_encoding_plan()
        .map_err(|error| format!("encoding plan: {error:?}"))?;
    let mut bytes = [0u8; 512];
    let values: Vec<_> = (0u32..).zip(output.iter().copied()).collect();
    plan.encode_split_by_index(&mut bytes, 1, 1, &values)
        .map_err(|error| format!("encode: {error:?}"))?;
    let decoded = decode(mode, &bytes);
    if decoded != output {
        return Err("encoded bytes do not decode to the written values".into());
    }
    let model = head.fitting.forward();
    let mut forward = model.create_output();
    model
        .evaluate(&decoded, &mut forward)
        .map_err(|error| format!("forward: {error:?}"))?;
    let forward = &forward[head.head];
    if result.achieved.known_xyz != forward.known_xyz {
        return Err("achieved known XYZ differs from the encoded forward output".into());
    }
    if result.achieved.visible != forward.visible_complete.then_some(forward.known_xyz) {
        return Err("achieved visible differs from the encoded forward output".into());
    }
    let applied = result.quality.uv == UvFitStatus::Applied;
    let uv = forward.portable_uv.map(|uv| uv.amount);
    if result.achieved.uv_drive != uv.filter(|_| applied) {
        return Err("achieved UV drive differs from the encoded forward output".into());
    }
    Ok(())
}

/// One captured frame through the retained descriptor, with phase timestamps. The same
/// sequence as `DirectRig::resolve` (whose `ManualClock` is private and is not advanced here;
/// static Color fitting reads no time).
fn phased_frame(
    rig: &DirectRig,
    descriptor: &Rc<ColorDescriptor>,
    value: &AttributeValue,
    previous: Option<&ColorContinuity>,
) -> Result<(Resolved, CapturedFrameToken, Phases), TransitionError> {
    let t0 = Instant::now();
    let capture = rig.engine.prepare_output_frame(RenderOptions::default());
    let token = capture.frame_token();
    let t1 = Instant::now();
    let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
    let t2 = Instant::now();
    let geometry = rig
        .engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .map_err(|error| invalid(format!("geometry: {error:?}")))?;
    let catalogue = Arc::clone(&rig.catalogue.borrow());
    let t3 = Instant::now();
    let result = rig.adapter.resolve(PhysicalRequest {
        frame: HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: catalogue.as_ref(),
            token: &token,
            scalar: &scalar,
        },
        target: rig.target,
        owner: ProgrammingOwner::Color,
        descriptor,
        value,
        previous,
    })?;
    let t4 = Instant::now();
    validate_complete_writes(&descriptor.footprint, &result.writes)?;
    let t5 = Instant::now();
    // Evidence only, outside the timed phases.
    let native = scalar
        .native_raw(&capture, &token, rig.target)
        .map_err(|error| invalid(format!("native raw: {error:?}")))?
        .raw()
        .to_vec();
    Ok((
        Resolved {
            capture,
            descriptor: Rc::clone(descriptor),
            native,
            result,
        },
        token,
        Phases {
            capture: t1 - t0,
            scalar: t2 - t1,
            geometry: t3 - t2,
            resolve: t4 - t3,
            validate: t5 - t4,
        },
    ))
}

/// Functional checks of one resolution. Failures are collected, never mixed with timing.
fn check(
    resolved: &Resolved,
    request: &Request,
    fixture: &BenchFixture,
    target: FixtureId,
    failures: &mut Failures,
) {
    let mut fail = |message: String| failures.push(message);
    let result = &resolved.result;
    if let Err(error) = validate_complete_writes(&resolved.descriptor.footprint, &result.writes) {
        fail(format!("incomplete writes: {error:?}"));
    }
    if result.requested != ColorRequest::Semantic(request.intent.clone()) {
        fail("the authored request was rewritten".into());
    }
    if request.value != program(&request.intent) {
        fail("the authored value object changed".into());
    }
    let head = resolved.descriptor.primary();
    let owned = head
        .controls
        .iter()
        .map(|control| control.channel_index)
        .collect::<Vec<_>>();
    if result
        .writes
        .iter()
        .any(|write| write.slot.destination != target || !owned.contains(&write.slot.channel_index))
    {
        fail("a write left the head's owned Color controls".into());
    }
    if result.writes.len() != owned.len() {
        fail(format!(
            "{} writes for {} owned controls",
            result.writes.len(),
            owned.len()
        ));
    }
    if result.continuity.heads.len() != 1 || result.quality.heads.len() != 1 {
        fail("one head expected in continuity and quality".into());
    }
    if let Err(error) = encoded_forward(resolved, &fixture.profile) {
        fail(format!("{}: {error}", fixture.name));
        return;
    }
    // The reused DirectRig check; consistent with the encoded check above, so it cannot panic
    // unless the two disagree.
    resolved.assert_forward();
}

/// Bounded functional failure list of one run.
#[derive(Default)]
struct Failures(Vec<String>);
impl Failures {
    fn push(&mut self, message: String) {
        if self.0.len() < 64 {
            self.0.push(message);
        }
    }
}

struct Run {
    fixture: &'static str,
    capability: &'static str,
    uv_model: &'static str,
    builder: &'static str,
    scenario: &'static str,
    description: &'static str,
    labels: Vec<String>,
    uv_amounts: Vec<f32>,
    skipped: Option<&'static str>,
    cold: Vec<ColdObservation>,
    warmup: usize,
    samples: Vec<WarmObservation>,
    failures: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
struct Parameters {
    cold: usize,
    warmup: usize,
    samples: usize,
}

fn run_one(
    fixture: &BenchFixture,
    fixture_index: usize,
    scenario: &Scenario,
    scenario_index: usize,
    parameters: Parameters,
) -> Run {
    let mut run = Run {
        fixture: fixture.name,
        capability: fixture.capability,
        uv_model: fixture.uv_model,
        builder: fixture.builder,
        scenario: scenario.name,
        description: scenario.description,
        labels: Vec::new(),
        uv_amounts: Vec::new(),
        skipped: None,
        cold: Vec::new(),
        warmup: parameters.warmup,
        samples: Vec::new(),
        failures: Vec::new(),
    };
    let requests = match requests(scenario.sweep, fixture) {
        Ok(requests) => requests,
        Err(reason) => {
            run.skipped = Some(reason);
            return run;
        }
    };
    run.labels = requests.iter().map(|r| r.label.clone()).collect();
    run.uv_amounts = requests.iter().map(|r| r.uv).collect();
    let mut failures = Failures::default();
    let mut order = Order::new(scenario_index, fixture_index);
    let profile = &fixture.profile;
    let rig = DirectRig::new(profile, &[profile]);
    // Cold: every sample is a new installed generation, resolved once through DirectRig.
    let mut last = None;
    for _ in 0..parameters.cold.max(1) {
        let first = &requests[order.next(requests.len())];
        let t0 = Instant::now();
        rig.install(profile, &[profile]);
        let t1 = Instant::now();
        let before = rig.adapter.counters();
        let t2 = Instant::now();
        let resolved = match rig.resolve(&first.value, None) {
            Ok(resolved) => resolved,
            Err(error) => {
                failures.push(format!("cold resolve rejected: {error:?}"));
                run.failures = failures.0;
                return run;
            }
        };
        let t3 = Instant::now();
        let counters = delta(rig.adapter.counters(), before);
        check(&resolved, first, fixture, rig.target, &mut failures);
        let isolated = ColorAdapter::default();
        let snapshot = resolved.capture.snapshot();
        let t4 = Instant::now();
        let compiled = isolated.compile(&snapshot, rig.target);
        let t5 = Instant::now();
        match compiled {
            Ok(Some(compiled)) if compiled.footprint == resolved.descriptor.footprint => {}
            _ => failures.push("isolated compile differs from the retained descriptor".into()),
        }
        run.cold.push(ColdObservation {
            install: t1 - t0,
            first_resolve: t3 - t2,
            isolated_compile: t5 - t4,
            counters,
            isolated_counters: isolated.counters(),
        });
        last = Some(resolved);
    }
    let cold = last.expect("one cold sample");
    // Warm: the descriptor DirectRig retained for this generation, resolved repeatedly.
    let descriptor = Rc::clone(&cold.descriptor);
    let generation = Arc::clone(&cold.capture.snapshot().fixtures);
    let mut previous = cold.result.continuity.clone();
    let mut previous_token: Option<CapturedFrameToken> = None;
    let mut reference: Option<WarmObservation> = None;
    for frame in 0..parameters.warmup + parameters.samples {
        let index = order.next(requests.len());
        let request = &requests[index];
        let before = rig.adapter.counters();
        let (resolved, token, phases) =
            match phased_frame(&rig, &descriptor, &request.value, Some(&previous)) {
                Ok(output) => output,
                Err(error) => {
                    failures.push(format!("warm frame rejected: {error:?}"));
                    continue;
                }
            };
        let counters = delta(rig.adapter.counters(), before);
        check(&resolved, request, fixture, rig.target, &mut failures);
        let mut fail = |message: String| failures.push(message);
        if !Arc::ptr_eq(&resolved.capture.snapshot().fixtures, &generation) {
            fail("the warm frame captured another generation".into());
        }
        if previous_token.as_ref() == Some(&token) {
            fail("two frames shared one capture token".into());
        }
        if counters.descriptor_compiles + counters.fitting_compiles + counters.fitting_cache_hits
            != 0
        {
            fail("a warm frame recompiled".into());
        }
        if counters.resolves != 1 {
            fail(format!("{} resolves in one frame", counters.resolves));
        }
        let quality = &resolved.result.quality;
        let work = quality.work;
        if (
            counters.fits,
            counters.candidates_ranked,
            counters.level_solves,
        ) != (
            u64::from(work.fits),
            u64::from(work.candidates_ranked),
            u64::from(work.fit.level_solves),
        ) {
            fail("adapter counters disagree with the published work".into());
        }
        let observation = WarmObservation {
            phases,
            counters,
            request: index,
            class: classify(quality),
            color_match: quality.color_match,
            visible: quality.visible,
            uv: quality.uv,
            constraint: quality.constraints.first().map(|c| c.status),
            writes: resolved.result.writes.iter().map(|w| w.raw).collect(),
        };
        // TL-553: the first warm frame may still fit (its continuity differs from the cold
        // frame's); every later unchanged frame replays it, so compare after the warmup.
        if scenario.sweep == Sweep::Unchanged && frame >= parameters.warmup {
            match &reference {
                None => reference = Some(observation.clone()),
                Some(first) => {
                    if first.writes != observation.writes {
                        fail("an unchanged request changed its writes".into());
                    }
                    if first.counters != counters {
                        fail("an unchanged request changed its work".into());
                    }
                }
            }
        }
        previous = resolved.result.continuity.clone();
        previous_token = Some(token);
        if frame >= parameters.warmup {
            run.samples.push(observation);
        }
    }
    // DirectRig itself still holds the same descriptor: no hidden recompilation.
    let before = rig.adapter.counters();
    match rig.resolve(&requests[0].value, Some(&previous)) {
        Ok(again) if Rc::ptr_eq(&again.descriptor, &descriptor) => {}
        Ok(_) => failures.push("DirectRig recompiled its retained descriptor".into()),
        Err(error) => failures.push(format!("DirectRig resolve rejected: {error:?}")),
    }
    if delta(rig.adapter.counters(), before).descriptor_compiles != 0 {
        failures.push("DirectRig recompiled its retained descriptor".into());
    }
    run.failures = failures.0;
    run
}

fn run_all(parameters: Parameters) -> Vec<Run> {
    let fixtures = bench_fixtures();
    let scenarios = scenarios();
    let mut runs = Vec::new();
    for (fixture_index, fixture) in fixtures.iter().enumerate() {
        for (scenario_index, scenario) in scenarios.iter().enumerate() {
            runs.push(run_one(
                fixture,
                fixture_index,
                scenario,
                scenario_index,
                parameters,
            ));
        }
    }
    runs
}

// ---------------------------------------------------------------------------------------------
// Deterministic smoke (normal test runs; functional assertions only, no timing).

#[test]
fn captured_color_fitting_smoke_reuses_descriptors_and_measures_changed_request_work() {
    let runs = run_all(Parameters {
        cold: 2,
        warmup: 1,
        samples: 6,
    });
    let run = |fixture: &str, scenario: &str| {
        runs.iter()
            .find(|run| run.fixture == fixture && run.scenario == scenario)
            .unwrap()
    };
    let mut measured = 0;
    for run in &runs {
        assert!(
            run.failures.is_empty(),
            "{}/{}: {:?}",
            run.fixture,
            run.scenario,
            run.failures
        );
        if let Some(reason) = run.skipped {
            assert_eq!(run.scenario, "wheel-constraint-sweep", "{reason}");
            continue;
        }
        measured += 1;
        assert_eq!(run.cold.len(), 2);
        assert_eq!(run.samples.len(), 6);
        // Cold: one descriptor and one fitter compile per new generation, bounded and stable.
        for cold in &run.cold {
            let c = cold.counters;
            assert_eq!(
                (
                    c.descriptor_compiles,
                    c.fitting_compiles,
                    c.fitting_cache_hits
                ),
                (1, 1, 0),
                "{}/{}",
                run.fixture,
                run.scenario
            );
            assert_eq!((c.resolves, c.fitting_failures), (1, 0));
            let i = cold.isolated_counters;
            assert_eq!((i.descriptor_compiles, i.fitting_compiles), (1, 1));
        }
        // Warm: no compile, one resolve and at least one actual fit per frame.
        for frame in &run.samples {
            let c = frame.counters;
            assert_eq!(
                (
                    c.descriptor_compiles,
                    c.fitting_compiles,
                    c.fitting_cache_hits
                ),
                (0, 0, 0)
            );
            assert_eq!(c.resolves, 1);
            if run.scenario == "warm-unchanged" {
                // TL-553: an unchanged request on unchanged raw values replays its last fit.
                assert_eq!((c.fits, c.result_reuses), (0, 1), "{}", run.fixture);
                assert_eq!(c.forward_evaluations, 0);
                continue;
            }
            assert!(c.fits >= 1, "{}/{}", run.fixture, run.scenario);
            assert_eq!(c.result_reuses, 0);
            assert!(c.forward_evaluations >= 1);
        }
    }
    // 7 fixtures × 7 scenarios, minus the wheel constraint on the three wheel-less fixtures.
    assert_eq!(measured, 46);

    // Unchanged warm requests repeat identical writes (checked per frame) and identical work.
    for run in runs.iter().filter(|run| run.scenario == "warm-unchanged") {
        assert!(
            run.samples
                .windows(2)
                .all(|w| w[0].counters == w[1].counters)
        );
    }
    // Candidate work per capability: continuous additive heads rank one candidate with one
    // visible solve; CMY flags and the wheel enumerate 81 filter states without a visible
    // solve; a wheel ranks its 3 steady slots; the spectral hybrid solves its emitters behind
    // each of the 3 slots.
    let work = |fixture: &str| {
        run(fixture, "hue-candidate-sweep")
            .samples
            .iter()
            .map(|frame| {
                let c = frame.counters;
                (
                    c.candidates_ranked,
                    c.visible_solves,
                    c.level_solves,
                    frame.class,
                )
            })
            .collect::<Vec<_>>()
    };
    for fixture in ["additive-rgb", "additive-rgbwauv-known-leakage"] {
        assert!(
            work(fixture)
                .iter()
                .all(|w| *w == (1, 1, 1, "numerical-fit"))
        );
    }
    assert!(
        work("subtractive-cmy-wheel")
            .iter()
            .all(|w| *w == (81, 0, 0, "numerical-fit"))
    );
    assert!(
        work("wheel-only")
            .iter()
            .all(|w| *w == (3, 0, 0, "numerical-fit"))
    );
    assert!(
        work("hybrid-spectral-rgbw-wheel")
            .iter()
            .all(|w| *w == (3, 3, 3, "numerical-fit"))
    );
    // The shipped TL-557 hybrid test profile has XYZ-only emitters behind a spectral wheel. Its
    // open slot has a unit transmission, which filters nothing (TL-552), so the emitters are
    // known through it and fitted numerically; the coloured slots stay unknown. A pin onto a
    // coloured slot (the constraint sweep) is the only way to leave the open slot.
    for run in runs.iter().filter(|run| {
        run.fixture == "hybrid-rgbw-wheel" && run.scenario != "wheel-constraint-sweep"
    }) {
        // TL-553: unchanged warm frames replay their fit and solve nothing.
        let solves = u64::from(run.scenario != "warm-unchanged");
        for frame in &run.samples {
            assert_eq!(frame.visible, VisibleFitStatus::Fitted, "{}", run.scenario);
            assert_eq!(frame.counters.visible_solves, solves, "{}", run.scenario);
        }
    }

    // UV: numerical fitting with frozen known leakage is separated from unsupported UV and
    // from unknown leakage, and only the known-leakage head runs the fixed-offset search.
    for frame in &run("additive-rgbwauv-known-leakage", "uv-sweep").samples {
        let uv = UV_AMOUNTS[frame.request];
        assert_eq!(frame.class, "numerical-fit");
        assert_eq!(frame.uv, UvFitStatus::Applied);
        let fixed = u64::from(uv > 0.);
        assert_eq!(frame.counters.fixed_offset_solves, fixed, "uv {uv}");
        assert_eq!(
            frame.counters.level_solves,
            if uv > 0. { 43 } else { 1 },
            "uv {uv}"
        );
    }
    for frame in &run("additive-rgbwauv-unknown-leakage", "uv-sweep").samples {
        let uv = UV_AMOUNTS[frame.request];
        assert_eq!(frame.uv, UvFitStatus::Applied);
        assert_eq!(frame.counters.fixed_offset_solves, 0);
        if uv > 0. {
            assert_eq!(frame.class, "unknown-uv-leakage", "uv {uv}");
            assert_eq!(frame.visible, VisibleFitStatus::PredictionIncomplete);
        }
    }
    for fixture in [
        "additive-rgb",
        "subtractive-cmy-wheel",
        "hybrid-rgbw-wheel",
        "hybrid-spectral-rgbw-wheel",
        "wheel-only",
    ] {
        for frame in &run(fixture, "uv-sweep").samples {
            let uv = UV_AMOUNTS[frame.request];
            if uv > 0. {
                assert_eq!(frame.class, "uv-unsupported", "{fixture} uv {uv}");
                assert_eq!(frame.uv, UvFitStatus::Unsupported);
            } else {
                assert_ne!(frame.class, "uv-unsupported");
            }
        }
    }
    // White Blend, CCT and Duv requests are actual numerical solves, never compiles.
    for fixture in [
        "additive-rgbwauv-known-leakage",
        "additive-rgbwauv-unknown-leakage",
        "hybrid-spectral-rgbw-wheel",
    ] {
        for scenario in ["white-blend-sweep", "cct-sweep", "duv-sweep"] {
            for frame in &run(fixture, scenario).samples {
                assert_eq!(frame.class, "numerical-fit", "{fixture}/{scenario}");
                assert!(frame.counters.level_solves >= 1);
            }
        }
    }
    // Wheel constraints: own pins apply, a foreign pin is reported, never reinterpreted.
    // An applied pin narrows the candidates to the pinned slot's.
    for (fixture, pinned) in [
        ("wheel-only", 1),
        ("subtractive-cmy-wheel", 27),
        ("hybrid-rgbw-wheel", 1),
        ("hybrid-spectral-rgbw-wheel", 1),
    ] {
        let run = run(fixture, "wheel-constraint-sweep");
        for frame in &run.samples {
            let own = run.labels[frame.request].starts_with("own-");
            let expected = match run.labels[frame.request].as_str() {
                "none" => None,
                _ if own => Some(ColorConstraintStatus::Applied),
                _ => Some(ColorConstraintStatus::SourceMismatch),
            };
            assert_eq!(frame.constraint, expected, "{fixture}");
            if own {
                // A refit (after parking a retained control) ranks the pinned candidates again.
                assert_eq!(
                    frame.counters.candidates_ranked,
                    pinned * (1 + frame.counters.refits),
                    "{fixture}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Manual release-mode benchmark: `#[ignore]`, no timing thresholds, immutable report.

fn env_usize(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a non-negative integer")),
        Err(_) => default,
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_path_buf()
}

/// Canonical performance root: explicit `LIGHT_PERFORMANCE_DIR`, else
/// `$LIGHT_ARTIFACTS_DIR/performance`, else `<repo>/.artifacts/performance`.
fn performance_root() -> (PathBuf, &'static str) {
    let repository = repository_root();
    let resolve = |name: &str, value: String| {
        assert!(!value.is_empty(), "{name} override cannot be empty");
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            repository.join(path)
        }
    };
    if let Ok(value) = std::env::var("LIGHT_PERFORMANCE_DIR") {
        return (
            resolve("LIGHT_PERFORMANCE_DIR", value),
            "LIGHT_PERFORMANCE_DIR",
        );
    }
    if let Ok(value) = std::env::var("LIGHT_ARTIFACTS_DIR") {
        return (
            resolve("LIGHT_ARTIFACTS_DIR", value).join("performance"),
            "LIGHT_ARTIFACTS_DIR/performance",
        );
    }
    (
        repository.join(".artifacts/performance"),
        "default .artifacts/performance",
    )
}

fn unavailable(reason: &str) -> Value {
    json!({"status": "unavailable", "reason": reason})
}

/// Nearest-rank distribution. Percentile resolution is limited by the sample count.
fn distribution(values: &[f64], unit: &str, source: &str) -> Value {
    if values.is_empty() {
        return unavailable("no samples recorded");
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank =
        |p: f64| sorted[((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1];
    json!({
        "status": "measured",
        "unit": unit,
        "source": source,
        "n": sorted.len(),
        "min": sorted[0],
        "p50": rank(0.50),
        "p95": rank(0.95),
        "p99": rank(0.99),
        "max": sorted[sorted.len() - 1],
        "total": sorted.iter().sum::<f64>(),
        "mean": sorted.iter().sum::<f64>() / sorted.len() as f64,
        "percentileMethod": "nearest-rank",
    })
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

/// One counter field of every observation of a run.
type CounterField<'a> = &'a dyn Fn(&ColorAdapterCounters) -> u64;

fn counters_json(series: &dyn Fn(CounterField) -> Vec<f64>) -> Value {
    let source = "ColorAdapter::counters delta per frame";
    let c = |f: &dyn Fn(&ColorAdapterCounters) -> u64| distribution(&series(f), "count", source);
    json!({
        "resolves": c(&|c| c.resolves),
        "fits": c(&|c| c.fits),
        "refits": c(&|c| c.refits),
        "candidatesRanked": c(&|c| c.candidates_ranked),
        "visibleSolves": c(&|c| c.visible_solves),
        "fixedOffsetSolves": c(&|c| c.fixed_offset_solves),
        "levelSolves": c(&|c| c.level_solves),
        "forwardEvaluations": c(&|c| c.forward_evaluations),
        "descriptorCompiles": c(&|c| c.descriptor_compiles),
        "fittingCompiles": c(&|c| c.fitting_compiles),
        "fittingCacheHits": c(&|c| c.fitting_cache_hits),
        "fittingFailures": c(&|c| c.fitting_failures),
        "sharedConflicts": c(&|c| c.shared_conflicts),
        "fitResultCache": {"status": "not-present",
            "reason": "ColorAdapter keeps no fit-result memo: every resolve fits; only compiled fitters are cached per generation"},
    })
}

/// Count of each status, keyed by its `Debug` name (class names are used as they are).
fn histogram<T: std::fmt::Debug>(values: impl Iterator<Item = T>) -> Value {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        let key = format!("{value:?}");
        *counts.entry(key.trim_matches('"').to_string()).or_default() += 1;
    }
    json!(counts)
}

fn run_json(run: &Run) -> Value {
    let base = json!({
        "fixture": run.fixture,
        "capability": run.capability,
        "uvModel": run.uv_model,
        "profileBuilder": run.builder,
        "scenario": run.scenario,
        "description": run.description,
        "requests": run.labels,
    });
    if let Some(reason) = run.skipped {
        let mut value = base;
        value["status"] = json!("not-applicable");
        value["reason"] = json!(reason);
        return value;
    }
    let wall = "std::time::Instant (monotonic) around test-side phase boundaries";
    let warm = |f: &dyn Fn(&WarmObservation) -> f64| run.samples.iter().map(f).collect::<Vec<_>>();
    let cold = |f: &dyn Fn(&ColdObservation) -> f64| run.cold.iter().map(f).collect::<Vec<_>>();
    let mut by_class = BTreeMap::<&str, Vec<f64>>::new();
    for frame in &run.samples {
        by_class
            .entry(frame.class)
            .or_default()
            .push(micros(frame.phases.total()));
    }
    let mut by_request = BTreeMap::<String, Value>::new();
    for (index, label) in run.labels.iter().enumerate() {
        let frames = run
            .samples
            .iter()
            .filter(|frame| frame.request == index)
            .collect::<Vec<_>>();
        let series = |f: &dyn Fn(&WarmObservation) -> f64| {
            frames.iter().map(|frame| f(frame)).collect::<Vec<_>>()
        };
        by_request.insert(
            label.clone(),
            json!({
                "uvAmount": run.uv_amounts[index],
                "frames": frames.len(),
                "class": histogram(frames.iter().map(|frame| frame.class)),
                "resolveUs": distribution(&series(&|f| micros(f.phases.resolve)), "us", wall),
                "candidatesRanked": distribution(&series(&|f| f.counters.candidates_ranked as f64), "count", "ColorAdapter::counters delta"),
                "levelSolves": distribution(&series(&|f| f.counters.level_solves as f64), "count", "ColorAdapter::counters delta"),
            }),
        );
    }
    let mut value = base;
    value["status"] = json!("measured");
    value["cold"] = json!({
        "samples": run.cold.len(),
        "timing": {
            "install": distribution(&cold(&|c| micros(c.install)), "us",
                "DirectRig::install: catalogue and Engine::replace_snapshot of a new fixture list"),
            "firstResolve": distribution(&cold(&|c| micros(c.first_resolve)), "us",
                "DirectRig::resolve of a new generation: capture, scalar, geometry, descriptor+fitter compile, resolve, write validation"),
            "isolatedCompile": distribution(&cold(&|c| micros(c.isolated_compile)), "us",
                "ColorAdapter::compile of the same captured snapshot on a fresh adapter"),
        },
        "counters": counters_json(&|f: CounterField| {
            run.cold.iter().map(|c| f(&c.counters) as f64).collect()
        }),
    });
    value["warm"] = json!({
        "warmupFrames": run.warmup,
        "sampleFrames": run.samples.len(),
        "timing": {
            "endToEnd": distribution(&warm(&|f| micros(f.phases.total())), "us", wall),
            "phases": {
                "capture": distribution(&warm(&|f| micros(f.phases.capture)), "us",
                    "Engine::prepare_output_frame + frame_token"),
                "scalar": distribution(&warm(&|f| micros(f.phases.scalar)), "us",
                    "Engine::prepare_static_family_frame"),
                "geometry": distribution(&warm(&|f| micros(f.phases.geometry)), "us",
                    "Engine::observe_static_family_geometry"),
                "resolve": distribution(&warm(&|f| micros(f.phases.resolve)), "us",
                    "ColorAdapter::resolve on the retained descriptor: token-bound native read, seeding, fitting, parking, forward publication"),
                "validate": distribution(&warm(&|f| micros(f.phases.validate)), "us",
                    "validate_complete_writes"),
                "fitterOnly": unavailable("inside resolve; no existing API separates CompiledColorFitting::fit without production instrumentation"),
                "perCandidate": unavailable("candidate ranking is not individually timed by any existing API"),
                "nativeEncoding": unavailable("the engine finalizer is not part of this adapter benchmark; encoding is checked functionally outside timing"),
            },
            "endToEndByClass": by_class.iter().map(|(class, values)| ((*class).to_string(),
                distribution(values, "us", wall))).collect::<serde_json::Map<_, _>>(),
        },
        "counters": counters_json(&|f: CounterField| {
            run.samples.iter().map(|s| f(&s.counters) as f64).collect()
        }),
        "classes": histogram(run.samples.iter().map(|f| f.class)),
        "colorMatch": histogram(run.samples.iter().filter(|f| f.class == "numerical-fit").map(|f| f.color_match)),
        "visibleStatus": histogram(run.samples.iter().map(|f| f.visible)),
        "uvStatus": histogram(run.samples.iter().map(|f| f.uv)),
        "constraintStatus": histogram(run.samples.iter().map(|f| f.constraint)),
        "byRequest": by_request,
        "frames": run.samples.iter().map(|f| json!({
            "request": run.labels[f.request],
            "captureUs": micros(f.phases.capture),
            "scalarUs": micros(f.phases.scalar),
            "geometryUs": micros(f.phases.geometry),
            "resolveUs": micros(f.phases.resolve),
            "validateUs": micros(f.phases.validate),
            "fits": f.counters.fits,
            "candidatesRanked": f.counters.candidates_ranked,
            "levelSolves": f.counters.level_solves,
            "class": f.class,
        })).collect::<Vec<_>>(),
    });
    value["functional"] = json!({"passed": run.failures.is_empty(), "failures": run.failures});
    value
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn binary_identity() -> Value {
    let Ok(path) = std::env::current_exe() else {
        return unavailable("current_exe unavailable");
    };
    match std::fs::read(&path) {
        Ok(bytes) => json!({
            "status": "recorded",
            "kind": "cargo test binary (light-headless-runtime lib tests)",
            "path": path.display().to_string(),
            "sha256": sha256_hex(&bytes),
            "bytes": bytes.len(),
        }),
        Err(_) => unavailable("test binary unreadable"),
    }
}

fn supplied(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn source_identity() -> Value {
    match supplied("LIGHT_COLOR_BENCH_SOURCE_SHA256") {
        Some(sha) => json!({
            "status": "supplied",
            "sourceSha256": sha,
            "manifest": supplied("LIGHT_COLOR_BENCH_SOURCE_MANIFEST")
                .map_or_else(|| unavailable("no manifest path supplied"), Value::String),
            "origin": supplied("LIGHT_COLOR_BENCH_SOURCE_ORIGIN")
                .map_or_else(|| unavailable("no origin supplied"), Value::String),
            "verifiedByBenchmark": false,
        }),
        None => unavailable(
            "set LIGHT_COLOR_BENCH_SOURCE_SHA256 (for example from tools/semantic-source-manifest.mjs)",
        ),
    }
}

fn host_identity() -> Value {
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "logicalCpus": std::thread::available_parallelism()
            .map_or_else(|_| unavailable("available_parallelism failed"), |n| json!(n.get())),
        "cpuModel": supplied("LIGHT_COLOR_BENCH_HOST_CPU")
            .map_or_else(|| unavailable("not supplied (LIGHT_COLOR_BENCH_HOST_CPU)"), Value::String),
        "memory": unavailable("not collected by the Rust test harness"),
        "gpu": unavailable("not used: no native Stage or presentation in this microbenchmark"),
        "loadIsolation": unavailable("other processes were not controlled"),
    })
}

/// Deterministic description of the workload; its digest is the workload identity.
fn workload_json(parameters: Parameters) -> Value {
    let fixtures = bench_fixtures();
    json!({
        "fixturesPerCapture": 1,
        "simultaneousOutputFrame": false,
        "statement": "Sequential one-fixture captures. Never aggregate these samples into a simultaneous multi-fixture frame.",
        "seed": format!("{SEED:#018x}"),
        "requestOrder": "xorshift64* from seed; never the same request twice in a row",
        "coldSamplesPerRun": parameters.cold.max(1),
        "warmupFramesPerRun": parameters.warmup,
        "sampleFramesPerRun": parameters.samples,
        "fixtures": fixtures.iter().map(|f| json!({
            "name": f.name,
            "capability": f.capability,
            "uvModel": f.uv_model,
            "profileBuilder": f.builder,
            "colorControls": f.profile.modes[0].channels.len() - 1,
        })).collect::<Vec<_>>(),
        "scenarios": scenarios().iter().map(|s| json!({
            "name": s.name,
            "description": s.description,
            "requests": fixtures.iter().map(|f| (f.name.to_string(),
                requests(s.sweep, f).map_or_else(|reason| json!({"notApplicable": reason}),
                    |r| json!(r.iter().map(|r| r.label.clone()).collect::<Vec<_>>()))))
                .collect::<serde_json::Map<_, _>>(),
        })).collect::<Vec<_>>(),
        "largeFixtureModes": unavailable("not implemented: optional multi-fixture modes cannot substitute for TL-596 frame gates"),
    })
}

fn summary_markdown(report: &Value) -> String {
    let mut text = String::new();
    text.push_str("# Captured Color fitting benchmark (TL-624)\n\n");
    text.push_str("Synthetic sequential one-fixture fit workload. Not a simultaneous output frame, render deadline, physical colour match, native Stage or TL-596 acceptance.\n\n");
    text.push_str(&format!(
        "- Build profile: {}\n- Cold/warmup/sample per run: {}/{}/{}\n- Workload SHA-256: {}\n- Functional result: {}\n\n",
        report["build"]["profile"],
        report["workload"]["identity"]["coldSamplesPerRun"],
        report["workload"]["identity"]["warmupFramesPerRun"],
        report["workload"]["identity"]["sampleFramesPerRun"],
        report["workload"]["sha256"],
        if report["functional"]["passed"] == json!(true) { "passed" } else { "FAILED" },
    ));
    text.push_str("| Fixture | Scenario | cold first resolve p50 us | isolated compile p50 us | warm p50 us | warm p95 us | warm p99 us | resolve p50 us | fits p50 | candidates p50 | level solves p50 | classes |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
    let f = |value: &Value| {
        value
            .as_f64()
            .map_or_else(|| "n/a".into(), |v| format!("{v:.1}"))
    };
    for run in report["runs"].as_array().into_iter().flatten() {
        if run["status"] != json!("measured") {
            text.push_str(&format!(
                "| {} | {} | not applicable | | | | | | | | | |\n",
                run["fixture"].as_str().unwrap_or_default(),
                run["scenario"].as_str().unwrap_or_default(),
            ));
            continue;
        }
        let warm = &run["warm"];
        let e2e = &warm["timing"]["endToEnd"];
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            run["fixture"].as_str().unwrap_or_default(),
            run["scenario"].as_str().unwrap_or_default(),
            f(&run["cold"]["timing"]["firstResolve"]["p50"]),
            f(&run["cold"]["timing"]["isolatedCompile"]["p50"]),
            f(&e2e["p50"]),
            f(&e2e["p95"]),
            f(&e2e["p99"]),
            f(&warm["timing"]["phases"]["resolve"]["p50"]),
            f(&warm["counters"]["fits"]["p50"]),
            f(&warm["counters"]["candidatesRanked"]["p50"]),
            f(&warm["counters"]["levelSolves"]["p50"]),
            warm["classes"],
        ));
    }
    text.push_str("\nWarm timing excludes request construction and functional checks. Classes separate numerical fits from unsupported UV and unknown UV leakage.\n");
    text
}

/// Manual: see `docs/engineering/color-fitting-benchmarks.md` for the full rerun command.
#[test]
#[ignore = "manual release-mode benchmark; writes an immutable report under .artifacts/performance"]
fn manual_color_fitting_benchmark() {
    let started_at = chrono::Utc::now();
    let started = Instant::now();
    let parameters = Parameters {
        cold: env_usize("LIGHT_COLOR_BENCH_COLD", 20).clamp(1, 200),
        warmup: env_usize("LIGHT_COLOR_BENCH_WARMUP", 20).clamp(1, 1000),
        samples: env_usize("LIGHT_COLOR_BENCH_SAMPLES", 300).clamp(1, 10_000),
    };
    let runs = run_all(parameters);
    let elapsed = started.elapsed();
    let passed = runs.iter().all(|run| run.failures.is_empty());
    let workload = workload_json(parameters);
    let workload_sha = sha256_hex(serde_json::to_string(&workload).unwrap().as_bytes());
    let report = json!({
        "schema": "tosklight.color-fitting-benchmark/v1",
        "issue": "TL-624",
        "evidenceClass": "gated-software-microbenchmark",
        "claims": {
            "simultaneousOutputFrame": false,
            "completeRenderDeadline": false,
            "physicalColorMatch": false,
            "nativeStage": false,
            "stageAcceptance": false,
            "frameRateAcceptance": false,
            "thresholds": false,
            "statement": "Actual captured Color adapter compile/resolve on test Engine instances, one fixture per capture. Synthetic fit workload only; TL-596 keeps output deadlines, paired builds and frame gates.",
        },
        "run": {
            "startedAtUtc": started_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "elapsedMillis": elapsed.as_secs_f64() * 1e3,
            "coldPath": "DirectRig::install -> DirectRig::resolve (capture, scalar, geometry, ColorAdapter::compile, resolve, validate_complete_writes)",
            "warmPath": "Engine::prepare_output_frame -> prepare_static_family_frame -> observe_static_family_geometry -> ColorAdapter::resolve(retained descriptor) -> validate_complete_writes",
            "checksOutsideTiming": "complete owned writes, unchanged request, generation and token identity, DMX encode/decode forward equality, Resolved::assert_forward",
        },
        "time": {
            "simulated": unavailable("warm frames do not advance DirectRig's private ManualClock; static Color fitting reads no time"),
            "observedOutputRate": unavailable("not an output loop; frames run back to back"),
            "wallClock": "std::time::Instant (monotonic)",
        },
        "build": {
            "profile": if cfg!(debug_assertions) { "debug-assertions (not release)" } else { "release (debug_assertions off)" },
            "binary": binary_identity(),
            "source": source_identity(),
            "rustc": supplied("LIGHT_COLOR_BENCH_RUSTC")
                .map_or_else(|| unavailable("not supplied (LIGHT_COLOR_BENCH_RUSTC)"), Value::String),
            "productionProgrammingContract": unavailable("not read; the Color adapter is driven directly on test Engine instances"),
        },
        "host": host_identity(),
        "workload": {"sha256": workload_sha, "identity": workload},
        "runs": runs.iter().map(run_json).collect::<Vec<_>>(),
        "functional": {
            "passed": passed,
            "failedRuns": runs.iter().filter(|run| !run.failures.is_empty())
                .map(|run| format!("{}/{}", run.fixture, run.scenario)).collect::<Vec<_>>(),
        },
    });
    let (root, origin) = performance_root();
    let parent = root.join("color-fitting");
    std::fs::create_dir_all(&parent).unwrap();
    let run_id = format!(
        "{}-s{}-pid{}",
        started_at.format("%Y%m%dT%H%M%S%.3fZ"),
        parameters.samples,
        std::process::id()
    );
    let directory = parent.join(run_id);
    // `create_dir` fails on an existing run: reports are immutable per run.
    std::fs::create_dir(&directory).unwrap();
    let write = |name: &str, contents: String| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(name))
            .unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    };
    let mut report = report;
    report["run"]["output"] = json!({
        "directory": directory.display().to_string(),
        "resolvedFrom": origin,
    });
    report["run"]["rerun"] = json!(
        "CARGO_TARGET_DIR=$PWD/.artifacts/build/cargo LIGHT_TMP_DIR=$PWD/.artifacts/tmp cargo test --release -p light-headless-runtime --lib manual_color_fitting_benchmark -- --ignored --nocapture --test-threads=1"
    );
    report["run"]["parameters"] = json!({
        "LIGHT_COLOR_BENCH_COLD": parameters.cold,
        "LIGHT_COLOR_BENCH_WARMUP": parameters.warmup,
        "LIGHT_COLOR_BENCH_SAMPLES": parameters.samples,
    });
    write(
        "report.json",
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    );
    write("summary.md", summary_markdown(&report));
    println!("TL-624 report: {}", directory.display());
    println!("{}", summary_markdown(&report));
    assert!(
        passed,
        "functional failures (timing is recorded separately): {:?}",
        report["functional"]["failedRuns"]
    );
}
