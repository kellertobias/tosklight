//! TL-630: installation Pan/Tilt inversion follows the cold compiled Position role, so a motor
//! alias bound to Pan or Tilt mirrors on the wire and in the captured native baseline exactly like
//! a literal `pan`/`tilt` channel. Synthetic profiles prove encoding semantics, not lamp motion.
use super::*;
use crate::{
    CapturedNativeRaw, FamilyProjectionEvidence, FamilyProjectionMaster, FamilyProjectionMetadata,
    PositionNativeWrite,
};
use light_core::programming::{PositionIntent, ProgrammingOwner};

const PAN_ALIAS: &str = "motor.base.rotation";
const TILT_ALIAS: &str = "motor.head.rotation";

/// Rename the bound Pan/Tilt channels (and their bound functions) to custom motor aliases.
fn alias(fixture: &mut PatchedFixture) {
    redefine(fixture, |profile| {
        for (channel, name) in profile.modes[0]
            .channels
            .iter_mut()
            .zip([PAN_ALIAS, TILT_ALIAS])
        {
            let attribute = AttributeKey(name.into());
            channel.attribute = attribute.clone();
            channel.fixture_attribute = attribute.clone();
            channel.functions[0].attribute = attribute;
        }
    });
}

/// Root (DMX 1), Pan-inverted copy (DMX 10) and Tilt-inverted copy (DMX 20).
fn installed(mut fixture: PatchedFixture) -> (PatchedFixture, [Uuid; 3]) {
    let (pan_copy, tilt_copy) = (Uuid::new_v4(), Uuid::new_v4());
    for (id, address, invert_pan) in [(pan_copy, 10, true), (tilt_copy, 20, false)] {
        fixture.multipatch.push(MultiPatchInstance {
            id,
            universe: Some(1),
            address: Some(address),
            invert_pan,
            invert_tilt: !invert_pan,
            ..Default::default()
        });
    }
    let root = fixture.fixture_id.0;
    (fixture, [root, pan_copy, tilt_copy])
}

fn words(rendered: &crate::RenderResult, instance: Uuid) -> Vec<u32> {
    let output = rendered
        .physical
        .instances
        .iter()
        .find(|output| output.instance_id == instance)
        .unwrap();
    assert!(output.complete);
    output.native_raw.to_vec()
}

fn axes(rendered: &crate::RenderResult, instance: Uuid) -> Vec<Option<f64>> {
    rendered
        .physical
        .instances
        .iter()
        .find(|output| output.instance_id == instance)
        .unwrap()
        .axes()
        .iter()
        .map(|axis| axis.absolute_degrees().map(|degrees| degrees.round()))
        .collect()
}

/// Captured pre-master native Position baselines of every instance, then the rendered frame of
/// the very same capture. Each baseline must equal the rendered native vector and DMX words.
fn capture_and_render(
    engine: &Engine,
    target: FixtureId,
    instances: &[Uuid],
) -> (Vec<Vec<u32>>, crate::RenderResult) {
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let frame = engine.prepare_static_family_frame(&capture, &[]);
    let mut out = CapturedNativeRaw::default();
    let captured = instances
        .iter()
        .map(|instance| {
            frame
                .native_position_raw_into(&capture, &token, target, *instance, &mut out)
                .unwrap();
            assert_eq!(out.instance_id(), Some(*instance));
            out.raw().to_vec()
        })
        .collect::<Vec<_>>();
    let rendered = engine.render_static_family_frame(&capture, frame).unwrap();
    for ((instance, baseline), address) in instances.iter().zip(&captured).zip([1, 10, 20]) {
        assert_eq!(
            &words(&rendered, *instance),
            baseline,
            "captured == rendered"
        );
        let encoded: Vec<u8> = baseline
            .iter()
            .flat_map(|raw| (*raw as u16).to_be_bytes())
            .collect();
        assert_eq!(
            &rendered.universes[&1][address - 1..address - 1 + encoded.len()],
            encoded.as_slice(),
            "coarse/fine DMX at {address}"
        );
    }
    (captured, rendered)
}

fn mirrored(root: u32, copy: u32, max: u32) {
    assert!(
        (max..=max + 1).contains(&(root + copy)),
        "{copy} mirrors {root} within 0..={max}"
    );
}

