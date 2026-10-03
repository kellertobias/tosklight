//! Position-native capture must describe the same root/copy channel space as final output.
//! These tests deliberately use ordinary scalar values, not hand-built fitter inputs.
use super::*;
use uuid::Uuid;

struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    root: FixtureId,
    copy: Uuid,
}

fn rig() -> Rig {
    let (mut fixture, root) = schema_v2_fixture(&[
        ("pan", false, false, false, false, false),
        ("tilt", false, false, false, false, false),
        ("beam.focus", false, false, false, false, false),
        ("intensity", false, false, false, false, true),
    ]);
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 7;
    for (index, channel) in mode.channels.iter_mut().take(3).enumerate() {
        channel.resolution = ChannelResolution::U16;
        channel.secondary_slots = vec![2 * index as u16 + 2];
        channel.highlight_raw = u16::MAX.into();
        channel.functions = vec![ChannelFunction::continuous(
            channel.attribute.0.to_string(),
            channel.attribute.clone(),
            u16::MAX.into(),
        )];
    }
    // Manufacturer inversion and installation inversion are distinct transformations.
    mode.channels[0].invert = true;
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    fixture.invert_pan = true;
    fixture.invert_tilt = false;
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        name: "Independent inversion".into(),
        universe: Some(1),
        address: Some(20),
        split_patches: vec![],
        location: Default::default(),
        rotation: Default::default(),
        invert_pan: false,
        invert_tilt: true,
        position_calibration: None,
        color_calibration: None,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        scenery_size_metres: None,
    });
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    for (attribute, value) in [
        ("pan", 0.25),
        ("tilt", 0.2),
        ("beam.focus", 0.6),
        ("intensity", 0.8),
    ] {
        programmers.set(
            session,
            root,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        );
    }
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    Rig {
        engine,
        programmers,
        session,
        root,
        copy,
    }
}

fn assert_empty(out: &CapturedNativeRaw) {
    assert!(out.token().is_none());
    assert!(out.destination().is_none());
    assert!(out.instance_id().is_none());
    assert!(out.raw().is_empty());
}

fn capture_instances(rig: &Rig, options: RenderOptions) -> (Vec<u32>, Vec<u32>, RenderResult) {
    let capture = rig.engine.prepare_output_frame(options);
    let token = capture.frame_token();
    let frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    let mut out = CapturedNativeRaw::default();
    frame
        .native_position_raw_into(&capture, &token, rig.root, rig.root.0, &mut out)
        .unwrap();
    assert_eq!(out.token(), Some(&token));
    assert_eq!(out.destination(), Some(rig.root));
    assert_eq!(out.instance_id(), Some(rig.root.0));
    let root = out.raw().to_vec();
    frame
        .native_position_raw_into(&capture, &token, rig.root, rig.copy, &mut out)
        .unwrap();
    assert_eq!(out.instance_id(), Some(rig.copy));
    let copy = out.raw().to_vec();
    // Reusing the same buffer for the generic domain must remove physical-instance metadata.
    frame
        .native_raw_into(&capture, &token, rig.root, &mut out)
        .unwrap();
    assert_eq!(out.instance_id(), None);
    assert_eq!(out.destination(), Some(rig.root));
    let generic = frame.native_raw(&capture, &token, rig.root).unwrap();
    assert_eq!(out.raw(), generic.raw());
    let rendered = rig
        .engine
        .render_static_family_frame(&capture, frame)
        .unwrap();
    (root, copy, rendered)
}

fn assert_render_agreement(rig: &Rig, root: &[u32], copy: &[u32], rendered: &RenderResult) {
    for (instance_id, expected, offset) in [(rig.root.0, root, 0), (rig.copy, copy, 19)] {
        let instance = rendered
            .physical
            .instances
            .iter()
            .find(|i| i.instance_id == instance_id)
            .unwrap();
        assert_eq!(instance.fixture_id, rig.root);
        assert!(instance.complete);
        assert_eq!(instance.native_raw.as_ref(), expected);
        // Verify every coarse/fine byte as well as the observational native vector.
        let mut encoded = Vec::new();
        for value in &expected[..3] {
            encoded.extend_from_slice(&(*value as u16).to_be_bytes());
        }
        encoded.push(expected[3] as u8);
        assert_eq!(&rendered.universes[&1][offset..offset + 7], encoded);
    }
}

