//! TL-553: a retained descriptor replays its last semantic fit while the intent and the head's
//! fit inputs are unchanged, with output identical to a fresh (unmemoized) resolve, and refits
//! as soon as any input changes.
use super::*;

/// A frame resolved through the retained descriptor and the same frame through a fresh one.
struct Paired {
    retained: Resolved,
    fresh: PhysicalResolution<ColorAdapter>,
    fits: u64,
    reuses: u64,
}

fn paired(
    rig: &Rig,
    descriptor: &ColorDescriptor,
    request: &ColorIntent,
    previous: Option<&ColorContinuity>,
) -> Paired {
    let before = rig.adapter.counters();
    let retained = rig
        .resolve_on(
            Some(descriptor),
            request,
            previous,
            RenderOptions::default(),
        )
        .unwrap();
    let after = rig.adapter.counters();
    let fresh = rig
        .resolve_on(None, request, previous, RenderOptions::default())
        .unwrap()
        .result;
    retained.verify(request, &rig.profile.borrow());
    Paired {
        retained,
        fresh,
        fits: after.fits - before.fits,
        reuses: after.result_reuses - before.result_reuses,
    }
}

fn without_work(mut quality: ColorQuality) -> ColorQuality {
    quality.work = ColorSolveWork::default();
    for head in &mut quality.heads {
        head.quality.work = ColorSolveWork::default();
    }
    quality
}

/// Writes, achieved output, continuity and status equal the unmemoized resolve; a replay
/// publishes zero work.
fn assert_identical(frame: &Paired, replayed: bool) {
    let (retained, fresh) = (&frame.retained.result, &frame.fresh);
    assert_eq!(retained.writes, fresh.writes);
    assert_eq!(retained.achieved, fresh.achieved);
    assert_eq!(retained.continuity, fresh.continuity);
    assert_eq!(retained.requested, fresh.requested);
    if replayed {
        assert_eq!((frame.fits, frame.reuses), (0, 1));
        assert_eq!(retained.quality, without_work(fresh.quality.clone()));
    } else {
        assert!(frame.fits >= 1);
        assert_eq!(frame.reuses, 0);
        assert_eq!(retained.quality, fresh.quality);
    }
}

fn descriptor(rig: &Rig) -> ColorDescriptor {
    let capture = rig.engine.prepare_output_frame(RenderOptions::default());
    rig.adapter
        .compile(&capture.snapshot(), rig.target)
        .unwrap()
        .expect("physical Color destination")
}

#[test]
fn unchanged_fit_inputs_replay_exactly_even_when_intensity_changes() {
    let rig = Rig::new(&rgbw());
    let descriptor = descriptor(&rig);
    let first = paired(&rig, &descriptor, &magenta(), None);
    assert_identical(&first, false);
    let continuity = first.retained.result.continuity.clone();
    // Seeding from the accepted continuity changes the fit input once.
    let seeded = paired(&rig, &descriptor, &magenta(), Some(&continuity));
    let continuity = seeded.retained.result.continuity.clone();
    let steady = paired(&rig, &descriptor, &magenta(), Some(&continuity));
    assert_identical(&steady, true);
    // Intensity is not an input of the Color fit: still a replay, still identical output.
    let mut intensity = steady.retained.native[0];
    for level in [0.3, 0.9, 0.0] {
        rig.set("intensity", level);
        let frame = paired(&rig, &descriptor, &magenta(), Some(&continuity));
        assert_ne!(
            frame.retained.native[0], intensity,
            "the native raw changed"
        );
        intensity = frame.retained.native[0];
        assert_identical(&frame, true);
    }
}

#[test]
fn every_fit_input_invalidates_the_replay() {
    let rig = Rig::new(&rgbw());
    let descriptor = descriptor(&rig);
    let first = paired(&rig, &descriptor, &magenta(), None);
    assert_identical(&first, false);
    assert_identical(&paired(&rig, &descriptor, &magenta(), None), true);
    // A changed intent refits.
    assert_identical(&paired(&rig, &descriptor, &warm_white(), None), false);
    assert_identical(&paired(&rig, &descriptor, &warm_white(), None), true);
    // A changed relative output (one intent field) refits.
    let dimmer = ColorIntent {
        relative_output: 0.5,
        ..warm_white()
    };
    assert_identical(&paired(&rig, &descriptor, &dimmer, None), false);
    // Changed previous writes change the seeded controls: refit.
    let continuity = first.retained.result.continuity.clone();
    assert_identical(
        &paired(&rig, &descriptor, &dimmer, Some(&continuity)),
        false,
    );
    // A changed scalar value of a Color control changes the unseeded input: refit.
    rig.set("color.green", 0.7);
    assert_identical(&paired(&rig, &descriptor, &dimmer, None), false);
    assert_identical(&paired(&rig, &descriptor, &dimmer, None), true);
}