#[test]
fn role_bound_pan_and_tilt_aliases_mirror_on_independent_copies_like_canonical_channels() {
    let mut results = Vec::new();
    for aliases in [false, true] {
        let mut fixture = mover();
        if aliases {
            alias(&mut fixture);
        }
        let target = fixture.fixture_id;
        let (fixture, instances) = installed(fixture);
        let (engine, programmers, session) = engine(fixture);
        let [pan, tilt] = if aliases {
            [PAN_ALIAS, TILT_ALIAS]
        } else {
            ["pan", "tilt"]
        };
        set(&programmers, session, target, pan, 0.3);
        set(&programmers, session, target, tilt, 0.6);
        let (captured, rendered) = capture_and_render(&engine, target, &instances);
        let [root, pan_copy, tilt_copy] = [&captured[0], &captured[1], &captured[2]];
        assert_eq!(root, &[19661, 39321]);
        mirrored(root[0], pan_copy[0], 65535);
        assert_eq!(pan_copy[1], root[1], "Pan-only copy keeps Tilt");
        assert_eq!(tilt_copy[0], root[0], "Tilt-only copy keeps Pan");
        mirrored(root[1], tilt_copy[1], 65535);
        // The physical prediction decodes each mirrored motor word back to the root's
        // calibrated angles, so prediction and wire agree on every instance.
        assert_eq!(axes(&rendered, instances[0]), [Some(-288.), Some(144.)]);
        for copy in &instances[1..] {
            assert_eq!(axes(&rendered, *copy), axes(&rendered, instances[0]));
        }
        results.push(captured);
    }
    assert_eq!(
        results[0], results[1],
        "aliases encode exactly like pan/tilt"
    );
}

#[test]
fn logical_head_owner_alias_mirrors_only_through_its_own_owner() {
    let mut fixture = mover();
    alias(&mut fixture);
    redefine(&mut fixture, |profile| {
        profile.modes[0].heads[0].master_shared = false;
    });
    let logical = FixtureId::new();
    let head_id = fixture.definition.profile_snapshot.as_ref().unwrap().modes[0].heads[0].id;
    fixture.logical_heads = vec![PatchedHead {
        profile_head_id: Some(head_id),
        fixture_id: logical,
        head_index: fixture.definition.heads[0].index,
    }];
    let root = fixture.fixture_id;
    let (fixture, instances) = installed(fixture);
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, logical, PAN_ALIAS, 0.25);
    set(&programmers, session, logical, TILT_ALIAS, 0.75);
    // A value on the root does not own this head's motors and changes nothing.
    set(&programmers, session, root, PAN_ALIAS, 0.9);
    let (captured, _) = capture_and_render(&engine, logical, &instances);
    assert_eq!(captured[0], [16384, 49151]);
    mirrored(captured[0][0], captured[1][0], 65535);
    assert_eq!(captured[1][1], captured[0][1]);
    assert_eq!(captured[2][0], captured[0][0]);
    mirrored(captured[0][1], captured[2][1], 65535);
}

#[test]
fn alias_mirrors_within_its_selected_function_range_and_never_an_unbound_function() {
    let build = |value: &str| {
        let mut fixture = mover();
        alias(&mut fixture);
        redefine(&mut fixture, |profile| {
            let channel = &mut profile.modes[0].channels[0];
            channel.functions[0].dmx_to = 32767;
            channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
                physical_min: -270.,
                physical_max: 270.,
                unit: Some("deg".into()),
            };
            let mut index = ChannelFunction::continuous(
                "Base index",
                AttributeKey("motor.base.index".into()),
                65535,
            );
            index.dmx_from = 32768;
            channel.functions.push(index);
        });
        let target = fixture.fixture_id;
        let (fixture, instances) = installed(fixture);
        let (engine, programmers, session) = engine(fixture);
        set(&programmers, session, target, value, 0.25);
        set(&programmers, session, target, TILT_ALIAS, 0.5);
        let (captured, rendered) = capture_and_render(&engine, target, &instances);
        (captured, rendered, instances)
    };
    let (bound, rendered, instances) = build(PAN_ALIAS);
    assert!(bound[0][0] <= 32767 && bound[1][0] <= 32767, "{bound:?}");
    mirrored(bound[0][0], bound[1][0], 32767);
    assert_eq!(bound[2][0], bound[0][0]);
    // Selected range -270..270: the root sits at -135 and the inverted copy reports it too.
    assert_eq!(axes(&rendered, instances[0])[0], Some(-135.));
    assert_eq!(axes(&rendered, instances[1])[0], Some(-135.));
    // Another function of the same channel has no Position role and is never inverted.
    let (unbound, _, _) = build("motor.base.index");
    assert!(unbound[0][0] >= 32768, "{unbound:?}");
    assert_eq!(unbound[1][0], unbound[0][0]);
    assert_eq!(unbound[2][0], unbound[0][0]);
}

#[test]
fn explicit_raw_alias_values_keep_bypassing_installation_inversion() {
    for (value, expected) in [
        (AttributeValue::RawDmxExact(12345), 12345),
        (AttributeValue::RawDmx(32), 8224),
    ] {
        let mut fixture = mover();
        alias(&mut fixture);
        let target = fixture.fixture_id;
        let (fixture, instances) = installed(fixture);
        let (engine, programmers, session) = engine(fixture);
        for name in [PAN_ALIAS, TILT_ALIAS] {
            programmers.set(session, target, AttributeKey(name.into()), value.clone());
        }
        let (captured, _) = capture_and_render(&engine, target, &instances);
        for words in &captured {
            assert_eq!(
                words,
                &[expected, expected],
                "explicit Raw is already native"
            );
        }
    }
}

