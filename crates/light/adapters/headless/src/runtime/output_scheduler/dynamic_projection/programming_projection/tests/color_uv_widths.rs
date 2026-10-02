//! TL-611: semantic UV at 24-bit and 32-bit native resolution through the actual captured
//! hybrid seam (engine capture, Dynamic composition, ColorAdapter, engine finalizer).
//!
//! Every profile here is a local synthetic RGB+UV profile built with the cfg(test) builder; it
//! proves software behavior, not a lamp calibration. The sidecar writes are not injected into
//! DMX by production yet: the byte encoding below is TEST-SUPPLIED native input, encoded by the
//! profile's own encoding plan, decoded and forward-evaluated by the test.
use super::super::super::physical_adapter::color::profiles::{
    Builder, patched, provenance, rgb_columns,
};
use super::super::super::physical_adapter::color::tests::{intent, program, uv_only_black};
use super::super::super::physical_adapter::*;
use super::color_physical::{Live, color_lane, semantic};
use super::*;
use light_engine::PreparedOutputFrame;
use light_fixture::forward::{
    ColorForwardResult, ColorMatch, CompiledColorFitting, UvFitStatus, VisibleFitStatus,
};
use light_fixture::{
    ChannelResolution, FixtureProfile, InstalledColorCalibration, InstalledColorPathCalibration,
    InstalledEmitterCalibration, OpticalEmitterBand, OpticalProvenance, OpticalSource,
    PhysicalDataQuality,
};

/// Mode channel index of the UV control: Intensity 0, Red 1, Green 2, Blue 3, UV 4.
const UV: u32 = 4;
const COLOR_CONTROLS: [u32; 4] = [1, 2, 3, UV];
const WIDTHS: [ChannelResolution; 2] = [ChannelResolution::U24, ChannelResolution::U32];

/// `(amount, U24 word, U32 word)`: round(f64(amount) * max), computed independently with exact
/// rational arithmetic. The interior words have nonzero low bytes and a fractional part away
/// from one half, so dropping fine bytes, flooring or 8-bit quantization all change them.
const CASES: [(f32, u32, u32); 4] = [
    (0.0, 0, 0),
    (0.123, 0x001f_7cee, 0x1f7c_eda0),
    (0.6, 0x0099_9999, 0x9999_99ff),
    (1.0, 0x00ff_ffff, 0xffff_ffff),
];

fn word(width: ChannelResolution, (_, u24, u32): (f32, u32, u32)) -> u32 {
    match width {
        ChannelResolution::U24 => u24,
        ChannelResolution::U32 => u32,
        _ => unreachable!("TL-611 covers U24 and U32"),
    }
}

/// RGB (U8) plus one UV emitter of `width` with unknown visible appearance (no XYZ, no
/// spectrum): UV is independent excitation that the visible fitter can neither use nor see.
fn rgb_uv(width: ChannelResolution, reversed: bool) -> FixtureProfile {
    let [r, g, b] = rgb_columns();
    let mut builder = Builder::new(&format!("TL-611 RGB+UV {width:?} reversed={reversed}"))
        .emitter("color.red", Some(r))
        .emitter("color.green", Some(g))
        .emitter("color.blue", Some(b));
    builder.emitter_with("color.uv", width, |e| {
        e.xyz = None;
        e.native_reversed = reversed;
        e.provenance = provenance(PhysicalDataQuality::Estimated);
    });
    let profile = builder.build();
    // The directed native control/function metadata the adapter must preserve.
    let mode = &profile.modes[0];
    let channel = &mode.channels[UV as usize];
    assert_eq!(channel.attribute.0.as_ref(), "color.uv");
    assert_eq!(channel.resolution, width);
    assert_eq!(channel.secondary_slots.len(), width.bytes() - 1);
    assert_eq!(channel.functions.len(), 1);
    assert_eq!(
        (channel.functions[0].dmx_from, channel.functions[0].dmx_to),
        (0, width.max_raw())
    );
    let OpticalSource::Additive { emitters } =
        &mode.color_physical.as_ref().unwrap().paths[0].source
    else {
        unreachable!()
    };
    let uv = emitters.last().unwrap();
    assert_eq!(uv.band, OpticalEmitterBand::Ultraviolet);
    assert_eq!(
        (uv.binding.channel_id, uv.binding.function_id),
        (channel.id, channel.functions[0].id)
    );
    assert_eq!(uv.native_reversed, reversed);
    assert!(
        uv.xyz.is_none() && uv.spectrum.is_empty(),
        "unknown UV spectrum"
    );
    profile
}

