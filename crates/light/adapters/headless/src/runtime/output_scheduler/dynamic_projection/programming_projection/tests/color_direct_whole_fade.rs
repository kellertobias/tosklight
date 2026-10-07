//! TL-600: whole Color FixAT fades into a Direct recipe whose source identity this fixture
//! cannot supply. The actual Live FixAT activation (`composition::apply_whole`) routes the
//! original from/to Transition through the pinned frame resolver, which is the Color adapter's
//! captured portable-appearance `transition`. Every frame keeps ONE Color owner through the
//! engine finalizer; completion restores the exact requested Direct recipe (fallback replay).
use super::super::super::physical_adapter::color::DirectReplayOutcome;
use super::super::super::physical_adapter::color::profiles::{rgbal, rgbw, rgbwauv};
use super::super::super::physical_adapter::color::tests::direct::{catalogue, direct};
use super::super::super::physical_adapter::color::tests::intent;
use super::super::super::physical_adapter::*;
use super::color_direct_transition::{Rig, is_semantic, semantic_value, uv_amount};
use super::*;

/// The portable semantic appearance a Direct endpoint stands for: the recorded (= captured
/// original) estimate through the shared core adoption.
fn portable(value: &AttributeValue) -> AttributeValue {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("Color program")
    };
    match program.as_ref() {
        ColorProgram::Semantic { .. } => value.clone(),
        ColorProgram::Direct { portable, .. } => {
            semantic_value(&semantic_color_adoption(portable, None).unwrap().intent)
        }
    }
}

/// Fade `from` → `to` with the shared core complete-family interpolation.
fn expected(from: &AttributeValue, to: &AttributeValue, progress: f32) -> AttributeValue {
    interpolate_programming_value(&portable(from), &portable(to), progress).unwrap()
}

fn visible_sum(rig: &Rig, result: &PhysicalHeadResult<ColorAdapter>) -> u32 {
    let uv = rig.profile.modes[0].channels.last().unwrap().id;
    result
        .writes
        .iter()
        .filter(|write| write.channel_id != uv)
        .map(|write| write.raw)
        .sum()
}

/// Drive one 1 s whole FixAT fade into `destination` and check three interior samples against
/// the core interpolation of the two portable endpoints, then the exact tagged completion.
fn fade_through_appearance(
    rig: &mut Rig,
    lane: &PhysicalAdapterLane<ColorAdapter>,
    underlay: &AttributeValue,
    destination: &AttributeValue,
) -> Vec<PhysicalHeadResult<ColorAdapter>> {
    rig.fade_to(destination.clone(), 1_000);
    sample_appearance_fade(rig, lane, underlay, destination)
}

/// Sample a whole 1 s fade into `destination` that started at the current clock (a Live FixAT
/// or an actual Cue GO): three interior samples, then the exact tagged completion.
pub(super) fn sample_appearance_fade(
    rig: &mut Rig,
    lane: &PhysicalAdapterLane<ColorAdapter>,
    underlay: &AttributeValue,
    destination: &AttributeValue,
) -> Vec<PhysicalHeadResult<ColorAdapter>> {
    let transitions = lane.adapter().counters().representation_transitions;
    let mut interior = Vec::new();
    for (step, progress) in [0.25f32, 0.5, 0.75].into_iter().enumerate() {
        let mid = rig.typed(250, lane);
        assert!(rig.requirements.is_empty(), "step {step}: no passive hold");
        assert!(is_semantic(&mid.value), "step {step}: portable appearance");
        assert_eq!(
            mid.value,
            expected(underlay, destination, progress),
            "step {step}: shared appearance interpolation of the ORIGINAL endpoints"
        );
        assert!(mid.quality.direct.is_none(), "no native replay claimed");
        assert!(
            mid.provenance.sources.entries().is_none(),
            "step {step}: appearance blend is an unknown transfer, not exact attribution"
        );
        interior.push(mid);
    }
    let counters = lane.adapter().counters();
    assert_eq!(counters.representation_transitions, transitions + 3);
    let done = rig.typed(250, lane);
    assert_eq!(&done.value, destination, "exact tagged Direct destination");
    assert!(
        done.provenance
            .sources
            .entries()
            .is_some_and(|entries| !entries.is_empty()),
        "the completed whole write is exactly attributed again"
    );
    assert!(matches!(
        done.quality.direct.as_ref().unwrap().replay,
        DirectReplayOutcome::Fallback { .. }
    ));
    interior.push(done);
    interior
}

