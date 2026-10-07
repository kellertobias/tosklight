//! TL-629: live-Group Focus/Zoom Cues across optical replacement, through the actual writer,
//! SQLite reopen, show-open compiler, Playback GO and the Focus/Zoom physical adapters.
//!
//! The Programmer records normalized Focus plus Field-convention Zoom degrees against a live
//! Group through `ProgrammingService::handle_cue_recording` and the `ActiveShowService` Cue
//! writer (the parent module's harness). Every step reopens the SQLite file, compiles it with
//! `prepare_show_candidate`, installs the snapshot into an engine, plays the Cue with
//! `EnginePlaybackCommand`, and resolves the played owners with one `OpticsAdapter::pair` against
//! one captured scalar/geometry/native frame. Each result is encoded into DMX at the fixture's
//! own address, decoded, and re-evaluated through an independently compiled
//! `CompiledOpticsForward` and through a direct interpolation of the profile's authored curve.
//!
//! The stored Cue list body never changes. Replacement profiles keep every channel/function UUID
//! (a profile revision of the same lamp) so obsolete continuity could match by identity; only the
//! response digest and the captured baseline witnesses keep it from being replayed.
//!
//! This is synthetic adapter/fitter evidence. It claims no live cutover, no production producer
//! and no physical calibration; production `SUPPORTED_PROGRAMMING_CONTRACT` stays 0 (the engine
//! here opts in explicitly, as the parent harness does).
use super::super::super::super::optics::profiles::{spot_b, wash_a};
use super::super::super::super::optics::tests::{field, focus};
use super::super::super::super::optics::{
    OpticsAdapter, OpticsContinuity, OpticsDescriptor, OpticsRequested,
};
use super::*;
use light_core::OpeningConvention;
use light_core::programming::ScalarIntent;
use light_fixture::forward::{CompiledOpticsForward, OpticsForwardStatus};
use light_fixture::{
    ChannelFunctionBehavior, OpticsFamily, OpticsFitStatus, PhysicalDataQuality,
    PhysicalMappingPoint,
};

/// Channel layout shared by every optics builder profile: 0 Intensity, 1 Zoom, 2 Focus.
const ZOOM: usize = 1;
const FOCUS: usize = 2;
const CUE_FULL: f64 = 1.0;
const CUE_FOCUS_ONLY: f64 = 2.0;

/// The stored intents. Cue 2 carries Focus only.
fn cue1_focus() -> AttributeValue {
    focus(0.37)
}
fn cue1_zoom() -> AttributeValue {
    field(20.)
}
fn cue2_focus() -> AttributeValue {
    focus(0.8)
}

/// Replace `profile`'s function curve on `channel` while keeping every native UUID.
fn recurve(
    profile: &mut FixtureProfile,
    channel: usize,
    (from, to): (u32, u32),
    unit: &str,
    points: &[(u32, f32)],
    convention: Option<OpeningConvention>,
) {
    let function = &mut profile.modes[0].channels[channel].functions[0];
    function.dmx_from = from;
    function.dmx_to = to;
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: points[0].1,
        physical_max: points[points.len() - 1].1,
        unit: Some(unit.into()),
    };
    let mapping = function.physical_mapping.as_mut().expect("authored curve");
    mapping.quality = PhysicalDataQuality::Measured;
    mapping.opening_convention = convention;
    mapping.samples = points
        .iter()
        .map(|&(raw, physical)| PhysicalMappingPoint { raw, physical })
        .collect();
}