/// Native word for `amount`, honoring the emitter direction.
fn native_word(width: ChannelResolution, case: (f32, u32, u32), reversed: bool) -> u32 {
    let offset = word(width, case);
    if reversed {
        width.max_raw() - offset
    } else {
        offset
    }
}

/// Installed calibration scaling one emitter of head 0 (a copy of the TL-557 destination rig's
/// helper, which is private to its module).
fn gain(profile: &FixtureProfile, emitter: usize, gain: f32) -> InstalledColorCalibration {
    let mode = &profile.modes[0];
    let path = &mode.color_physical.as_ref().unwrap().paths[0];
    let OpticalSource::Additive { emitters } = &path.source else {
        unreachable!()
    };
    InstalledColorCalibration {
        version: 1,
        revision: 1,
        paths: vec![InstalledColorPathCalibration {
            source_identity: profile
                .native_color_identity(mode.id, path.head_id)
                .unwrap(),
            emitters: vec![InstalledEmitterCalibration {
                emitter_id: emitters[emitter].id,
                output_gain: gain,
                provenance: OpticalProvenance {
                    quality: PhysicalDataQuality::Estimated,
                    ..Default::default()
                },
            }],
            measurements: vec![],
        }],
    }
}

fn rig(
    profile: &FixtureProfile,
    base: &ColorIntent,
    lane: DynamicDefinition,
) -> (Live, PhysicalAdapterLane<ColorAdapter>) {
    let target = FixtureId::new();
    let live = Live::start(target, base, lane);
    live.install_fixtures(vec![patched(profile, target, 1)]);
    (live, PhysicalAdapterLane::live(ColorAdapter::default()))
}

fn uv_lane(amount: f32) -> DynamicDefinition {
    color_lane("UV", ColorComponent::Uv, amount)
}

fn with_uv(mut base: ColorIntent, amount: f32) -> ColorIntent {
    base.uv = light_core::programming::UvIntent { amount };
    base
}

/// Capture and run one frame; check the accepted token chain and the unchanged request.
fn frame(
    live: &mut Live,
    lane: &PhysicalAdapterLane<ColorAdapter>,
) -> (PreparedOutputFrame, PublishedPhysicalFrame<ColorAdapter>) {
    let capture = live.capture();
    let published = live.run(&capture, lane).unwrap();
    assert!(published.requirements.is_empty());
    assert_eq!(published.token, capture.frame_token());
    assert_eq!(
        lane.last_accepted(),
        Some(capture.frame_token()),
        "the lane accepted exactly this captured frame"
    );
    assert_eq!(published.results.len(), 1, "one complete Color owner");
    let result = &published.results[0];
    assert_eq!(
        (result.target, result.owner),
        (live.target, ProgrammingOwner::Color)
    );
    assert_eq!(
        result.token, published.token,
        "writes and request share one token"
    );
    assert_eq!(&result.requested, semantic(&result.value));
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(live.target, &ProgrammingOwner::Color.key()),
        Some(&result.value),
        "the finalizer received the composed value the sidecar describes"
    );
    (capture, published)
}

/// The programmed Color value is still exactly `base`: neither the Dynamic value nor any
/// achieved/quantized output is written back.
fn assert_programmed(live: &Live, base: &ColorIntent) {
    let state = live.programmers.get(live.session).unwrap();
    let programmed: Vec<_> = state
        .values
        .iter()
        .filter(|v| v.fixture_id == live.target && v.attribute == ProgrammingOwner::Color.key())
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(programmed, [program(base)]);
}