/// Semantic → foreign Direct(B) with known appearance interpolates at every interior sample,
/// restores B exactly at completion and Release reveals the static owner.
#[test]
fn semantic_to_foreign_direct_fade_interpolates_known_portable_appearance() {
    let foreign = rgbal();
    let base = semantic_value(&intent([1., 0., 0.], 0.));
    let mut rig = Rig::new(|_, _| base.clone(), &[&foreign]);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let blue = direct(&rig.catalogue, &foreign, &[0, 0, 255, 0, 0]);
    let results = fade_through_appearance(&mut rig, &lane, &base, &blue);
    // The red write falls monotonically while blue rises: a real fade, not a step.
    let red = |r: &PhysicalHeadResult<ColorAdapter>| r.writes[0].raw;
    let blue_raw = |r: &PhysicalHeadResult<ColorAdapter>| r.writes[2].raw;
    for pair in results[..3].windows(2) {
        assert!(red(&pair[1]) < red(&pair[0]) && blue_raw(&pair[1]) > blue_raw(&pair[0]));
    }
    rig.release();
    for _ in 0..4 {
        if let Some(result) = rig.frame(250, &lane) {
            assert_eq!(result.value, base);
        }
    }
    assert_eq!(rig.last_static, Some(base));
}

/// Direct(A) → Direct(B), both foreign to this fixture: the original A is the from endpoint
/// (never an adopted copy) and B is restored exactly.
#[test]
fn foreign_direct_a_to_foreign_direct_b_interpolates_and_restores_b() {
    let a = rgbw();
    let b = rgbal();
    let from = std::cell::RefCell::new(None);
    let mut rig = Rig::new(
        |catalogue, _| {
            let value = direct(catalogue, &a, &[65535, 0, 0, 0]);
            *from.borrow_mut() = Some(value.clone());
            value
        },
        &[&a, &b],
    );
    let from = from.into_inner().unwrap();
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let to = direct(&rig.catalogue, &b, &[0, 255, 0, 0, 0]);
    fade_through_appearance(&mut rig, &lane, &from, &to);
}

/// A's original model is not in this generation's catalogue: its valid recorded estimate is the
/// from appearance (the composer never blocks on the missing model, nor invents one).
#[test]
fn missing_original_model_uses_the_valid_recorded_estimate() {
    let a = rgbw();
    let b = rgbal();
    let from = direct(&catalogue(&[&a]), &a, &[0, 0, 0, 255]);
    let mut rig = Rig::new(|_, _| from.clone(), &[&b]);
    assert!(
        rig.catalogue
            .resolve(&super::super::super::physical_adapter::color::tests::direct::identity(&a))
            .is_err(),
        "A is absent from the generation"
    );
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let to = direct(&rig.catalogue, &b, &[0, 0, 255, 0, 0]);
    fade_through_appearance(&mut rig, &lane, &from, &to);
}