/// Revision 2 of Wash A, every UUID unchanged: Zoom becomes ascending (Wash A is reversed) and
/// nonlinear on 4096..=61440 (6° → 30° → 48°), Focus ascending nonlinear on 40..=240
/// (0 % → 50 % → 100 %; Wash A Focus is reversed linear on 10..=200).
fn wash_a_rev2(original: &FixtureProfile) -> FixtureProfile {
    let mut profile = original.clone();
    profile.revision = 2;
    recurve(
        &mut profile,
        ZOOM,
        (4096, 61440),
        "deg",
        &[(4096, 6.), (16384, 30.), (61440, 48.)],
        Some(OpeningConvention::Field),
    );
    recurve(
        &mut profile,
        FOCUS,
        (40, 240),
        "%",
        &[(40, 0.), (90, 50.), (240, 100.)],
        None,
    );
    profile.validate().unwrap();
    profile
}

/// Revision 3: revision 2 recalibrated as a Beam opening. Same UUIDs, same native ranges.
fn wash_a_rev3_beam(rev2: &FixtureProfile) -> FixtureProfile {
    let mut profile = rev2.clone();
    profile.revision = 3;
    profile.modes[0].channels[ZOOM].functions[0]
        .physical_mapping
        .as_mut()
        .unwrap()
        .opening_convention = Some(OpeningConvention::Beam);
    profile.validate().unwrap();
    profile
}

/// The authored curve of `channel`'s first function evaluated at `raw` by plain piecewise-linear
/// interpolation of its samples (or of its endpoints), without any compiled model. Focus is
/// returned normalized, Zoom in degrees. None outside the function or without a unit.
fn authored(profile: &FixtureProfile, channel: usize, raw: u32) -> Option<f64> {
    let function = &profile.modes[0].channels[channel].functions[0];
    if !(function.dmx_from..=function.dmx_to).contains(&raw) {
        return None;
    }
    let ChannelFunctionBehavior::Continuous {
        physical_min,
        physical_max,
        unit: Some(unit),
    } = &function.behavior
    else {
        return None;
    };
    let samples = function
        .physical_mapping
        .as_ref()
        .map(|m| m.samples.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            vec![
                PhysicalMappingPoint {
                    raw: function.dmx_from,
                    physical: *physical_min,
                },
                PhysicalMappingPoint {
                    raw: function.dmx_to,
                    physical: *physical_max,
                },
            ]
        });
    let pair = samples
        .windows(2)
        .find(|w| (w[0].raw..=w[1].raw).contains(&raw))
        .unwrap();
    let t = f64::from(raw - pair[0].raw) / f64::from(pair[1].raw - pair[0].raw);
    let physical = f64::from(pair[0].physical)
        + t * (f64::from(pair[1].physical) - f64::from(pair[0].physical));
    Some(if unit == "%" {
        physical / 100.
    } else {
        physical
    })
}

/// The largest physical change of one raw step around `raw` on the authored curve; one step of
/// native travel for a nominal (unit-less) function.
fn step(profile: &FixtureProfile, channel: usize, raw: u32) -> f64 {
    let Some(here) = authored(profile, channel, raw) else {
        let function = &profile.modes[0].channels[channel].functions[0];
        return 1. / f64::from(function.dmx_to - function.dmx_from);
    };
    [raw.saturating_sub(1), raw + 1]
        .into_iter()
        .filter_map(|r| authored(profile, channel, r))
        .map(|v| (v - here).abs())
        .fold(0., f64::max)
}

/// One family's played owner and its resolution.
struct Family {
    value: AttributeValue,
    descriptor: OpticsDescriptor,
    result: PhysicalResolution<OpticsAdapter>,
}

impl Family {
    fn raw(&self) -> u32 {
        let [write] = self.result.writes.as_slice() else {
            panic!("exactly the owning native control is written")
        };
        write.raw
    }
}

/// One fixture's Focus and Zoom resolved from one captured frame.
struct Frame {
    target: FixtureId,
    /// Captured pre-master native baseline of the whole mode.
    native: Vec<u32>,
    focus: Family,
    zoom: Family,
}

/// The Focus/Zoom pair sharing one fitter cache, as the Live lanes construct it.
struct Optics {
    focus: OpticsAdapter,
    zoom: OpticsAdapter,
}