/// Writes of one destination: exactly the four Color controls, in order, never Intensity.
fn destination_writes(
    result: &PhysicalHeadResult<ColorAdapter>,
    destination: FixtureId,
    profile: &FixtureProfile,
) -> Vec<NativeControlWrite> {
    let writes: Vec<_> = result
        .writes
        .iter()
        .filter(|w| w.slot.destination == destination)
        .copied()
        .collect();
    assert_eq!(
        writes
            .iter()
            .map(|w| w.slot.channel_index)
            .collect::<Vec<_>>(),
        COLOR_CONTROLS,
        "complete native Color footprint of {destination:?}, each control once"
    );
    for write in &writes {
        let channel = &profile.modes[0].channels[write.slot.channel_index as usize];
        assert_eq!(write.channel_id, channel.id);
        assert!(write.raw <= channel.resolution.max_raw());
        if let Some(function) = write.function_id {
            assert_eq!(function, channel.functions[0].id);
        }
    }
    writes
}

fn uv_write(writes: &[NativeControlWrite]) -> NativeControlWrite {
    *writes.last().unwrap()
}

fn visible(writes: &[NativeControlWrite]) -> Vec<u32> {
    writes[..3].iter().map(|w| w.raw).collect()
}

/// Big-endian bytes of a UV word in its DMX slots (coarse first, fine bytes following).
fn be_bytes(width: ChannelResolution, word: u32) -> Vec<u8> {
    word.to_be_bytes()[4 - width.bytes()..].to_vec()
}

/// Forward-evaluate the captured pre-master native values of the root with `destination`'s
/// sidecar writes applied, through an independently compiled model of that destination
/// (`calibration` = its installed calibration). The vector is then encoded by the profile's
/// encoding plan into a DMX frame (test-supplied native input: production does not inject
/// sidecar writes yet), decoded and evaluated again. Returns the forward result.
fn forward(
    live: &Live,
    capture: &PreparedOutputFrame,
    profile: &FixtureProfile,
    calibration: Option<&InstalledColorCalibration>,
    writes: &[NativeControlWrite],
) -> ColorForwardResult {
    let token = capture.frame_token();
    let baseline = live.engine.prepare_static_family_frame(capture, &[]);
    let mut raw = baseline
        .native_raw(capture, &token, live.target)
        .unwrap()
        .raw()
        .to_vec();
    for write in writes {
        raw[write.slot.channel_index as usize] = write.raw;
    }
    let mode = &profile.modes[0];
    let fitting = CompiledColorFitting::compile(profile, mode.id, calibration)
        .unwrap()
        .unwrap();
    let mut direct = fitting.forward().create_output();
    fitting.forward().evaluate(&raw, &mut direct).unwrap();

    // Test-supplied native input: encode, check the UV slots byte-for-byte, decode, evaluate.
    let plan = mode.compile_encoding_plan().unwrap();
    let mut bytes = [0u8; 512];
    let values: Vec<_> = (0u32..).zip(raw.iter().copied()).collect();
    plan.encode_split_by_index(&mut bytes, 1, 1, &values)
        .unwrap();
    let channel = &mode.channels[UV as usize];
    let coarse = usize::from(
        mode.channels[..UV as usize]
            .iter()
            .map(|c| c.resolution.bytes() as u16)
            .sum::<u16>(),
    );
    assert_eq!(
        bytes[coarse..coarse + channel.resolution.bytes()],
        be_bytes(channel.resolution, raw[UV as usize])[..],
        "every byte of the UV word reaches its own DMX slot"
    );
    let mut slot = 0usize;
    let decoded: Vec<u32> = mode
        .channels
        .iter()
        .map(|c| {
            let value = bytes[slot..slot + c.resolution.bytes()]
                .iter()
                .fold(0u32, |raw, &byte| (raw << 8) | u32::from(byte));
            slot += c.resolution.bytes();
            value
        })
        .collect();
    assert_eq!(decoded, raw, "DMX bytes decode to the written native words");
    let mut encoded = fitting.forward().create_output();
    fitting.forward().evaluate(&decoded, &mut encoded).unwrap();
    assert_eq!(encoded, direct);
    direct.swap_remove(0)
}

