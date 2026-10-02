//! TL-559 round-3 review regressions: exact native replay does not claim a chromaticity match,
//! native-edit adoption validates its publication on both branches, and a held visible
//! solution reports UV appearance and shared-control conflicts from the final written values.
use super::native::{PublishedColorHead, adopt_native_edit};
use super::profiles::*;
use super::tests::{intent, program};
use super::tests_destinations::two_heads;
use super::tests_direct::{
    DirectRig, direct, direct_program, path_channels, rgbw_widths, unknown_white_uv,
};
use super::*;
use light_core::NativeColorBinding;
use light_core::programming::{ComponentEdit, NativeColorEdit};
use light_fixture::{FixtureProfile, OpticalEmitterBand, OpticalSource};

/// Finding 1: a verified compatible replay whose destination optics differ from the source
/// estimate is Exact as native identity, but its colour match comes from comparing actual
/// forward output with the source estimate.
#[test]
fn exact_native_replay_reports_the_measured_match_not_identity() {
    let source = rgbw_widths();
    let red = [255, 0, 0, 0];
    let same = DirectRig::new(&source, &[&source]);
    let value = direct(&same.catalogue.borrow(), &source, &red);
    let resolved = same.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    assert_eq!(resolved.result.quality.color_match, ColorMatch::Exact);
    assert!(resolved.result.quality.delta_uv.unwrap() < 1e-4);

    let mut recalibrated = source.clone();
    recalibrated.revision = 2;
    let path = &mut recalibrated.modes[0].color_physical.as_mut().unwrap().paths[0];
    if let OpticalSource::Additive { emitters } = &mut path.source {
        emitters[0].xyz = Some(xyz(0.30, 0.20, 0.02));
    }
    recalibrated.validate().unwrap();
    let rig = DirectRig::new(&recalibrated, &[&source, &recalibrated]);
    let value = direct(&rig.catalogue.borrow(), &source, &red);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    let quality = &resolved.result.quality;
    assert_ne!(quality.color_match, ColorMatch::Exact, "{quality:?}");
    assert!(quality.delta_uv.unwrap() > f64::from(light_core::color_intent::EXACT_DELTA_UV));
    assert!(quality.luminance_limited);

    // Unknown source appearance: identity replay, unmeasured match.
    let unknown = unknown_white_uv(xyz(0.02, 0.01, 0.08));
    let rig = DirectRig::new(&unknown, &[&unknown]);
    let value = direct(&rig.catalogue.borrow(), &unknown, &[0, 0, 0, 255, 0]);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    assert_eq!(resolved.result.quality.color_match, ColorMatch::Unknown);
}

fn set_first(profile: &FixtureProfile, raw: u32) -> [ComponentEdit; 1] {
    let channel = path_channels(profile)[0];
    [ComponentEdit::Native {
        binding: NativeColorBinding {
            channel_id: channel.id,
            function_id: channel.functions[0].id,
        },
        operation: NativeColorEdit::Set(raw),
    }]
}

/// Finding 2: both the capture branch (Semantic) and the in-place branch (already Direct of the
/// same source) reject a publication from another capture, and one of another target.
#[test]
fn native_edit_adoption_rejects_foreign_tokens_and_targets_on_both_branches() {
    let profile = rgbwauv(None);
    let rig = DirectRig::new(&profile, &[&profile]);
    let semantic = program(&intent([1., 0., 1.], 0.));
    let existing = direct(&rig.catalogue.borrow(), &profile, &[10, 20, 30, 40, 50, 0]);
    let edits = set_first(&profile, 7);
    for value in [&semantic, &existing] {
        let resolved = rig.resolve(value, None).unwrap();
        let stale = rig.resolve(value, None).unwrap();
        let head = resolved.descriptor.primary().head_id;
        let token = resolved.capture.frame_token();
        let stale_token = stale.capture.frame_token();
        let published = PublishedColorHead {
            token: &token,
            target: rig.target,
            value,
            writes: &resolved.result.writes,
        };
        let adopt = |published| {
            adopt_native_edit(
                &resolved.capture,
                &resolved.descriptor,
                published,
                head,
                &edits,
            )
        };
        let adopted = adopt(published).unwrap();
        assert!(matches!(
            direct_program(&adopted).as_ref(),
            ColorProgram::Direct { .. }
        ));
        for (name, candidate) in [
            (
                "stale token",
                PublishedColorHead {
                    token: &stale_token,
                    ..published
                },
            ),
            (
                "wrong target",
                PublishedColorHead {
                    target: FixtureId::new(),
                    ..published
                },
            ),
            (
                "foreign footprint",
                PublishedColorHead {
                    writes: &resolved.result.writes[1..],
                    ..published
                },
            ),
        ] {
            assert!(
                adopt(candidate).is_err(),
                "{name} accepted for a {} value",
                if value == &semantic {
                    "Semantic"
                } else {
                    "Direct"
                }
            );
        }
    }
}