impl Optics {
    fn new() -> Self {
        let (focus, zoom) = OpticsAdapter::pair();
        assert_eq!(
            (focus.family(), zoom.family()),
            (OpticsFamily::Focus, OpticsFamily::Zoom)
        );
        Self { focus, zoom }
    }

    /// Resolve both played owners of `target` against one capture, with optional previous
    /// accepted continuity per family.
    fn frame(
        &self,
        output: &Output,
        target: FixtureId,
        previous: (Option<&OpticsContinuity>, Option<&OpticsContinuity>),
    ) -> Frame {
        output.clock.advance_millis(25);
        let capture = output.engine.prepare_output_frame(RenderOptions::default());
        let token = capture.frame_token();
        let mut scalar = output.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = output
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let snapshot = capture.snapshot();
        let native = scalar
            .native_raw(&capture, &token, target)
            .unwrap()
            .raw()
            .to_vec();
        let resolved = output.engine.resolved_values();
        let family = |adapter: &OpticsAdapter, previous: Option<&OpticsContinuity>| {
            let owner = match adapter.family() {
                OpticsFamily::Focus => ProgrammingOwner::Focus,
                OpticsFamily::Zoom => ProgrammingOwner::Zoom,
            };
            let key = owner.key();
            let value = scalar
                .value(target, &key)
                .cloned()
                .unwrap_or_else(|| panic!("the played Cue owns {owner:?}"));
            assert_eq!(
                resolved.get(&(target, key)),
                Some(&value),
                "the captured scalar value is the played owner"
            );
            let descriptor = adapter
                .compile(&snapshot, target)
                .unwrap()
                .expect("optics destination");
            let result = adapter
                .resolve(PhysicalRequest {
                    frame: HybridFrameContext {
                        capture: &capture,
                        geometry: &geometry,
                        native_models: snapshot.native_color_sources.as_ref(),
                        token: &token,
                        scalar: &scalar,
                    },
                    target,
                    owner,
                    descriptor: &descriptor,
                    value: &value,
                    previous,
                })
                .unwrap();
            Family {
                value,
                descriptor,
                result,
            }
        };
        let focus = family(&self.focus, previous.0);
        let zoom = family(&self.zoom, previous.1);
        Frame {
            target,
            native,
            focus,
            zoom,
        }
    }
}

/// Decode one fixture's DMX bytes at `address` with the builder's sequential layout.
fn decode(mode: &light_fixture::FixtureMode, address: u16, bytes: &[u8; 512]) -> Vec<u32> {
    let base = usize::from(address) - 1;
    let mut slot = 1usize;
    mode.channels
        .iter()
        .map(|channel| {
            let mut raw = u32::from(bytes[base + slot - 1]);
            for secondary in &channel.secondary_slots {
                raw = (raw << 8) | u32::from(bytes[base + usize::from(*secondary) - 1]);
            }
            slot += channel.resolution.bytes();
            raw
        })
        .collect()
}