#[test]
fn a_discrete_wheel_position_is_a_fit_input() {
    let rig = Rig::new(&wheel_only());
    let descriptor = descriptor(&rig);
    let request = intent([1., 0., 0.], 0.);
    assert_identical(&paired(&rig, &descriptor, &request, None), false);
    assert_identical(&paired(&rig, &descriptor, &request, None), true);
    for value in [0.1, 0.5, 0.9] {
        rig.set("color.wheel.1", value);
        assert_identical(&paired(&rig, &descriptor, &request, None), false);
        assert_identical(&paired(&rig, &descriptor, &request, None), true);
    }
}

#[test]
fn head_input_channels_cover_the_color_footprint_and_never_intensity() {
    for profile in [rgbw(), wheel_only(), cmy_wheel(), rgbwauv(Some(white()))] {
        let rig = Rig::new(&profile);
        let descriptor = descriptor(&rig);
        for head in descriptor.heads.iter() {
            assert!(!head.inputs.contains(&0), "{}: Intensity", profile.name);
            for control in head.controls.iter() {
                assert!(head.inputs.contains(&(control.channel_index as usize)));
            }
        }
    }
}

fn target_replays(descriptor: &ColorDescriptor) -> u64 {
    descriptor.scratch.lock().target_replays
}

/// TL-639 round 3: once the intent repeats, the whole target replays its kept result while the
/// intent, the previous continuity and the raw values of every Color channel are unchanged,
/// identical to a fresh resolve; Intensity is outside that key.
#[test]
fn an_unchanged_target_replays_whole_and_any_key_change_resolves_again() {
    let rig = Rig::new(&rgbw());
    let descriptor = descriptor(&rig);
    let first = paired(&rig, &descriptor, &magenta(), None);
    let seeded = paired(
        &rig,
        &descriptor,
        &magenta(),
        Some(&first.retained.result.continuity),
    );
    let continuity = seeded.retained.result.continuity.clone();
    // From the second resolve of the intent on, the target is kept.
    assert_identical(
        &paired(&rig, &descriptor, &magenta(), Some(&continuity)),
        true,
    );
    for level in [0.3, 0.9, 0.0] {
        rig.set("intensity", level);
        let replays = target_replays(&descriptor);
        assert_identical(
            &paired(&rig, &descriptor, &magenta(), Some(&continuity)),
            true,
        );
        assert_eq!(
            target_replays(&descriptor),
            replays + 1,
            "Intensity {level}"
        );
    }
    // A changed previous continuity, intent or Color channel resolves the heads again, with
    // the output of a fresh resolve.
    let replays = target_replays(&descriptor);
    let same_as_fresh = |frame: &Paired| {
        let (retained, fresh) = (&frame.retained.result, &frame.fresh);
        assert_eq!(retained.writes, fresh.writes);
        assert_eq!(retained.achieved, fresh.achieved);
        assert_eq!(retained.continuity, fresh.continuity);
        assert_eq!(
            without_work(retained.quality.clone()),
            without_work(fresh.quality.clone())
        );
    };
    same_as_fresh(&paired(&rig, &descriptor, &magenta(), None));
    same_as_fresh(&paired(&rig, &descriptor, &warm_white(), Some(&continuity)));
    rig.set("color.green", 0.7);
    same_as_fresh(&paired(&rig, &descriptor, &warm_white(), Some(&continuity)));
    assert_eq!(target_replays(&descriptor), replays);
    // The repeated intent was kept by the last resolve: both following resolves replay.
    assert_identical(
        &paired(&rig, &descriptor, &warm_white(), Some(&continuity)),
        true,
    );
    assert_identical(
        &paired(&rig, &descriptor, &warm_white(), Some(&continuity)),
        true,
    );
    assert_eq!(target_replays(&descriptor), replays + 2);
}