#[test]
fn unbound_aliases_are_not_guessed_while_canonical_pan_tilt_keep_legacy_inversion() {
    // No physical Position model: only literal pan/tilt names keep the legacy inversion.
    let (mut fixture, target) = schema_v2_fixture(&[
        ("Pan", false, false, false, false, false),
        ("tilt", false, false, false, false, false),
        (PAN_ALIAS, false, false, false, false, false),
        ("pan.speed", false, false, false, false, false),
    ]);
    fixture.invert_pan = true;
    fixture.invert_tilt = true;
    let (engine, programmers, session) = engine(fixture);
    for name in ["Pan", "tilt", PAN_ALIAS, "pan.speed"] {
        set(&programmers, session, target, name, 0.2);
    }
    let rendered = engine.render(Default::default()).unwrap();
    assert_eq!(&rendered.universes[&1][0..4], &[204, 204, 51, 51]);
}

fn hold(engine: &Engine, fixture: &mut PatchedFixture) -> light_fixture::FrozenPositionOutput {
    let accepted = engine.render(Default::default()).unwrap();
    let native = engine
        .position_freeze_from_physical(accepted.generation, &accepted.physical, fixture.fixture_id)
        .unwrap();
    fixture.freeze.targets.insert(
        fixture.fixture_id,
        FrozenFixtureTarget {
            families: vec![FreezeFamily::Position],
            position_native: Some(native.clone()),
            ..Default::default()
        },
    );
    let mut snapshot = engine.snapshot().as_ref().clone();
    Arc::make_mut(&mut snapshot.fixtures)[0] = fixture.clone();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    native
}

#[test]
fn frozen_alias_words_of_inverted_copies_replay_without_second_inversion() {
    let mut fixture = mover();
    alias(&mut fixture);
    let target = fixture.fixture_id;
    let (mut fixture, instances) = installed(fixture);
    let (engine, programmers, session) = engine(fixture.clone());
    set(&programmers, session, target, PAN_ALIAS, 0.3);
    set(&programmers, session, target, TILT_ALIAS, 0.6);
    let held = hold(&engine, &mut fixture);
    let held_words: Vec<Vec<u32>> = held
        .instances
        .iter()
        .map(|instance| {
            instance
                .controls
                .iter()
                .map(|control| control.raw)
                .collect()
        })
        .collect();
    assert_eq!(held_words[0], [19661, 39321]);
    mirrored(held_words[0][0], held_words[1][0], 65535);
    mirrored(held_words[0][1], held_words[2][1], 65535);
    // Later programmer motion cannot move the hold, and the held copy words stay exact.
    set(&programmers, session, target, PAN_ALIAS, 0.9);
    set(&programmers, session, target, TILT_ALIAS, 0.1);
    let rendered = engine.render(Default::default()).unwrap();
    for ((instance, expected), address) in instances.iter().zip(&held_words).zip([1, 10, 20]) {
        assert_eq!(&words(&rendered, *instance), expected, "held native word");
        let encoded: Vec<u8> = expected
            .iter()
            .flat_map(|raw| (*raw as u16).to_be_bytes())
            .collect();
        assert_eq!(
            &rendered.universes[&1][address - 1..address + 3],
            encoded.as_slice()
        );
    }
}

#[test]
fn fitted_native_alias_words_are_not_inverted_on_inverted_copies() {
    let mut fixture = mover();
    alias(&mut fixture);
    let target = fixture.fixture_id;
    let (fixture, instances) = installed(fixture);
    let (engine, programmers, session) = engine(fixture.clone());
    programmers.set(
        session,
        target,
        ProgrammingOwner::Position.key(),
        AttributeValue::Position(Arc::new(PositionIntent::angles(0., 0.))),
    );
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut frame = engine.prepare_static_family_frame(&capture, &[]);
    frame
        .project_family(
            target,
            ProgrammingOwner::Position,
            AttributeValue::Position(Arc::new(PositionIntent::angles(0., 0.))),
            FamilyProjectionMetadata {
                changed_at: None,
                evidence: FamilyProjectionEvidence::PreserveBaseline,
                master: FamilyProjectionMaster::PreserveBaseline,
            },
        )
        .unwrap();
    let mode = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0];
    // Fitted words are already installation-native per instance (here deliberately distinct).
    let fitted = [[1000, 2000], [3000, 4000], [5000, 6000]];
    let writes: Vec<_> = instances
        .iter()
        .zip(fitted)
        .flat_map(|(instance_id, raw)| {
            mode.channels
                .iter()
                .enumerate()
                .map(move |(index, channel)| PositionNativeWrite {
                    target,
                    instance_id: *instance_id,
                    channel_index: index as u32,
                    channel_id: channel.id,
                    function_id: Some(channel.functions[0].id),
                    split: channel.split,
                    raw: raw[index],
                })
        })
        .collect();
    frame
        .project_position_native(&capture, &token, &writes)
        .unwrap();
    let rendered = engine.render_static_family_frame(&capture, frame).unwrap();
    for (instance, expected) in instances.iter().zip(fitted) {
        assert_eq!(
            words(&rendered, *instance),
            expected,
            "fitted word unchanged"
        );
    }
    assert_eq!(&rendered.universes[&1][9..13], &[11, 184, 15, 160]);
}