/// Two cells sharing one master UV channel: the master cell's UV emitter is reversed with known
/// leakage, the second cell's is forward with unknown leakage.
fn two_heads_sharing_uv() -> FixtureProfile {
    let mut profile = two_heads(rgb());
    let mode = &mut profile.modes[0];
    let mut uv = rgbwauv(None).modes[0].channels[6].clone();
    uv.head_id = mode.heads[0].id;
    let (channel, function) = (uv.id, uv.functions[0].id);
    mode.channels.push(uv);
    mode.splits[0].footprint += 1;
    let emitter = |reversed: bool, leak: Option<Xyz>| light_fixture::OpticalEmitter {
        id: Uuid::new_v4(),
        name: "color.uv".into(),
        binding: NativeColorBinding {
            channel_id: channel,
            function_id: function,
        },
        xyz: leak,
        spectrum: vec![],
        band: OpticalEmitterBand::Ultraviolet,
        native_reversed: reversed,
        maximum_level: 1.0,
        response_exponent: 1.0,
        provenance: provenance(PhysicalDataQuality::Estimated),
    };
    let paths = &mut mode.color_physical.as_mut().unwrap().paths;
    for (path, (reversed, leak)) in paths
        .iter_mut()
        .zip([(true, Some(xyz(0.02, 0.01, 0.08))), (false, None)])
    {
        path.controls.push(channel);
        if let OpticalSource::Additive { emitters } = &mut path.source {
            emitters.push(emitter(reversed, leak));
        }
    }
    profile.validate().unwrap();
    profile
}

/// Finding 3: with visible held, the second cell's UV proposal (0) differs from the shared raw
/// the master cell wrote (255, reversed off). The second cell reports the shared conflict and,
/// from the FINAL values (its UV drive is 1.0 with unknown leakage), unknown UV appearance,
/// although its provisional UV-only fit (drive 0) was known.
#[test]
fn held_visible_reports_final_uv_appearance_and_shared_uv_conflicts() {
    let profile = two_heads_sharing_uv();
    let source = unknown_white_uv(xyz(0.02, 0.01, 0.08));
    let rig = DirectRig::new(&rgb(), &[&source]);
    let root = rig.target;
    let unpatched = patched(&profile, root, 1);
    let mut validated = unpatched.clone();
    validated.logical_heads.push(light_fixture::PatchedHead {
        profile_head_id: Some(profile.modes[0].heads[1].id),
        head_index: 1,
        fixture_id: FixtureId::new(),
    });
    rig.install_topology(validated, unpatched, &[&source, &profile]);
    let value = direct(&rig.catalogue.borrow(), &source, &[0, 0, 0, 255, 0]);
    let resolved = rig.resolve(&value, None).unwrap();
    let heads = &resolved.result.quality.heads;
    assert_eq!(heads.len(), 2);
    let uv = profile.modes[0].channels.last().unwrap().id;
    assert_eq!(resolved.raw(uv), 255, "reversed master UV is closed at 255");
    let master = &heads[0].quality;
    assert!(matches!(
        master.direct.as_ref().unwrap().replay,
        DirectReplayOutcome::Fallback {
            visible: VisibleFallback::Hold,
            ..
        }
    ));
    assert!(!master.shared_conflict);
    assert!(master.uv_appearance_known, "zero master drive is known");
    let cell = &heads[1].quality;
    assert!(cell.shared_conflict, "{cell:?}");
    assert!(
        cell.limitations
            .contains(ColorFitLimitations::SHARED_CONTROL)
    );
    assert!(
        !cell.uv_appearance_known,
        "final UV drive 1.0 has unknown leakage"
    );
    assert!(
        cell.limitations
            .contains(ColorFitLimitations::UNKNOWN_UV_APPEARANCE)
    );
    assert_eq!(cell.total_quality, PhysicalDataQuality::Unknown);
    assert_eq!(rig.adapter.counters().shared_conflicts, 1);
}

// TL-599: measured replay quality uses the fixture fitter's f64 chromaticity and black metric.