/// Published achieved/status of one destination equals the forward evaluation.
fn assert_achieved(outcome: &AchievedColor, quality: &ColorQuality, forward: &ColorForwardResult) {
    assert_eq!(outcome.known_xyz, forward.known_xyz);
    assert_eq!(
        outcome.visible,
        forward.visible_complete.then_some(forward.known_xyz)
    );
    let applied = quality.uv == UvFitStatus::Applied;
    assert_eq!(
        outcome.uv_drive,
        forward.portable_uv.map(|uv| uv.amount).filter(|_| applied)
    );
}

/// AC1/AC2: U24/U32 UV at 0, interior and 1, with simultaneous visible output (purple) and
/// UV-only zero-Y (black, relativeOutput 0), for normal and reversed UV controls.
#[test]
fn u24_and_u32_uv_words_use_full_resolution_with_visible_output_and_uv_only() {
    let purple = intent([1., 0., 1.], 0.);
    for width in WIDTHS {
        for reversed in [false, true] {
            let profile = rgb_uv(width, reversed);
            for (base, uv_only) in [(purple.clone(), false), (uv_only_black(), true)] {
                let mut reference_visible = None;
                for case in CASES {
                    let (amount, ..) = case;
                    let context =
                        format!("{width:?} reversed={reversed} uv_only={uv_only} {amount}");
                    let (mut live, lane) = rig(&profile, &base, uv_lane(amount));
                    let (capture, published) = frame(&mut live, &lane);
                    let result = &published.results[0];
                    let composed = semantic(&result.value);
                    assert_eq!(composed, &with_uv(base.clone(), amount), "{context}");

                    let writes = destination_writes(result, live.target, &profile);
                    assert_eq!(result.writes.len(), 4, "{context}");
                    let uv = uv_write(&writes);
                    let expected = native_word(width, case, reversed);
                    assert_eq!(uv.raw, expected, "{context}: exact full-width UV word");
                    assert_eq!(uv.parked, amount == 0., "{context}: only UV 0 is parked");
                    if amount > 0. {
                        assert_eq!(
                            uv.function_id,
                            Some(profile.modes[0].channels[UV as usize].functions[0].id),
                            "{context}: the UV write keeps its native function"
                        );
                    }
                    if (0. ..1.).contains(&amount) && amount > 0. {
                        assert_ne!(word(width, case) & 0xff, 0, "meaningful interior word");
                    }

                    // Visible output never depends on UV; UV-only leaves visible emitters off.
                    let drives = visible(&writes);
                    if uv_only {
                        assert_eq!(drives, [0, 0, 0], "{context}: no visible leakage");
                    } else {
                        assert_eq!(drives, [255, 0, 255], "{context}: purple is fitted");
                    }
                    let reference = reference_visible.get_or_insert_with(|| drives.clone());
                    assert_eq!(&drives, reference, "{context}: UV is never borrowed");

                    let quality = &result.quality;
                    assert_eq!(quality.uv, UvFitStatus::Applied, "{context}");
                    assert!(!quality.uv_clipped, "{context}");
                    assert_eq!(quality.uv_appearance_known, amount == 0., "{context}");
                    if amount > 0. {
                        assert_eq!(quality.visible, VisibleFitStatus::PredictionIncomplete);
                        assert_eq!(quality.total_quality, PhysicalDataQuality::Unknown);
                        assert!(result.achieved.visible.is_none(), "unknown is not black");
                    } else {
                        assert_eq!(quality.color_match, ColorMatch::Exact, "{context}");
                        assert!(result.achieved.visible.is_some(), "{context}");
                    }
                    let max = f64::from(width.max_raw());
                    assert_eq!(
                        result.achieved.uv_drive,
                        Some(f64::from(word(width, case)) / max),
                        "{context}: achieved UV is the quantized drive, not the request"
                    );
                    let forward = forward(&live, &capture, &profile, None, &writes);
                    assert_achieved(&result.achieved, quality, &forward);
                    if uv_only {
                        assert_eq!(forward.known_xyz.y, 0., "{context}: zero visible Y");
                    }
                    assert_eq!(quality.heads.len(), 1);
                    assert_eq!(quality.heads[0].achieved, result.achieved);
                    assert_programmed(&live, &base);
                }
            }
        }
    }
}