#[test]
fn position_native_normalized_u16_matches_root_and_independent_copy_output() {
    let rig = rig();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    let generic = frame
        .native_raw(&capture, &capture.frame_token(), rig.root)
        .unwrap();
    assert_eq!(generic.instance_id(), None);
    assert_eq!(generic.raw(), &[49151, 13107, 39321, 204]);
    let (root, copy, rendered) = capture_instances(&rig, Default::default());
    assert_eq!(
        root,
        [16384, 13107, 39321, 204],
        "patch and profile Pan inversions cancel once"
    );
    assert_eq!(
        copy,
        [49151, 52428, 39321, 204],
        "copy uses its own Pan/Tilt inversion"
    );
    assert_render_agreement(&rig, &root, &copy, &rendered);

    let (mastered_root, mastered_copy, mastered) = capture_instances(
        &rig,
        RenderOptions {
            grand_master: 0.25,
            ..Default::default()
        },
    );
    assert_eq!(mastered_root, root);
    assert_eq!(mastered_copy, copy);
    assert_eq!(mastered.physical.instances[0].native_raw[3], 51);
    let (blackout_root, blackout_copy, _) = capture_instances(
        &rig,
        RenderOptions {
            blackout: true,
            ..Default::default()
        },
    );
    assert_eq!(blackout_root, root);
    assert_eq!(blackout_copy, copy);
}

#[test]
fn position_native_explicit_raw_never_receives_installation_inversion() {
    let rig = rig();
    for (pan, tilt, expected_pan, expected_tilt) in [
        (
            AttributeValue::RawDmxExact(12345),
            AttributeValue::RawDmxExact(45678),
            12345,
            45678,
        ),
        // RawDmx is a semantic 8-bit fraction: manufacturer channel inversion still applies.
        (
            AttributeValue::RawDmx(32),
            AttributeValue::RawDmx(200),
            57311,
            51400,
        ),
    ] {
        rig.programmers
            .set(rig.session, rig.root, AttributeKey("pan".into()), pan);
        rig.programmers
            .set(rig.session, rig.root, AttributeKey("tilt".into()), tilt);
        let (root, copy, rendered) = capture_instances(&rig, Default::default());
        assert_eq!(root, [expected_pan, expected_tilt, 39321, 204]);
        assert_eq!(
            copy, root,
            "explicit Raw is already native with respect to installation"
        );
        assert_render_agreement(&rig, &root, &copy, &rendered);
    }
}

#[test]
fn position_native_rejects_foreign_capture_instance_and_preload_branch_and_clears_reused_output() {
    let rig = rig();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    let token = capture.frame_token();
    let other = rig.engine.prepare_output_frame(Default::default());
    let preload = rig.engine.prepare_preload_frame(&capture, None);
    let state = PreloadFrameState::default();
    let before_token = preload.frame_token(&state, PreloadBranch::BeforeRelease);
    let after_token = preload.frame_token(&state, PreloadBranch::AfterRelease);
    let before = rig.engine.prepare_preload_static_family_frame(
        &preload,
        &[],
        &state,
        PreloadBranch::BeforeRelease,
    );
    let mut out = CapturedNativeRaw::default();
    for (test_capture, test_token, target, instance) in [
        (&other, &other.frame_token(), rig.root, rig.copy),
        (&capture, &other.frame_token(), rig.root, rig.copy),
        (&capture, &before_token, rig.root, rig.copy),
        (&capture, &token, FixtureId::new(), rig.copy),
        (&capture, &token, rig.root, Uuid::new_v4()),
    ] {
        frame
            .native_position_raw_into(&capture, &token, rig.root, rig.copy, &mut out)
            .unwrap();
        assert!(!out.raw().is_empty());
        assert!(
            frame
                .native_position_raw_into(test_capture, test_token, target, instance, &mut out)
                .is_err()
        );
        assert_empty(&out);
    }
    before
        .native_position_raw_into(&capture, &before_token, rig.root, rig.copy, &mut out)
        .unwrap();
    assert_eq!(out.token(), Some(&before_token));
    assert_eq!(out.instance_id(), Some(rig.copy));
    assert_eq!(out.raw(), &[49151, 52428, 39321, 204]);
    assert!(
        before
            .native_position_raw_into(&capture, &after_token, rig.root, rig.copy, &mut out)
            .is_err()
    );
    assert_empty(&out);
    before
        .native_position_raw_into(&capture, &before_token, rig.root, rig.copy, &mut out)
        .unwrap();
    assert!(
        before
            .native_position_raw_into(&capture, &token, rig.root, rig.copy, &mut out)
            .is_err()
    );
    assert_empty(&out);
}