impl Frame {
    /// Complete evidence for both families of one fixture: footprints are the actual owning
    /// native controls, requests are the exact stored intents, the combined native output
    /// encodes/decodes at full resolution, and the decoded bytes re-simulate `achieved` through
    /// both the compiled forward model and the authored curve. Returns the decoded output.
    fn verify(&self, profile: &FixtureProfile, address: u16) -> Vec<u32> {
        let mode = &profile.modes[0];
        let mut output = self.native.clone();
        for (family, channel) in [(&self.focus, FOCUS), (&self.zoom, ZOOM)] {
            let (descriptor, result) = (&family.descriptor, &family.result);
            let slot = NativeControlSlot {
                destination: self.target,
                channel_index: channel as u32,
                split: 1,
            };
            assert_eq!(*descriptor.footprint, [slot], "the actual owning control");
            assert_eq!(descriptor.destination, self.target);
            validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
            let [write] = result.writes.as_slice() else {
                unreachable!()
            };
            assert_eq!(write.channel_id, mode.channels[channel].id);
            let expected = match (descriptor.family, &family.value) {
                (OpticsFamily::Focus, AttributeValue::Normalized(v)) => OpticsRequested {
                    family: OpticsFamily::Focus,
                    value: f64::from(*v),
                    convention: None,
                },
                (OpticsFamily::Zoom, AttributeValue::Zoom(z)) => OpticsRequested {
                    family: OpticsFamily::Zoom,
                    value: match z.opening_degrees {
                        ScalarIntent::Value(d) => f64::from(d),
                        ScalarIntent::Spread(_) => panic!("materialized per target"),
                    },
                    convention: Some(z.convention),
                },
                other => panic!("unexpected played value {other:?}"),
            };
            assert_eq!(
                result.requested, expected,
                "the request is the stored intent"
            );
            output[channel] = write.raw;
        }
        let plan = mode.compile_encoding_plan().unwrap();
        let mut bytes = [0u8; 512];
        let values: Vec<_> = (0u32..).zip(output.iter().copied()).collect();
        plan.encode_split_by_index(&mut bytes, address, 1, &values)
            .unwrap();
        let decoded = decode(mode, address, &bytes);
        assert_eq!(decoded, output, "full-resolution encode/decode");
        let model = CompiledOpticsForward::compile(mode).unwrap();
        let mut forward = model.create_output();
        model.evaluate(&decoded, &mut forward).unwrap();
        let head = &forward[self.zoom.descriptor.head];
        // Zoom: compiled forward, authored curve and the published achieved value agree.
        let zoom = &self.zoom.result;
        match (head.zoom_status, head.zoom) {
            (OpticsForwardStatus::Resolved, Some(z)) => {
                assert_eq!(zoom.achieved, Some(z.degrees));
                assert_eq!(zoom.quality.function_id, Some(z.function_id));
                assert_eq!(zoom.quality.convention, z.convention);
                let reference = authored(profile, ZOOM, decoded[ZOOM]).unwrap();
                assert!((reference - z.degrees).abs() < 1e-6, "authored Zoom curve");
            }
            _ => assert_eq!(zoom.achieved, None, "unknown, never zero"),
        }
        let focus = &self.focus.result;
        match (head.focus_status, head.focus) {
            (OpticsForwardStatus::Resolved, Some(f)) => {
                assert_eq!(focus.achieved, Some(f.percent / 100.));
                assert_eq!(focus.quality.function_id, Some(f.function_id));
                assert_eq!(focus.quality.nominal, f.nominal);
                if !f.nominal {
                    let reference = authored(profile, FOCUS, decoded[FOCUS]).unwrap();
                    assert!(
                        (reference - f.percent / 100.).abs() < 1e-6,
                        "authored Focus curve"
                    );
                }
            }
            _ => assert_eq!(focus.achieved, None, "unknown, never zero"),
        }
        decoded
    }

    /// Both families are fitted to their stored intent within half a raw step of the curve.
    fn assert_fitted(&self, profile: &FixtureProfile) {
        for (family, channel) in [(&self.focus, FOCUS), (&self.zoom, ZOOM)] {
            let quality = family.result.quality;
            assert_eq!(quality.status, OpticsFitStatus::Fitted, "{channel}");
            assert!(!quality.held && !quality.clipped);
            assert!(!family.result.writes[0].parked);
            let raw = family.raw();
            let achieved = family.result.achieved.expect("known achieved value");
            let requested = family.result.requested.value;
            let tolerance = 0.5 * step(profile, channel, raw) + 1e-9;
            assert!(
                (achieved - requested).abs() <= tolerance,
                "{}: channel {channel} {requested} -> {achieved} at raw {raw}",
                profile.name
            );
        }
    }
}

