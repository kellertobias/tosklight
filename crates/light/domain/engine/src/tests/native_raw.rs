//! TL-592: pre-render native raw values for physical fitters, bound to one captured frame.
use super::*;

fn engine(reacting_red: bool) -> (Engine, FixtureId, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    registry.start(session);
    let id = FixtureId::new();
    let mut fixture = calibrated_visual_fixture(id);
    let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
    let mut profile = profile.clone();
    let mode = &mut profile.modes[0];
    mode.channels[1].invert = false;
    mode.channels[1].reacts_to_virtual_intensity = reacting_red;
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    for (name, value) in [
        ("intensity", 0.5),
        ("color.red", 0.8),
        ("color.green", 0.2),
        ("color.blue", 1.),
    ] {
        registry.set(
            session,
            id,
            AttributeKey(name.into()),
            AttributeValue::Normalized(value),
        );
    }
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, id, clock)
}

fn read(engine: &Engine, capture: &PreparedOutputFrame, target: FixtureId) -> Vec<u32> {
    let token = capture.frame_token();
    let frame = engine.prepare_static_family_frame(capture, &[]);
    let raw = frame.native_raw(capture, &token, target).unwrap();
    assert_eq!(raw.token(), Some(&token));
    assert_eq!(raw.destination(), Some(target));
    raw.raw().to_vec()
}

#[test]
fn native_raw_is_the_pre_master_scalar_baseline_of_the_destination_mode() {
    let (engine, id, _) = engine(false);
    let capture = engine.prepare_output_frame(Default::default());
    let raw = read(&engine, &capture, id);
    // Same values the ordinary render encodes when no master or overlay applies.
    let rendered = engine.render(Default::default()).unwrap();
    let dmx = &rendered.universes[&1][0..4];
    assert_eq!(raw, dmx.iter().map(|v| u32::from(*v)).collect::<Vec<_>>());
    assert_eq!(raw, [128, 204, 51, 255]);

    // Masters, blackout and virtual intensity never reach the pre-master values.
    let (engine, id, _) = engine_with_reacting_red();
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
        let capture = engine.prepare_output_frame(options);
        assert_eq!(
            read(&engine, &capture, id),
            [128, 204, 51, 255],
            "{options:?}"
        );
    }
    let rendered = engine.render(Default::default()).unwrap();
    assert_eq!(
        rendered.universes[&1][1], 102,
        "the render applies virtual intensity exactly once"
    );
    let heads = profile_head_destinations(&engine.snapshot(), id);
    assert_eq!(heads.len(), 1);
    assert_eq!((heads[0].destination, heads[0].head_index), (id, 0));
    assert!(profile_head_destinations(&engine.snapshot(), FixtureId::new()).is_empty());
}

fn engine_with_reacting_red() -> (Engine, FixtureId, Arc<ManualClock>) {
    engine(true)
}

#[test]
fn native_raw_rejects_foreign_tokens_captures_and_unknown_targets() {
    let (engine, id, clock) = engine(false);
    let capture = engine.prepare_output_frame(Default::default());
    let frame = engine.prepare_static_family_frame(&capture, &[]);
    clock.advance_millis(25);
    let other = engine.prepare_output_frame(Default::default());
    let mut out = CapturedNativeRaw::default();
    assert!(matches!(
        frame.native_raw_into(&other, &other.frame_token(), id, &mut out),
        Err(EngineError::StalePreparedFrame)
    ));
    assert!(
        frame
            .native_raw_into(&capture, &other.frame_token(), id, &mut out)
            .is_err()
    );
    let preload = engine.prepare_preload_frame(&capture, None);
    let branch = preload.frame_token(&PreloadFrameState::default(), PreloadBranch::AfterRelease);
    assert!(
        frame
            .native_raw_into(&capture, &branch, id, &mut out)
            .is_err()
    );
    assert!(
        frame
            .native_raw_into(&capture, &capture.frame_token(), FixtureId::new(), &mut out)
            .is_err()
    );
    assert!(out.token().is_none() && out.raw().is_empty());
    frame
        .native_raw_into(&capture, &capture.frame_token(), id, &mut out)
        .unwrap();
    assert_eq!(out.raw().len(), 4);
    // Reading is side-effect free: the same capture still renders.
    engine.render_static_family_frame(&capture, frame).unwrap();
}

/// TL-639 round 4: a channel subset reads exactly the complete capture's values, in the order
/// asked, for every subset; a channel outside the mode is rejected.
#[test]
fn a_channel_subset_reads_exactly_the_complete_capture() {
    let (engine, id, _) = engine(true);
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let frame = engine.prepare_static_family_frame(&capture, &[]);
    let complete = frame.native_raw(&capture, &token, id).unwrap();
    let complete = complete.raw();
    let mut out = vec![99];
    for mask in 0u32..(1 << complete.len()) {
        let channels = (0..complete.len())
            .filter(|channel| mask & (1 << channel) != 0)
            .collect::<Vec<_>>();
        let destination = frame
            .native_raw_channels_into(&capture, &token, id, &channels, &mut out)
            .unwrap();
        assert_eq!(destination, id);
        let expected = channels
            .iter()
            .map(|&channel| complete[channel])
            .collect::<Vec<_>>();
        assert_eq!(out, expected, "subset {channels:?}");
    }
    assert!(
        frame
            .native_raw_channels_into(&capture, &token, id, &[complete.len()], &mut out)
            .is_err()
    );
    assert!(out.is_empty(), "a rejected read leaves nothing behind");
}
