//! TL-593 route references: a composed semantic Color intent on the shipped ToskLight Media
//! Server profile resolves against a real captured frame into the personality's tint/Grayscale
//! controls, and those writes encode into the exact DMX bytes the Media decoder reads. The
//! Media-side decode and GPU render of the same translation are pinned in
//! `crates/media/adapters/render/tests/white_blend_renders.rs`.
use super::super::color::profiles::{cmy_wheel, patched, rgb};
use super::super::color::tests::{intent, magenta, program};
use super::*;
use light_core::programming::UvIntent;
use light_core::{AttributeKey, ManualClock, SessionId};
use light_dynamics::DynamicRuntime;
use light_engine::{Engine, RenderOptions};
use light_fixture::{FixtureProfile, PatchedHead};
use light_programmer::ProgrammerRegistry;

/// Media personality slot offsets (`media_domain::personality::channels`): layer Cyan,
/// Magenta, Yellow, Grayscale at 16–19 of each 59-slot layer block; master cyan, magenta,
/// yellow at 2–4 of the 40-slot master block that follows the layers.
const LAYER_BLOCK: usize = 59;
const LAYER_COLOR: [usize; 4] = [16, 17, 18, 19];
const MASTER_COLOR: [usize; 3] = [2, 3, 4];
const LAYER_DIMMER: usize = 14;

pub(in crate::runtime) fn shipped_media_server() -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/tosklight--media-server.toskfixture");
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

/// The 2-layer mode patched at address 1 with each layer as its own logical head; the master
/// head stays on the root fixture.
pub(in crate::runtime) fn media_fixture(
    profile: &FixtureProfile,
    root: FixtureId,
    layers: &[FixtureId],
) -> light_fixture::PatchedFixture {
    let mut fixture = patched(profile, root, 1);
    let mode = &profile.modes[0];
    let mut ids = layers.iter();
    for (index, head) in mode.heads.iter().enumerate() {
        if head.master_shared {
            continue;
        }
        fixture.logical_heads.push(PatchedHead {
            profile_head_id: Some(head.id),
            head_index: fixture.definition.heads[index].index,
            fixture_id: *ids.next().expect("one id per layer"),
        });
    }
    fixture
}

struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    clock: Arc<ManualClock>,
    root: FixtureId,
    layers: [FixtureId; 2],
    profile: FixtureProfile,
    adapter: MediaColorAdapter,
}

struct Resolved {
    descriptor: MediaColorDescriptor,
    native: Vec<u32>,
    result: PhysicalResolution<MediaColorAdapter>,
}

impl Rig {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let rig = Self {
            engine: Engine::new(programmers.clone()),
            programmers,
            session,
            clock,
            root: FixtureId::new(),
            layers: [FixtureId::new(), FixtureId::new()],
            profile: shipped_media_server(),
            adapter: MediaColorAdapter::default(),
        };
        rig.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![media_fixture(&rig.profile, rig.root, &rig.layers)].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        rig
    }

    fn resolve_with(
        &self,
        target: FixtureId,
        value: &AttributeValue,
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
            .compile(&capture.snapshot(), target)?
            .expect("Media color destination");
        let native = scalar
            .native_raw(&capture, &token, target)
            .unwrap()
            .raw()
            .to_vec();
        let result = self.adapter.resolve(PhysicalRequest {
            frame,
            target,
            owner: ProgrammingOwner::Color,
            descriptor: &descriptor,
            value,
            previous: None,
        })?;
        Ok(Resolved {
            descriptor,
            native,
            result,
        })
    }

    fn resolve(&self, target: FixtureId, request: &ColorIntent) -> Resolved {
        let resolved = self
            .resolve_with(target, &program(request), RenderOptions::default())
            .unwrap();
        validate_complete_writes(&resolved.descriptor.footprint, &resolved.result.writes).unwrap();
        assert_eq!(&resolved.result.requested, request, "never rewritten");
        resolved
    }

    /// Encode the head's pre-master native vector with the writes applied into a universe.
    fn dmx(&self, resolved: &Resolved) -> [u8; 512] {
        let mut output = resolved.native.clone();
        for write in &resolved.result.writes {
            assert_eq!(write.slot.destination, self.root);
            output[write.slot.channel_index as usize] = write.raw;
        }
        let plan = self.profile.modes[0].compile_encoding_plan().unwrap();
        let mut bytes = [0u8; 512];
        let values: Vec<_> = (0u32..).zip(output.iter().copied()).collect();
        plan.encode_split_by_index(&mut bytes, 1, 1, &values)
            .unwrap();
        bytes
    }
}

fn blended(rgb: [f32; 3], white_blend: f32) -> ColorIntent {
    ColorIntent {
        white_blend,
        ..intent(rgb, 0.)
    }
}

