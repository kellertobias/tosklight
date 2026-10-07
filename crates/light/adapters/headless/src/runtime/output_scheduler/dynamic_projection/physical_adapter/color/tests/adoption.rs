//! TL-559 native descriptors, reference head, page overflow (AC7) and representation
//! adoption (AC4): only a real native edit turns Semantic into Direct, from the published
//! premaster writes of one frame; the first semantic edit adopts a Direct value once.
use super::super::native::*;
use super::super::profiles::*;
use super::super::tests::{intent, program, uv_only_black};
use super::super::*;
use super::direct::{DirectRig, direct, direct_program, identity, path_channels};
use light_core::programming::{
    ColorComponent, ComponentEdit, NativeColorEdit, ProgrammingComponent, ScalarEdit,
    VirtualColorAuthoringV1,
};
use light_core::{NativeColorBinding, programming::NativeColorRecipe};
use light_fixture::FixtureProfile;

fn nine_controls() -> FixtureProfile {
    additive(
        "TL-559 nine controls",
        &[
            ("color.white", Some(white())),
            ("color.amber", Some(amber())),
            ("color.lime", Some(xyz(0.40, 0.80, 0.10))),
            ("color.cyan", Some(xyz(0.20, 0.35, 0.60))),
            ("color.indigo", Some(xyz(0.10, 0.05, 0.50))),
            ("color.deep_red", Some(xyz(0.30, 0.12, 0.01))),
        ],
    )
}

fn binding(profile: &FixtureProfile, index: usize) -> NativeColorBinding {
    let channel = path_channels(profile)[index];
    NativeColorBinding {
        channel_id: channel.id,
        function_id: channel.functions[0].id,
    }
}

fn set(profile: &FixtureProfile, index: usize, raw: u32) -> ComponentEdit {
    ComponentEdit::Native {
        binding: binding(profile, index),
        operation: NativeColorEdit::Set(raw),
    }
}

fn recipe(value: &AttributeValue) -> &NativeColorRecipe {
    match direct_program(value).as_ref() {
        ColorProgram::Direct { recipe, .. } => recipe,
        ColorProgram::Semantic { .. } => panic!("expected Direct"),
    }
}

fn published(resolved: &super::direct::Resolved, value: &AttributeValue) -> Vec<u32> {
    let _ = value;
    resolved.result.writes.iter().map(|w| w.raw).collect()
}

/// AC7: descriptors come from the verified identity; pages 3/4 hold eight controls, the rest
/// overflow to the modal in path order; the reference head is the first eligible target.
#[test]
fn native_descriptors_pages_overflow_and_reference_head_come_from_verified_identity() {
    let profile = nine_controls();
    let rig = DirectRig::new(&profile, &[&profile]);
    let resolved = rig
        .resolve(&program(&intent([1., 0., 0.], 0.)), None)
        .unwrap();
    let descriptor = &resolved.descriptor;
    let head = descriptor.primary();
    assert_eq!(head.native.as_ref(), Some(&identity(&profile)));
    let pages = descriptor
        .native_pages(rig.target, head.head_id)
        .expect("verified head");
    assert_eq!((pages.pages.len(), pages.overflow.len()), (8, 1));
    let order: Vec<_> = pages
        .pages
        .iter()
        .chain(pages.overflow)
        .map(|c| c.channel_id)
        .collect();
    let expected: Vec<_> = path_channels(&profile).iter().map(|c| c.id).collect();
    assert_eq!(order, expected, "path order, nothing omitted");
    assert!(pages.pages.iter().all(|c| c.functions.len() == 1));
    assert_eq!(pages.pages[0].raw_max, 65535, "Red keeps its U16 width");

    // A head without a verified identity is not eligible as reference.
    let mut unverified = patched(&profile, FixtureId::new(), 1);
    unverified.definition.runtime_color_context = None;
    let other = rig.adapter.compile(
        &light_engine::EngineSnapshot {
            fixtures: vec![unverified.clone()].into(),
            ..Default::default()
        },
        unverified.fixture_id,
    );
    let other = other.unwrap().unwrap();
    assert!(other.primary().native.is_none());
    assert!(
        other
            .native_pages(unverified.fixture_id, head.head_id)
            .is_none()
    );
    let reference = reference_head([
        (unverified.fixture_id, &other),
        (rig.target, descriptor.as_ref()),
    ])
    .unwrap();
    assert_eq!(
        (reference.target, reference.head_id),
        (rig.target, head.head_id)
    );

    // Inspection is read-only: values carry descriptor-derived functions, nothing is adopted.
    let before = rig.adapter.counters();
    let values = inspect_native_values(
        head,
        &PublishedColorHead {
            token: &resolved.capture.frame_token(),
            target: rig.target,
            value: &program(&intent([1., 0., 0.], 0.)),
            writes: &resolved.result.writes,
        },
    )
    .unwrap();
    assert_eq!(values.len(), 9);
    assert!(values.iter().all(|v| !v.function_id.is_nil()));
    assert_eq!(rig.adapter.counters(), before);
}