/// The group changes of one stored Cue: `(attribute, value)` in stored order.
fn group_changes(body: &Value, cue: usize) -> Vec<(String, AttributeValue)> {
    body.pointer(&format!("/cues/{cue}/group_changes"))
        .and_then(Value::as_array)
        .expect("group changes")
        .iter()
        .map(|change| {
            assert_eq!(change["group_id"], GROUP, "stored against the live Group");
            (
                change["attribute"].as_str().unwrap().to_owned(),
                serde_json::from_value(change["value"].clone()).unwrap(),
            )
        })
        .collect()
}

fn pool_go(output: &Output, action: PoolPlaybackAction) {
    output
        .engine
        .execute_playback(EnginePlaybackCommand::Pool {
            number: PLAYBACK,
            action,
        })
        .unwrap();
    // Well past any default Cue fade: the played state is the settled Cue.
    output.clock.advance_millis(600_000);
}

/// Reopen and compile, install, GoTo Cue 1 then GO to the Focus-only Cue 2.
fn play(show: &Show, output: &Output, cue: f64) {
    output.engine.replace_snapshot(show.compile()).unwrap();
    pool_go(
        output,
        PoolPlaybackAction::GoTo(CueNumber::try_from_legacy_f64(CUE_FULL).unwrap()),
    );
    if cue == CUE_FOCUS_ONLY {
        pool_go(output, PoolPlaybackAction::Go);
    }
}