/// `profile` with its first (red) path emitter replaced by `value`, at a new revision.
fn with_first_emitter(profile: &FixtureProfile, revision: u32, value: Xyz) -> FixtureProfile {
    let mut changed = profile.clone();
    changed.revision = revision;
    let path = &mut changed.modes[0].color_physical.as_mut().unwrap().paths[0];
    if let OpticalSource::Additive { emitters } = &mut path.source {
        emitters[0].xyz = Some(value);
    }
    changed.validate().unwrap();
    changed
}

/// Every published diagnostic number, top-level and per head, is finite or absent.
fn assert_diagnostics_finite(quality: &ColorQuality) {
    let finite = |q: &ColorQuality| {
        q.delta_uv.is_none_or(f64::is_finite) && q.luminance_ratio.is_none_or(f64::is_finite)
    };
    assert!(finite(quality), "{quality:?}");
    for head in &quality.heads {
        assert!(finite(&head.quality), "{:?}", head.quality);
    }
}

/// Dim red: XYZ denominator ≈ 3.5e-7, below the core f32 1e-6 chromaticity threshold.
const DIM_RED: Xyz = Xyz {
    x: 4e-8,
    y: 2e-8,
    z: 2e-9,
};
/// Dim blue at a comparable level.
const DIM_BLUE: Xyz = Xyz {
    x: 1.5e-8,
    y: 1e-8,
    z: 8e-8,
};
const RED_ONLY: [u32; 4] = [255, 0, 0, 0];

/// A dim red source replayed exactly on a compatible destination whose recalibrated red emitter
/// is dim blue: native identity is Exact, but the measured match is out of gamut, never Exact
/// and never black.
#[test]
fn dim_mismatched_native_replay_is_neither_exact_nor_black() {
    let source = with_first_emitter(&rgbw_widths(), 1, DIM_RED);
    let destination = with_first_emitter(&source, 2, DIM_BLUE);
    let rig = DirectRig::new(&destination, &[&source, &destination]);
    let value = direct(&rig.catalogue.borrow(), &source, &RED_ONLY);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    let quality = &resolved.result.quality;
    assert_eq!(quality.color_match, ColorMatch::OutOfGamut, "{quality:?}");
    let delta = quality.delta_uv.expect("dim colours keep chromaticity");
    assert!(delta > f64::from(light_core::color_intent::APPROXIMATE_DELTA_UV));
    let ratio = quality
        .luminance_ratio
        .expect("dim source Y supports a ratio");
    assert!((ratio - 0.5).abs() < 1e-3, "{ratio}");
    assert!(quality.luminance_limited);
    assert_diagnostics_finite(quality);
}

/// The same dim red replayed on the same native identity is a measured Exact match with a
/// real chromaticity distance and luminance ratio, not a black shortcut.
#[test]
fn dim_same_colour_native_replay_is_a_measured_exact_match() {
    let source = with_first_emitter(&rgbw_widths(), 1, DIM_RED);
    let rig = DirectRig::new(&source, &[&source]);
    let value = direct(&rig.catalogue.borrow(), &source, &RED_ONLY);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    let quality = &resolved.result.quality;
    assert_eq!(quality.color_match, ColorMatch::Exact, "{quality:?}");
    assert!(quality.delta_uv.expect("chromatic, not black") < 1e-6);
    let ratio = quality.luminance_ratio.expect("not black");
    assert!((ratio - 1.).abs() < 1e-6, "{ratio}");
    assert!(!quality.luminance_limited);
    assert_diagnostics_finite(quality);
}

/// A true black recipe replays as black: Exact with no chromaticity or ratio.
#[test]
fn true_black_native_replay_is_exact_black() {
    let source = rgbw_widths();
    let rig = DirectRig::new(&source, &[&source]);
    let value = direct(&rig.catalogue.borrow(), &source, &[0, 0, 0, 0]);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    let quality = &resolved.result.quality;
    assert_eq!(quality.color_match, ColorMatch::Exact, "{quality:?}");
    assert_eq!(quality.delta_uv, None);
    assert_eq!(quality.luminance_ratio, None);
    assert!(!quality.luminance_limited);
}