/// AC1: an explicit UV 0 clears a previously nonzero full-width word on the same lane, through
/// an interior word, while visible output stays fitted and continuity records the clear.
#[test]
fn explicit_zero_uv_clears_a_previously_nonzero_full_width_word() {
    let purple = intent([1., 0., 1.], 0.);
    for width in WIDTHS {
        let profile = rgb_uv(width, false);
        let first = with_uv(purple.clone(), 1.);
        let (mut live, lane) = rig(
            &profile,
            &first,
            color_lane("Relative output", ColorComponent::RelativeOutput, 1.),
        );
        let mut tokens = vec![];
        for (index, case) in [CASES[3], CASES[1], CASES[0]].into_iter().enumerate() {
            let base = with_uv(purple.clone(), case.0);
            if index > 0 {
                live.programmers.set(
                    live.session,
                    live.target,
                    ProgrammingOwner::Color.key(),
                    program(&base),
                );
            }
            let (capture, published) = frame(&mut live, &lane);
            tokens.push(capture.frame_token());
            let result = &published.results[0];
            assert_eq!(semantic(&result.value), &base, "{width:?} step {index}");
            let writes = destination_writes(result, live.target, &profile);
            let uv = uv_write(&writes);
            assert_eq!(uv.raw, word(width, case), "{width:?} step {index}");
            assert_eq!(visible(&writes), [255, 0, 255]);
            let continuity = lane
                .continuity(live.target, ProgrammingOwner::Color)
                .unwrap();
            let recorded = continuity.heads[0]
                .controls
                .iter()
                .find(|(index, ..)| *index == UV)
                .unwrap();
            assert_eq!(recorded.2, uv.raw, "continuity holds the accepted word");
            let forward = forward(&live, &capture, &profile, None, &writes);
            assert_achieved(&result.achieved, &result.quality, &forward);
            assert_programmed(&live, &base);
        }
        let last = lane
            .continuity(live.target, ProgrammingOwner::Color)
            .unwrap();
        assert_eq!(
            last.heads[0]
                .controls
                .iter()
                .find(|(i, ..)| *i == UV)
                .unwrap()
                .2,
            0,
            "the explicit zero cleared the previously nonzero word"
        );
        tokens.dedup();
        assert_eq!(tokens.len(), 3, "one distinct accepted token per frame");
    }
}

/// AC2: relativeOutput (here a Dynamic lane over the programmed base) scales visible output
/// only; at 0 the visible emitters are off and the UV word is unchanged.
#[test]
fn relative_output_never_attenuates_full_width_uv() {
    for width in WIDTHS {
        let profile = rgb_uv(width, false);
        let base = with_uv(intent([1., 0., 1.], 0.), CASES[1].0);
        let mut drives = vec![];
        for level in [1., 0.25, 0.] {
            let (mut live, lane) = rig(
                &profile,
                &base,
                color_lane("Relative output", ColorComponent::RelativeOutput, level),
            );
            let (capture, published) = frame(&mut live, &lane);
            let result = &published.results[0];
            let composed = semantic(&result.value);
            assert_eq!(composed.relative_output, level);
            assert_eq!(composed.uv, base.uv);
            let writes = destination_writes(result, live.target, &profile);
            assert_eq!(
                uv_write(&writes).raw,
                word(width, CASES[1]),
                "{width:?} relativeOutput {level} never scales UV"
            );
            let forward = forward(&live, &capture, &profile, None, &writes);
            assert_achieved(&result.achieved, &result.quality, &forward);
            assert_programmed(&live, &base);
            drives.push(visible(&writes));
        }
        assert_eq!(drives[0], [255, 0, 255]);
        assert!(
            drives[1][0] < drives[0][0] && drives[1][2] < drives[0][2] && drives[1][0] > 0,
            "{width:?}: visible is attenuated {drives:?}"
        );
        assert_eq!(
            drives[2],
            [0, 0, 0],
            "{width:?}: zero output leaves UV only"
        );
    }
}