#[test]
fn group_focus_zoom_cues_survive_reopen_replacement_group_growth_and_convention_mismatch() {
    let show = Show::new();
    let (a, b) = (FixtureId::new(), FixtureId::new());
    let original = wash_a();
    let rev2 = wash_a_rev2(&original);
    let beam = wash_a_rev3_beam(&rev2);
    let spot = spot_b();
    show.patch(&fixture(&original, a, 1, 1));
    show.group(&[a]);

    // Record through the actual Programmer and ActiveShow Cue writer.
    let (focus_key, zoom_key) = (ProgrammingOwner::Focus.key(), ProgrammingOwner::Zoom.key());
    show.programmers
        .set_group(show.session, GROUP.into(), focus_key.clone(), cue1_focus());
    show.programmers
        .set_group(show.session, GROUP.into(), zoom_key.clone(), cue1_zoom());
    show.record(CUE_FULL);
    show.programmers
        .set_group(show.session, GROUP.into(), focus_key.clone(), cue2_focus());
    show.record(CUE_FOCUS_ONLY);
    let (list_id, recorded) = show.cue_list();
    assert_eq!(recorded["cues"].as_array().unwrap().len(), 2);
    assert_eq!(
        group_changes(&recorded, 0),
        [
            (focus_key.0.to_string(), cue1_focus()),
            (zoom_key.0.to_string(), cue1_zoom()),
        ],
        "exact normalized Focus and Field-convention Zoom degrees"
    );
    assert_eq!(
        group_changes(&recorded, 1),
        [(focus_key.0.to_string(), cue2_focus())],
        "Cue 2 stores Focus only"
    );
    for cue in [0, 1] {
        assert!(
            recorded
                .pointer(&format!("/cues/{cue}/changes"))
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty),
            "no per-member expansion"
        );
    }
    let unchanged = |step: &str| {
        assert_eq!(
            show.cue_list(),
            (list_id.clone(), recorded.clone()),
            "{step}: the stored Cue list is byte-for-byte unchanged"
        );
        let snapshot = show.compile();
        assert_eq!(snapshot.cue_lists.len(), 1);
        assert_eq!(
            snapshot.cue_lists[0].cues.len(),
            2,
            "{step}: no reprogramming"
        );
    };

    let output = Output::new();
    let optics = Optics::new();

    // Step 0: the recorded lamp.
    unchanged("original");
    play(&show, &output, CUE_FULL);
    let original_full = optics.frame(&output, a, (None, None));
    original_full.verify(&original, 1);
    original_full.assert_fitted(&original);
    assert_eq!(original_full.focus.value, cue1_focus());
    assert_eq!(original_full.zoom.value, cue1_zoom());
    assert_eq!(
        original_full.zoom.raw(),
        32768,
        "20° on reversed 50° → 20° → 5°"
    );
    assert_eq!(
        original_full.focus.raw(),
        130,
        "37 % on reversed 100 % → 0 % over 10..=200"
    );
    play(&show, &output, CUE_FOCUS_ONLY);
    let original_focus_only = optics.frame(&output, a, (None, None));
    original_focus_only.verify(&original, 1);
    original_focus_only.assert_fitted(&original);
    assert_eq!(
        original_focus_only.focus.value,
        cue2_focus(),
        "Focus changes"
    );
    assert_eq!(
        original_focus_only.zoom.value,
        cue1_zoom(),
        "independent Zoom remains"
    );
    assert_eq!(original_focus_only.focus.raw(), 48);
    assert_eq!(original_focus_only.zoom.raw(), 32768);

    // Step 1: revision 2 (reversed/nonlinear ranges, same UUIDs) and a new live-Group member of
    // another type (Spot B: ascending U8 Zoom, nominal U16 Focus).
    show.patch(&fixture(&rev2, a, 1, 1));
    show.patch(&fixture(&spot, b, 2, 40));
    show.group(&[a, b]);
    unchanged("replacement and Group growth");
    let compiles = optics.focus.counters().fitting_compiles;
    play(&show, &output, CUE_FULL);
    let stale = optics.zoom.counters().stale_continuity;
    // Wash A's accepted continuity matches by UUID but not by response: never replayed.
    let rev2_full = optics.frame(
        &output,
        a,
        (
            Some(&original_full.focus.result.continuity),
            Some(&original_full.zoom.result.continuity),
        ),
    );
    assert_eq!(
        original_full.zoom.result.continuity.control.unwrap().1,
        rev2_full.zoom.result.continuity.control.unwrap().1,
        "same native channel identity"
    );
    assert_ne!(
        rev2_full.zoom.descriptor.response,
        original_full.zoom.descriptor.response
    );
    assert_eq!(
        optics.zoom.counters().stale_continuity,
        stale + 2,
        "both obsolete anchors are rejected"
    );
    rev2_full.verify(&rev2, 1);
    rev2_full.assert_fitted(&rev2);
    assert_eq!(rev2_full.zoom.value, cue1_zoom());
    assert_eq!(
        rev2_full.zoom.raw(),
        11264,
        "20° on ascending 6° → 30° → 48°"
    );
    assert_eq!(
        rev2_full.focus.raw(),
        77,
        "37 % on ascending 0 % → 50 % → 100 %"
    );
    assert_ne!(rev2_full.zoom.raw(), original_full.zoom.raw());
    assert_ne!(rev2_full.focus.raw(), original_full.focus.raw());
    for raw in [rev2_full.zoom.raw(), rev2_full.focus.raw()] {
        assert_ne!(raw, 0, "no parked zero");
    }
    // No percentage-as-angle: 20° is not 20 % of the Zoom travel.
    assert_ne!(rev2_full.zoom.raw(), (0.2f64 * 65535.).round() as u32);
    let member = optics.frame(&output, b, (None, None));
    member.verify(&spot, 40);
    member.assert_fitted(&spot);
    assert_eq!(
        (member.focus.value.clone(), member.zoom.value.clone()),
        (cue1_focus(), cue1_zoom())
    );
    assert_eq!(member.zoom.raw(), 128, "20° on Spot B's own curve");
    assert_eq!(member.focus.raw(), (0.37f64 * 65535.).round() as u32);
    assert!(
        member.focus.result.quality.nominal,
        "nominal travel, not a focal distance"
    );
    assert!(
        optics.focus.counters().fitting_compiles >= compiles + 2,
        "the replacement and the new member compile their own fitters"
    );
    play(&show, &output, CUE_FOCUS_ONLY);
    for (target, profile, address, zoom_raw) in [(a, &rev2, 1, 11264), (b, &spot, 40, 128)] {
        let frame = optics.frame(&output, target, (None, None));
        frame.verify(profile, address);
        frame.assert_fitted(profile);
        assert_eq!(
            frame.focus.value,
            cue2_focus(),
            "Focus-only Cue changes Focus"
        );
        assert_eq!(frame.zoom.value, cue1_zoom(), "independent Zoom remains");
        assert_eq!(frame.zoom.raw(), zoom_raw);
    }

    // Step 2: the same lamp recalibrated as Beam. The stored Field request is checked, never
    // converted: Zoom holds the captured baseline passively; Focus and the other member fit.
    show.patch(&fixture(&beam, a, 1, 1));
    unchanged("Beam recalibration");
    play(&show, &output, CUE_FULL);
    let held = optics.frame(
        &output,
        a,
        (
            Some(&rev2_full.focus.result.continuity),
            Some(&rev2_full.zoom.result.continuity),
        ),
    );
    held.verify(&beam, 1);
    let quality = held.zoom.result.quality;
    assert_eq!(quality.status, OpticsFitStatus::ConventionMismatch);
    assert!(quality.held && held.zoom.result.writes[0].parked);
    assert_eq!(
        held.zoom.result.requested.convention,
        Some(OpeningConvention::Field)
    );
    assert_eq!(
        held.zoom.result.requested.value, 20.,
        "the stored request is retained"
    );
    // A typed Zoom has no scalar encoding: the captured baseline is the control's default raw,
    // outside every Zoom function of this revision, so the held output is unknown, never 0°.
    assert_eq!(held.native[ZOOM], beam.modes[0].channels[ZOOM].default_raw);
    assert_eq!(held.zoom.result.achieved, None);
    assert_eq!(
        quality.convention, None,
        "no convention is claimed for unknown output"
    );
    assert_eq!(
        held.zoom.raw(),
        held.native[ZOOM],
        "held at the captured baseline, not the obsolete Field anchor"
    );
    assert_ne!(held.zoom.raw(), rev2_full.zoom.raw());
    assert_eq!(held.focus.result.quality.status, OpticsFitStatus::Fitted);
    assert_eq!(
        held.focus.raw(),
        77,
        "Focus is not held by a Zoom limitation"
    );
    let member = optics.frame(&output, b, (None, None));
    member.verify(&spot, 40);
    member.assert_fitted(&spot);

    // Step 3: back to the supported revision 2. The fresh descriptor proves the response; the
    // held Beam continuity is obsolete and is not overlaid; a fresh continuity then chains.
    show.patch(&fixture(&rev2, a, 1, 1));
    unchanged("return to the supported model");
    play(&show, &output, CUE_FULL);
    let stale = optics.zoom.counters().stale_continuity;
    let returned = optics.frame(&output, a, (None, Some(&held.zoom.result.continuity)));
    returned.verify(&rev2, 1);
    returned.assert_fitted(&rev2);
    assert_eq!(
        returned.zoom.descriptor.response,
        rev2_full.zoom.descriptor.response
    );
    assert_ne!(
        returned.zoom.descriptor.response,
        held.zoom.descriptor.response
    );
    assert_eq!(optics.zoom.counters().stale_continuity, stale + 1);
    assert_eq!(
        returned.zoom.raw(),
        11264,
        "fresh fit, same as the first rev 2 proof"
    );
    let chained = optics.frame(
        &output,
        a,
        (
            Some(&returned.focus.result.continuity),
            Some(&returned.zoom.result.continuity),
        ),
    );
    chained.verify(&rev2, 1);
    assert_eq!(
        optics.zoom.counters().stale_continuity,
        stale + 1,
        "compatible fresh continuity is accepted"
    );
    assert_eq!(
        chained.zoom.result.continuity,
        returned.zoom.result.continuity
    );
    assert_eq!(
        chained.focus.result.continuity,
        returned.focus.result.continuity
    );
}