/// A valid nonblack source with Y = 0 is chromatic, compared by Δu′v′, and publishes no
/// luminance ratio instead of NaN/Inf, on both a matching and a mismatching destination.
#[test]
fn zero_luminance_nonblack_source_never_publishes_nan_or_inf() {
    let source = with_first_emitter(&rgbw_widths(), 1, xyz(0.30, 0.0, 0.10));
    let rig = DirectRig::new(&source, &[&source]);
    let value = direct(&rig.catalogue.borrow(), &source, &RED_ONLY);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    let quality = &resolved.result.quality;
    assert_diagnostics_finite(quality);
    assert_eq!(quality.color_match, ColorMatch::Exact, "{quality:?}");
    assert!(quality.delta_uv.expect("zero-Y nonblack is chromatic") < 1e-6);
    assert_eq!(quality.luminance_ratio, None);
    assert!(!quality.luminance_limited);

    let mismatched = with_first_emitter(&source, 2, xyz(0.10, 0.0, 0.30));
    let rig = DirectRig::new(&mismatched, &[&source, &mismatched]);
    let value = direct(&rig.catalogue.borrow(), &source, &RED_ONLY);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_eq!(resolved.direct().replay, DirectReplayOutcome::Exact);
    let quality = &resolved.result.quality;
    assert_diagnostics_finite(quality);
    assert_eq!(quality.color_match, ColorMatch::OutOfGamut, "{quality:?}");
    assert_eq!(quality.luminance_ratio, None);
}

/// `DirectReplayOutcome::Exact` is native identity only: an incompatible destination with
/// identical optics never reports an Exact replay, whatever its measured match.
#[test]
fn exact_replay_outcome_is_native_identity_only() {
    let source = with_first_emitter(&rgbw_widths(), 1, DIM_RED);
    let other = with_first_emitter(&rgbw_widths(), 1, DIM_RED);
    assert_ne!(source.id, other.id);
    let rig = DirectRig::new(&other, &[&source, &other]);
    let value = direct(&rig.catalogue.borrow(), &source, &RED_ONLY);
    let resolved = rig.resolve(&value, None).unwrap();
    assert_ne!(resolved.direct().replay, DirectReplayOutcome::Exact);
    assert_diagnostics_finite(&resolved.result.quality);
}

/// The comparison itself, including inputs no compiled profile produces.
#[test]
fn measured_source_comparison_classifies_black_dim_zero_y_and_unknown() {
    use super::direct::measure_against_source as measure;
    let scale = |v: Xyz, k: f32| xyz(v.x * k, v.y * k, v.z * k);
    let black = xyz(0., 0., 0.);
    // Dim red vs dim blue: out of gamut, finite diagnostics.
    let m = measure(DIM_BLUE, true, Some(DIM_RED));
    assert_eq!(m.color_match, ColorMatch::OutOfGamut);
    assert!(m.delta_uv.is_some() && m.luminance_ratio.is_some());
    // Dim same colour at half the level: chromatic Exact, luminance-limited.
    let m = measure(scale(DIM_RED, 0.5), true, Some(DIM_RED));
    assert_eq!(m.color_match, ColorMatch::Exact);
    assert!((m.luminance_ratio.unwrap() - 0.5).abs() < 1e-6);
    assert!(m.luminance_limited);
    // Black output for a dim chromatic source is not a match.
    let m = measure(black, true, Some(DIM_RED));
    assert_eq!(m.color_match, ColorMatch::OutOfGamut);
    assert_eq!(m.delta_uv, None);
    assert!(m.luminance_limited);
    // True black against black.
    let m = measure(black, true, Some(black));
    assert_eq!(m.color_match, ColorMatch::Exact);
    assert_eq!((m.delta_uv, m.luminance_ratio), (None, None));
    assert!(!m.luminance_limited);
    // Visible output against a black source.
    let m = measure(DIM_RED, true, Some(black));
    assert_eq!(m.color_match, ColorMatch::OutOfGamut);
    assert!(m.luminance_limited);
    // Nonblack zero-Y source: ratio absent, never NaN/Inf, for zero and nonzero actual Y.
    let zero_y = xyz(0.3, 0., 0.1);
    for actual in [zero_y, xyz(0.3, 0.2, 0.1), black] {
        let m = measure(actual, true, Some(zero_y));
        assert_eq!(m.luminance_ratio, None);
        assert!(m.delta_uv.is_none_or(f64::is_finite));
        assert_ne!(m.color_match, ColorMatch::Unknown);
    }
    assert_eq!(
        measure(zero_y, true, Some(zero_y)).color_match,
        ColorMatch::Exact
    );
    // Unknown source, incomplete output or non-finite input: Unknown, never Exact.
    for (actual, complete, source) in [
        (DIM_RED, true, None),
        (DIM_RED, false, Some(DIM_RED)),
        (black, false, Some(black)),
        (xyz(f32::NAN, 0., 0.), true, Some(DIM_RED)),
        (DIM_RED, true, Some(xyz(0., f32::INFINITY, 0.))),
    ] {
        let m = measure(actual, complete, source);
        assert_eq!(m.color_match, ColorMatch::Unknown);
        assert_eq!((m.delta_uv, m.luminance_ratio), (None, None));
        assert!(!m.luminance_limited);
    }
}