/// AC4: the first real native edit captures the published premaster solution under its own
/// token, derives each function from the descriptor (parked writes have none), applies the
/// edit once and yields Direct; non-native edits never adopt; an existing Direct value of the
/// same source is edited in place; a mixed selection receives one shared value.
#[test]
fn only_a_real_native_edit_adopts_direct_from_the_published_premaster_writes() {
    let profile = rgbwauv(None);
    let rig = DirectRig::new(&profile, &[&profile]);
    let semantic = program(&uv_only_black());
    let resolved = rig.resolve(&semantic, None).unwrap();
    // Retained/parked writes may be published without a function: adoption derives every
    // function from the descriptor and the premaster raw value, never from the sidecar.
    let mut stripped = resolved.result.writes.clone();
    stripped
        .iter_mut()
        .filter(|w| w.raw == 0)
        .for_each(|w| w.function_id = None);
    assert!(stripped.iter().any(|w| w.function_id.is_none()));
    let head = resolved.descriptor.primary().head_id;
    let token = resolved.capture.frame_token();
    let seen = PublishedColorHead {
        token: &token,
        target: rig.target,
        value: &semantic,
        writes: &stripped,
    };
    let hue = ComponentEdit::Scalar {
        component: ProgrammingComponent::Color(ColorComponent::Hue),
        operation: ScalarEdit::Set(light_core::programming::ScalarIntent::Value(120.)),
    };
    for edits in [vec![], vec![hue.clone()]] {
        assert!(
            adopt_native_edit(&resolved.capture, &resolved.descriptor, seen, head, &edits).is_err(),
            "only a native edit adopts"
        );
    }
    let adopted = adopt_native_edit(
        &resolved.capture,
        &resolved.descriptor,
        seen,
        head,
        &[set(&profile, 0, 77)],
    )
    .unwrap();
    let first = recipe(&adopted);
    assert_eq!(first.source, identity(&profile));
    let mut expected = published(&resolved, &semantic);
    expected[0] = 77;
    let mut raws: Vec<_> = path_channels(&profile)
        .iter()
        .map(|c| {
            first
                .channels
                .iter()
                .find(|v| v.channel_id == c.id)
                .unwrap()
                .raw
        })
        .collect();
    assert_eq!(
        raws, expected,
        "premaster solution adopted, edit applied once"
    );
    let ColorProgram::Direct { portable, .. } = direct_program(&adopted).as_ref() else {
        unreachable!()
    };
    assert_eq!(
        portable.uv.map(|uv| uv.amount),
        Some(*expected.last().unwrap() as f32 / 255.),
        "independent UV knowledge preserved"
    );

    // Editing the Direct value again never reseeds it from the output.
    let replayed = rig.resolve(&adopted, None).unwrap();
    let token = replayed.capture.frame_token();
    let mut fake = replayed.result.writes.clone();
    fake.iter_mut().for_each(|w| w.raw = 0);
    let again = adopt_native_edit(
        &replayed.capture,
        &replayed.descriptor,
        PublishedColorHead {
            token: &token,
            target: rig.target,
            value: &adopted,
            writes: &fake,
        },
        head,
        &[set(&profile, 1, 5)],
    )
    .unwrap();
    raws = path_channels(&profile)
        .iter()
        .map(|c| {
            recipe(&again)
                .channels
                .iter()
                .find(|v| v.channel_id == c.id)
                .unwrap()
                .raw
        })
        .collect();
    expected[1] = 5;
    assert_eq!(raws, expected);

    // A write whose published function disagrees with the descriptor is never adopted.
    let mut wrong = resolved.result.writes.clone();
    wrong[1].function_id = Some(Uuid::new_v4());
    assert!(
        adopt_native_edit(
            &resolved.capture,
            &resolved.descriptor,
            PublishedColorHead {
                writes: &wrong,
                ..seen
            },
            head,
            &[set(&profile, 0, 1)],
        )
        .is_err()
    );

    // Mixed selection: one capture and edit on the reference, one shared tagged value.
    let others = [rig.target, FixtureId::new(), FixtureId::new()];
    let assigned = adopt_native_edit_for_selection(
        &resolved.capture,
        (&resolved.descriptor, seen, head),
        &others,
        &[set(&profile, 0, 77)],
    )
    .unwrap();
    assert_eq!(assigned.len(), 3);
    for (_, value) in &assigned {
        assert!(Arc::ptr_eq(
            direct_program(value),
            direct_program(&assigned[0].1)
        ));
        assert_eq!(recipe(value), recipe(&adopted));
    }
}