/// Unknown visible appearance (UV with unknown leakage on the foreign source) is an explicit
/// adapter hold of the original underlay — never a fabricated smooth fade or a native recipe of
/// the foreign layout — and the destination replaces it exactly at completion.
#[test]
fn unknown_visible_appearance_holds_the_original_underlay_until_completion() {
    let foreign = rgbwauv(None);
    let base = semantic_value(&intent([1., 0., 0.], 0.));
    let mut rig = Rig::new(|_, _| base.clone(), &[&foreign]);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let unknown = direct(&rig.catalogue, &foreign, &[0, 0, 0, 0, 0, 128]);
    rig.fade_to(unknown.clone(), 1_000);
    for step in 0..3 {
        let held = rig.typed(250, &lane);
        assert!(rig.requirements.is_empty());
        assert_eq!(held.value, base, "step {step}: explicit hold of the source");
    }
    assert_eq!(lane.adapter().counters().representation_holds, 3);
    let done = rig.typed(250, &lane);
    assert_eq!(done.value, unknown);
    assert!(matches!(
        done.quality.direct.as_ref().unwrap().replay,
        DirectReplayOutcome::Fallback { .. }
    ));
}

/// Native Direct(P) with full UV → foreign Direct(B) without UV: UV fades between the two
/// endpoint amounts and is zero at completion, never kept from the previous endpoint.
#[test]
fn uv_fades_to_zero_and_never_leaks_from_the_previous_endpoint() {
    let foreign = rgbal();
    let mut rig = Rig::new(
        |catalogue, profile| direct(catalogue, profile, &[200, 40, 0, 0, 0, 255]),
        &[&foreign],
    );
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let from = rig.last_static.clone().unwrap();
    let to = direct(&rig.catalogue, &foreign, &[0, 0, 255, 0, 0]);
    let results = fade_through_appearance(&mut rig, &lane, &from, &to);
    let mut previous = 1.;
    for (step, result) in results[..3].iter().enumerate() {
        let uv = uv_amount(&result.value).unwrap();
        assert!(uv < previous && uv > 0., "step {step}: UV fades out ({uv})");
        previous = uv;
        let expected = (f64::from(uv) * 255.).round() as u32;
        assert!(rig.uv_write(result).abs_diff(expected) <= 1);
    }
    assert_eq!(rig.uv_write(&results[3]), 0, "previous UV never leaks");
}

/// Relative output and known black: a reduced red fades into a foreign known-black recipe by
/// appearance (visible drive falls monotonically) and ends dark, exactly the requested recipe.
#[test]
fn reduced_output_fades_to_a_foreign_known_black_recipe() {
    let foreign = rgbal();
    let base = semantic_value(&ColorIntent {
        relative_output: 0.5,
        ..intent([1., 0., 0.], 0.)
    });
    let mut rig = Rig::new(|_, _| base.clone(), &[&foreign]);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let black = direct(&rig.catalogue, &foreign, &[0, 0, 0, 0, 0]);
    let results = fade_through_appearance(&mut rig, &lane, &base, &black);
    let mut previous = u32::MAX;
    for (step, result) in results[..3].iter().enumerate() {
        let sum = visible_sum(&rig, result);
        assert!(
            sum < previous && sum > 0,
            "step {step}: dims by appearance ({sum})"
        );
        previous = sum;
    }
    assert_eq!(visible_sum(&rig, &results[3]), 0, "known black stays black");
    assert_eq!(rig.uv_write(&results[3]), 0);
}

/// Release during the foreign fade reveals the static owner at once; there is one Color owner
/// every frame and no requirement is left behind.
#[test]
fn release_during_a_foreign_fade_reveals_the_static_owner() {
    let foreign = rgbal();
    let base = semantic_value(&intent([0., 1., 0.], 0.));
    let mut rig = Rig::new(|_, _| base.clone(), &[&foreign]);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    assert!(rig.frame(10, &lane).is_none());
    let to = direct(&rig.catalogue, &foreign, &[0, 0, 255, 0, 0]);
    rig.fade_to(to.clone(), 1_000);
    let mid = rig.typed(400, &lane);
    assert_eq!(mid.value, expected(&base, &to, 0.4));
    rig.release();
    for _ in 0..4 {
        if let Some(result) = rig.frame(250, &lane) {
            assert_eq!(result.value, base);
        }
        assert!(rig.requirements.is_empty());
    }
    assert_eq!(rig.last_static, Some(base));
}