#[test]
fn media_heads_compile_to_personality_controls_and_lamp_fitting_declines_them() {
    let rig = Rig::new();
    let snapshot = rig.engine.snapshot();
    let layer = rig
        .adapter
        .compile(&snapshot, rig.layers[1])
        .unwrap()
        .unwrap();
    assert_eq!(layer.head.surface, MediaColorSurface::Layer);
    assert_eq!(layer.footprint.len(), 4);
    assert_eq!(layer.destination, rig.root);
    let master = rig.adapter.compile(&snapshot, rig.root).unwrap().unwrap();
    assert_eq!(master.head.surface, MediaColorSurface::Master);
    assert_eq!(
        master.footprint.len(),
        3,
        "the master has no White Blend control"
    );

    let lamp = ColorAdapter::default();
    for target in [rig.root, rig.layers[0], rig.layers[1]] {
        assert!(
            lamp.compile(&snapshot, target).unwrap().is_none(),
            "lamp RGBW/CMY fitting never drives a Media head"
        );
    }
    // And the reverse: a lamp is not a Media head.
    let rgb_target = FixtureId::new();
    let lamp_engine = Engine::new(ProgrammerRegistry::default());
    lamp_engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![patched(&rgb(), rgb_target, 1)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let lamp_snapshot = lamp_engine.snapshot();
    assert!(lamp.compile(&lamp_snapshot, rgb_target).unwrap().is_some());
    assert!(
        rig.adapter
            .compile(&lamp_snapshot, rgb_target)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_media_head_is_never_lamp_fitted_even_when_a_lamp_color_model_exists() {
    // The shipped Media profile has no lamp forward model at all. This synthetic head carries
    // the TL-592 CMY+wheel lamp model *and* the Media personality identities: the lamp fitter
    // could fit it, but ownership stays with Media White Blend semantics.
    let lamp_model = cmy_wheel();
    let mut media = lamp_model.clone();
    for channel in &mut media.modes[0].channels {
        let native = match &*channel.fixture_attribute.0 {
            "color.cyan" => "media.layer.cyan",
            "color.magenta" => "media.layer.magenta",
            "color.yellow" => "media.layer.yellow",
            _ => continue,
        };
        channel.fixture_attribute = AttributeKey(native.into());
        channel.reacts_to_virtual_intensity = false;
        channel.reacts_to_sequence_master = false;
        channel.reacts_to_group_master = false;
        channel.reacts_to_grand_master = false;
        channel.functions[0].behavior = light_fixture::ChannelFunctionBehavior::Continuous {
            physical_min: 0.0,
            physical_max: 255.0,
            unit: None,
        };
    }
    // Retain the wheel's real lamp model and add the separate Media Grayscale wire control.
    let cyan_index = media.modes[0]
        .channels
        .iter()
        .position(|channel| &*channel.fixture_attribute.0 == "media.layer.cyan")
        .unwrap();
    let mut grayscale = media.modes[0].channels[cyan_index].clone();
    grayscale.id = Uuid::new_v4();
    grayscale.fixture_attribute = AttributeKey("media.layer.grayscale".into());
    grayscale.attribute = grayscale.fixture_attribute.clone();
    grayscale.canonical_transform = light_fixture::CanonicalTransform::Identity;
    grayscale.functions[0].id = Uuid::new_v4();
    grayscale.functions[0].attribute = grayscale.attribute.clone();
    media.modes[0].channels.push(grayscale);
    media.modes[0].splits[0].footprint += 1;
    assert!(
        light_fixture::forward::CompiledColorForward::compile(&media, media.modes[0].id, None)
            .unwrap()
            .is_some(),
        "the synthetic Media head still has a real lamp forward model"
    );
    let mut unsupported = media.clone();
    unsupported.modes[0].channels[cyan_index].resolution = light_fixture::ChannelResolution::U16;
    unsupported.modes[0].splits[0].footprint += 1;
    unsupported.modes[0].channels[cyan_index].secondary_slots =
        vec![unsupported.modes[0].splits[0].footprint];
    let lamp = ColorAdapter::default();
    let adapter = MediaColorAdapter::default();
    for (profile, lamp_supported, media_supported) in [
        (&lamp_model, true, false),
        (&media, false, true),
        (&unsupported, false, false),
    ] {
        let target = FixtureId::new();
        let engine = Engine::new(ProgrammerRegistry::default());
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(profile, target, 1)].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let snapshot = engine.snapshot();
        assert_eq!(
            lamp.compile(&snapshot, target).unwrap().is_some(),
            lamp_supported
        );
        assert_eq!(
            adapter.compile(&snapshot, target).unwrap().is_some(),
            media_supported
        );
    }
}

#[test]
fn white_blend_0_50_100_black_dim_and_tinted_white_reach_the_personality_bytes() {
    let rig = Rig::new();
    let mut dim = blended([1., 0.735, 0.], 0.5);
    dim.relative_output = 0.5;
    let cases: [(&str, ColorIntent, [u8; 4]); 7] = [
        ("white 0%", blended([1.; 3], 0.), [0, 0, 0, 0]),
        ("white 50%", blended([1.; 3], 0.5), [0, 0, 0, 128]),
        ("white 100%", blended([1.; 3], 1.), [0, 0, 0, 255]),
        (
            "red tint kept at 100%",
            blended([1., 0., 0.], 1.),
            [0, 255, 255, 255],
        ),
        (
            "tinted white 50%",
            blended([1., 0.735, 0.], 0.5),
            [0, 128, 255, 128],
        ),
        ("dim tinted white", dim, [128, 191, 255, 128]),
        ("black", blended([0.; 3], 0.5), [255, 255, 255, 128]),
    ];
    for (layer_index, target) in rig.layers.iter().enumerate() {
        for (name, request, expected) in &cases {
            let resolved = rig.resolve(*target, request);
            let bytes = rig.dmx(&resolved);
            let block = layer_index * LAYER_BLOCK;
            assert_eq!(
                LAYER_COLOR.map(|offset| bytes[block + offset]),
                *expected,
                "{name} on layer {}",
                layer_index + 1
            );
            let achieved = resolved.result.achieved;
            assert_eq!(
                achieved.white_blend,
                Some(f32::from(expected[3]) / 255.),
                "{name}"
            );
            for (component, byte) in achieved.tint.iter().zip(expected) {
                assert_eq!(*component, 1. - f32::from(*byte) / 255., "{name}");
            }
            // The other layer and the master keep their scalar baseline.
            let other = (1 - layer_index) * LAYER_BLOCK;
            assert_eq!(LAYER_COLOR.map(|offset| bytes[other + offset]), [0; 4]);
        }
    }
    assert_eq!(rig.adapter.counters().resolves, 14);
}

#[test]
fn intensity_alpha_masters_and_blackout_stay_independent_of_media_color() {
    let rig = Rig::new();
    let request = magenta();
    let base = rig.resolve(rig.layers[0], &request);
    assert!(
        base.result
            .writes
            .iter()
            .all(
                |w| rig.profile.modes[0].channels[w.slot.channel_index as usize]
                    .attribute
                    .0
                    != "intensity".into()
            ),
        "the layer dimmer is Intensity, never a Color write"
    );
    rig.programmers.set(
        rig.session,
        rig.layers[0],
        AttributeKey("intensity".into()),
        AttributeValue::Normalized(0.6),
    );
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
        let resolved = rig
            .resolve_with(rig.layers[0], &program(&request), options)
            .unwrap();
        let raws = |r: &Resolved| r.result.writes.iter().map(|w| w.raw).collect::<Vec<_>>();
        assert_eq!(raws(&resolved), raws(&base), "{options:?}");
        assert_eq!(resolved.result.achieved, base.result.achieved);
        assert_eq!(rig.dmx(&resolved)[LAYER_DIMMER], 153, "pre-master dimmer");
    }
    // relativeOutput dims the tint linearly, White Blend and dimmer are untouched.
    assert_eq!(base.result.quality.derived.white_blend, Some(0.25));
    let tint = base.result.quality.derived.tint;
    let expected = [0.8, 0., 0.8];
    assert!(
        tint.iter().zip(expected).all(|(t, e)| (t - e).abs() < 1e-4),
        "{tint:?}"
    );
}

#[test]
fn the_master_writes_tint_only_and_reports_white_blend_as_absent() {
    let rig = Rig::new();
    let mut request = blended([1., 0.735, 0.], 0.7);
    request.uv = UvIntent { amount: 0.4 };
    let resolved = rig.resolve(rig.root, &request);
    let bytes = rig.dmx(&resolved);
    let master = 2 * LAYER_BLOCK;
    assert_eq!(
        MASTER_COLOR.map(|offset| bytes[master + offset]),
        [0, 128, 255]
    );
    assert_eq!(
        resolved.result.achieved.white_blend, None,
        "not an authored 0"
    );
    let limits = resolved.result.quality.limitations;
    assert!(limits.white_blend_unsupported && limits.uv_unsupported);
    assert_eq!(resolved.result.quality.surface, MediaColorSurface::Master);
    // No layer Grayscale is written on behalf of the master.
    for block in [0, LAYER_BLOCK] {
        assert_eq!(bytes[block + LAYER_COLOR[3]], 0);
    }
}

#[test]
fn non_semantic_color_stays_a_passive_requirement() {
    let rig = Rig::new();
    // A scalar or Direct native recipe is not a semantic request; Media never guesses one.
    let direct = AttributeValue::Normalized(0.5);
    assert!(matches!(
        rig.resolve_with(rig.layers[0], &direct, RenderOptions::default()),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    ));
}