/// AC4/AC8: the first semantic edit of a Direct value adopts its forward estimate once
/// (relative output, black and UV kept; unknown appearance needs an explicit start) and a
/// semantic value is edited without adoption.
#[test]
fn first_semantic_edit_adopts_direct_once_through_the_complete_family_operation() {
    let profile = rgbwauv(None);
    let rig = DirectRig::new(&profile, &[&profile]);
    let catalogue = Arc::clone(&rig.catalogue.borrow());
    let saturation = |value: f32| ComponentEdit::Scalar {
        component: ProgrammingComponent::Color(ColorComponent::RelativeOutput),
        operation: ScalarEdit::Set(light_core::programming::ScalarIntent::Value(value)),
    };
    // UV-only black: visible known black (UV off leaks nothing), UV known.
    let black = direct(&catalogue, &profile, &[0, 0, 0, 0, 0, 0]);
    let (value, adoption) = adopt_semantic_edit(
        &black,
        &[saturation(1.)],
        None,
        catalogue.as_ref(),
        &VirtualColorAuthoringV1,
    )
    .unwrap();
    let adoption = adoption.expect("Direct adopted once");
    assert_eq!(adoption.intent.base_xyz.y, 0.);
    let AttributeValue::ColorProgram(result) = &value else {
        unreachable!()
    };
    let ColorProgram::Semantic { intent } = result.as_ref() else {
        panic!("semantic after adoption")
    };
    assert_eq!(intent.base_xyz.y, 0., "black stays black");
    assert_eq!(intent.uv.amount, 0.);

    // Unknown appearance (UV on with unknown leakage) needs an explicit starting Color.
    let unknown = direct(&catalogue, &profile, &[0, 0, 0, 0, 0, 128]);
    assert!(
        adopt_semantic_edit(
            &unknown,
            &[saturation(1.)],
            None,
            catalogue.as_ref(),
            &VirtualColorAuthoringV1
        )
        .is_err()
    );
    let start = intent_with_output(0.4);
    let (value, adoption) = adopt_semantic_edit(
        &unknown,
        &[saturation(0.4)],
        Some(&start),
        catalogue.as_ref(),
        &VirtualColorAuthoringV1,
    )
    .unwrap();
    let adoption = adoption.unwrap();
    assert!(adoption.visible_from_start);
    let AttributeValue::ColorProgram(result) = &value else {
        unreachable!()
    };
    let ColorProgram::Semantic { intent } = result.as_ref() else {
        panic!("semantic")
    };
    assert_eq!(
        intent.uv.amount,
        128. / 255.,
        "known UV adopted independently"
    );
    assert_eq!(intent.relative_output, 0.4);

    // A semantic value is edited directly; nothing is adopted.
    let (_, none) = adopt_semantic_edit(
        &program(&intent_with_output(1.)),
        &[saturation(0.5)],
        None,
        catalogue.as_ref(),
        &VirtualColorAuthoringV1,
    )
    .unwrap();
    assert!(none.is_none());
    let native = ComponentEdit::Native {
        binding: binding(&profile, 0),
        operation: NativeColorEdit::Set(1),
    };
    assert!(
        adopt_semantic_edit(
            &black,
            &[native],
            None,
            catalogue.as_ref(),
            &VirtualColorAuthoringV1
        )
        .is_err()
    );
}

fn intent_with_output(relative_output: f32) -> ColorIntent {
    ColorIntent {
        relative_output,
        ..intent([1., 0.5, 0.], 0.)
    }
}