/// AC3: a multipatch copy with its own visible calibration gets its own visible fit and the
/// same independent full-width UV word, under the exact captured token, with complete native
/// footprints on both destinations and the programmed request unchanged.
#[test]
fn a_calibrated_multipatch_copy_fits_its_own_visible_and_keeps_independent_full_width_uv() {
    for width in WIDTHS {
        let profile = rgb_uv(width, false);
        let calibration = gain(&profile, 0, 0.5);
        let target = FixtureId::new();
        let mut fixture = patched(&profile, target, 1);
        let copy = light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            universe: Some(1),
            address: Some(20),
            color_calibration: Some(calibration.clone()),
            ..Default::default()
        };
        let copy_id = FixtureId(copy.id);
        fixture.multipatch = vec![copy];
        let mut base = intent([1., 0., 1.], 0.);
        base.relative_output = 0.3;
        let mut live = Live::start(target, &base, uv_lane(CASES[1].0));
        live.install_fixtures(vec![fixture]);
        let lane = PhysicalAdapterLane::live(ColorAdapter::default());
        for _ in 0..2 {
            let (capture, published) = frame(&mut live, &lane);
            let result = &published.results[0];
            assert_eq!(semantic(&result.value), &with_uv(base.clone(), CASES[1].0));
            assert_eq!(result.writes.len(), 8, "both complete native footprints");
            let root = destination_writes(result, target, &profile);
            let copied = destination_writes(result, copy_id, &profile);
            assert_eq!(
                result
                    .writes
                    .iter()
                    .map(|w| w.slot.destination)
                    .collect::<Vec<_>>(),
                [[target; 4], [copy_id; 4]].concat(),
                "root first, then the copy"
            );
            for writes in [&root, &copied] {
                let uv = uv_write(writes);
                assert_eq!(uv.raw, word(width, CASES[1]), "{width:?}: independent UV");
                assert!(!uv.parked);
            }
            let (r, c) = (visible(&root), visible(&copied));
            assert!(
                c[0] > r[0],
                "{width:?}: the dimmer copy red is driven harder: root {r:?}, copy {c:?}"
            );
            assert_eq!(r[1], 0);
            assert_eq!(c[1], 0);

            let heads = &result.quality.heads;
            assert_eq!(
                heads.iter().map(|h| h.destination).collect::<Vec<_>>(),
                [target, copy_id]
            );
            for (head, writes, calibration) in [
                (&heads[0], &root, None),
                (&heads[1], &copied, Some(&calibration)),
            ] {
                assert_eq!(head.quality.uv, UvFitStatus::Applied);
                assert!(!head.quality.uv_appearance_known);
                assert_eq!(
                    head.achieved.uv_drive,
                    Some(f64::from(word(width, CASES[1])) / f64::from(width.max_raw()))
                );
                let forward = forward(&live, &capture, &profile, calibration, writes);
                assert_achieved(&head.achieved, &head.quality, &forward);
            }
            let [root_xyz, copy_xyz] = [0, 1].map(|i| heads[i].achieved.known_xyz);
            for (a, b) in [
                (root_xyz.x, copy_xyz.x),
                (root_xyz.y, copy_xyz.y),
                (root_xyz.z, copy_xyz.z),
            ] {
                assert!((a - b).abs() < 2e-3, "{root_xyz:?} vs {copy_xyz:?}");
            }
            // Negative control: ignoring the copy's calibration misreads its visible output.
            let uncalibrated = forward(&live, &capture, &profile, None, &copied);
            assert!(uncalibrated.known_xyz.x > copy_xyz.x * 1.2);
            assert_eq!(
                uncalibrated.portable_uv.map(|uv| uv.amount),
                heads[1].achieved.uv_drive,
                "UV drive does not depend on the visible calibration"
            );
            assert_programmed(&live, &base);
        }
        let counters = lane.adapter().counters();
        assert_eq!(
            (counters.fitting_compiles, counters.copy_destinations),
            (2, 1)
        );
    }
}