#[test]
fn position_native_unpatched_dmx_keeps_inversion_but_internal_profile_does_not() {
    let rig = rig();
    let mut snapshot = rig.engine.snapshot().as_ref().clone();
    let mut fixture = snapshot.fixtures[0].clone();
    fixture.universe = None;
    fixture.address = None;
    fixture.multipatch[0].universe = None;
    fixture.multipatch[0].address = None;
    snapshot.fixtures = vec![fixture.clone()].into();
    rig.engine.replace_snapshot(snapshot.clone()).unwrap();
    let (root, copy, rendered) = capture_instances(&rig, Default::default());
    assert_eq!(root, [16384, 13107, 39321, 204]);
    assert_eq!(copy, [49151, 52428, 39321, 204]);
    assert!(rendered.universes.is_empty());
    for (id, values) in [(rig.root.0, &root), (rig.copy, &copy)] {
        let observed = rendered
            .physical
            .instances
            .iter()
            .find(|i| i.instance_id == id)
            .unwrap();
        assert_eq!(observed.native_raw.as_ref(), values);
    }

    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    profile.patch_policy = light_fixture::PatchPolicy::Internal;
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 0;
    for channel in &mut mode.channels {
        channel.secondary_slots.clear();
    }
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    // Internal fixtures cannot have physical copies, even with no DMX address.
    fixture.multipatch.clear();
    snapshot.fixtures = vec![fixture].into();
    rig.engine.replace_snapshot(snapshot).unwrap();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    let token = capture.frame_token();
    let mut out = CapturedNativeRaw::default();
    frame
        .native_position_raw_into(&capture, &token, rig.root, rig.root.0, &mut out)
        .unwrap();
    assert_eq!(
        out.raw(),
        &[49151, 13107, 39321, 204],
        "non-DMX profiles ignore installation axis inversion"
    );
    assert_eq!(out.instance_id(), Some(rig.root.0));
    assert!(
        frame
            .native_position_raw_into(&capture, &token, rig.root, rig.copy, &mut out)
            .is_err()
    );
    assert_empty(&out);
    let rendered = rig
        .engine
        .render_static_family_frame(&capture, frame)
        .unwrap();
    assert!(rendered.universes.is_empty());
    assert_eq!(rendered.physical.instances.len(), 1);
    assert_eq!(
        rendered.physical.instances[0].native_raw.as_ref(),
        &[49151, 13107, 39321, 204]
    );
}

/// TL-553: a static token captures each root/instance vector once and replays it. Every replay
/// must equal a fresh capture from another token of the same capture, root, copy and generic
/// domains must not share entries, and a later capture sees later Programmer values.
#[test]
fn repeated_native_captures_of_one_token_equal_fresh_captures() {
    let rig = rig();
    let read = |frame: &PreparedStaticFamilyFrame,
                capture: &PreparedOutputFrame,
                instance: Option<Uuid>| {
        let token = capture.frame_token();
        let mut out = CapturedNativeRaw::default();
        match instance {
            Some(id) => frame.native_position_raw_into(capture, &token, rig.root, id, &mut out),
            None => frame.native_raw_into(capture, &token, rig.root, &mut out),
        }
        .unwrap();
        assert_eq!(out.token(), Some(&token));
        assert_eq!(out.destination(), Some(rig.root));
        assert_eq!(out.instance_id(), instance);
        out.raw().to_vec()
    };
    let instances = [None, Some(rig.root.0), Some(rig.copy)];
    let mut previous: Option<Vec<Vec<u32>>> = None;
    for step in 0..3 {
        rig.programmers.set(
            rig.session,
            rig.root,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2 + 0.3 * step as f32),
        );
        let capture = rig.engine.prepare_output_frame(Default::default());
        let cached = rig.engine.prepare_static_family_frame(&capture, &[]);
        let first = instances.map(|instance| read(&cached, &capture, instance));
        for (index, instance) in instances.into_iter().enumerate() {
            let replayed = read(&cached, &capture, instance);
            let fresh = rig.engine.prepare_static_family_frame(&capture, &[]);
            assert_eq!(replayed, read(&fresh, &capture, instance), "{instance:?}");
            assert_eq!(replayed, first[index]);
        }
        // Installation inversion differs between root and copy, so their entries differ.
        assert_ne!(first[1], first[2]);
        if let Some(previous) = &previous {
            assert_ne!(
                previous[0], first[0],
                "a new capture reads the new Pan value"
            );
        }
        previous = Some(first.to_vec());
    }
}
