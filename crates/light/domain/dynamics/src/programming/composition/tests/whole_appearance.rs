//! TL-600: a whole Color FixAT mask whose Direct destination cannot be adopted on this fixture
//! (`Requires(ColorAppearance)`) is sampled by the pinned frame resolver, from the ORIGINAL
//! underlay to the ORIGINAL requested whole. Nothing else is rescued.
use super::*;
use crate::{
    FamilyCompositionSample, FamilyExpressionOperation, RetainedFamilyCompositionScratch,
    WholeFamilyExpressionFrameResolver, compose_retained_dynamic_family,
    compose_retained_dynamic_family_traced,
};
use std::cell::RefCell;

type Call = (TransitionRequirement, AttributeValue, AttributeValue, f32);

/// Records every call; answers with a fixed value (or the from/to endpoint) per operation.
struct AppearanceFrame {
    calls: RefCell<Vec<Call>>,
    answer: Box<
        dyn Fn(&AttributeValue, &AttributeValue, f32) -> Result<AttributeValue, TransitionError>,
    >,
}
impl AppearanceFrame {
    fn new(
        answer: impl Fn(
            &AttributeValue,
            &AttributeValue,
            f32,
        ) -> Result<AttributeValue, TransitionError>
        + 'static,
    ) -> Self {
        Self {
            calls: RefCell::default(),
            answer: Box::new(answer),
        }
    }
}
impl WholeFamilyExpressionFrameResolver for AppearanceFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        let FamilyExpressionOperation::Transition { progress } = operation else {
            return Err(TransitionError::Requires(requirement));
        };
        self.calls
            .borrow_mut()
            .push((requirement, from.clone(), to.clone(), progress));
        (self.answer)(from, to, progress)
    }
}

fn model_with(profile: u128) -> Arc<NativeModel> {
    let mut model = Arc::try_unwrap(native_model()).ok().unwrap();
    model.source.profile_id = Uuid::from_u128(profile);
    model.source.profile_digest = format!("profile-{profile}");
    Arc::new(model)
}

fn whole_mask(value: AttributeValue, model: Option<Arc<NativeModel>>, mix: f32) -> FamilySample {
    let address = DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap();
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                address,
                model.map(|model| model as Arc<dyn NativeColorEditModel + Send + Sync>),
            )
            .unwrap(),
        ),
        DynamicValue::Family(value),
        color(ColorComponent::Red, 0.0, 9).rank,
        mix,
    )
    .unwrap()
    .into_fix_at()
}

fn rgb_semantic(red: f32, blue: f32) -> AttributeValue {
    let mut intent = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut intent, ColorComponent::Red, red)
        .unwrap();
    VirtualColorAuthoringV1
        .set_base_component(&mut intent, ColorComponent::Blue, blue)
        .unwrap();
    semantic(intent)
}

fn run(
    base: &AttributeValue,
    samples: &[FamilySample],
    adoption: &FamilyAdoptionResolver<'_>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> Result<AttributeValue, TransitionError> {
    let samples = samples
        .iter()
        .cloned()
        .map(FamilyCompositionSample::from)
        .collect::<Vec<_>>();
    compose_retained_dynamic_family(
        ProgrammingOwner::Color,
        base,
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            resolve_adoption: Some(adoption),
            ..Default::default()
        },
        frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
}

fn foreign(_: &AttributeValue, _: &DynamicValueAddress) -> Result<AttributeValue, TransitionError> {
    Err(TransitionError::Requires(
        TransitionRequirement::ColorAppearance,
    ))
}

/// Semantic → foreign Direct(B) and Direct(A) → foreign Direct(B): the resolver receives the
/// original underlay, the original requested whole and the activation progress, at several
/// interior samples. Its interpolated result is the composed owner.
#[test]
fn foreign_direct_whole_fade_is_sampled_by_the_frame_from_the_original_endpoints() {
    let a = model_with(11);
    let b = model_with(12);
    let red = rgb_semantic(1.0, 0.0);
    let direct_a = semantic_to_direct(&a, [40, 50]);
    let direct_b = semantic_to_direct(&b, [200, 7]);
    for underlay in [red.clone(), direct_a.clone()] {
        for progress in [0.25f32, 0.5, 0.75] {
            let frame =
                AppearanceFrame::new(|_, _, progress| Ok(rgb_semantic(1.0 - progress, progress)));
            let result = run(
                &underlay,
                &[whole_mask(direct_b.clone(), Some(b.clone()), progress)],
                &foreign,
                &frame,
            )
            .unwrap();
            assert_eq!(result, rgb_semantic(1.0 - progress, progress));
            let calls = frame.calls.borrow();
            let [(requirement, from, to, sampled)] = calls.as_slice() else {
                panic!("one appearance sample per frame")
            };
            assert_eq!(*requirement, TransitionRequirement::ColorAppearance);
            assert_eq!(from, &underlay, "original underlay, never an adopted copy");
            assert_eq!(to, &direct_b, "original requested destination");
            assert_eq!(*sampled, progress);
        }
    }
    // Completion is the exact requested destination; the frame is not consulted.
    let frame = AppearanceFrame::new(|_, _, _| panic!("completion never asks the frame"));
    assert_eq!(
        run(
            &red,
            &[whole_mask(direct_b.clone(), Some(b), 1.0)],
            &foreign,
            &frame
        )
        .unwrap(),
        direct_b
    );
}

/// The resolver's explicit hold (unknown appearance) and its exact endpoints are accepted; any
/// other Direct result would be a fabricated recipe of a foreign layout and is rejected.
#[test]
fn frame_results_may_hold_or_restore_endpoints_but_never_fabricate_a_direct_recipe() {
    let b = model_with(12);
    let direct_a = semantic_to_direct(&model_with(11), [40, 50]);
    let direct_b = semantic_to_direct(&b, [200, 7]);
    let mask = || [whole_mask(direct_b.clone(), Some(b.clone()), 0.5)];
    let hold = AppearanceFrame::new(|from, _, _| Ok(from.clone()));
    assert_eq!(run(&direct_a, &mask(), &foreign, &hold).unwrap(), direct_a);
    let to = AppearanceFrame::new(|_, to, _| Ok(to.clone()));
    assert_eq!(run(&direct_a, &mask(), &foreign, &to).unwrap(), direct_b);
    let fabricated_recipe = semantic_to_direct(&b, [100, 3]);
    let fabricated = AppearanceFrame::new(move |_, _, _| Ok(fabricated_recipe.clone()));
    assert!(matches!(
        run(&direct_a, &mask(), &foreign, &fabricated),
        Err(TransitionError::Invalid(_))
    ));
    let other_owner = AppearanceFrame::new(|_, _, _| Ok(angles(0.0, 0.0)));
    assert!(matches!(
        run(&direct_a, &mask(), &foreign, &other_owner),
        Err(TransitionError::Invalid(_))
    ));
    // A passive frame keeps the passive requirement visible to the caller.
    let passive = AppearanceFrame::new(|_, _, _| {
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance,
        ))
    });
    assert_eq!(
        run(&direct_a, &mask(), &foreign, &passive),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    );
}

/// Only `Requires(ColorAppearance)` from adoption routes to the frame. Invalid adoption,
/// missing models and other requirements reach the caller unchanged; a successful same-source
/// adoption keeps the native interpolation and never asks the frame.
#[test]
fn only_color_appearance_adoption_is_rescued_and_native_adoption_is_unchanged() {
    let b = model_with(12);
    let direct_b = semantic_to_direct(&b, [200, 8]);
    let red = rgb_semantic(1.0, 0.0);
    let mask = || [whole_mask(direct_b.clone(), Some(b.clone()), 0.5)];
    let frame = AppearanceFrame::new(|_, _, _| panic!("must not be rescued"));
    for error in [
        TransitionError::Requires(TransitionRequirement::NativeColorModel),
        TransitionError::Requires(TransitionRequirement::MaterializedEndpoints),
        IntentError("malformed adoption".into()).into(),
    ] {
        let failing = |_: &AttributeValue, _: &DynamicValueAddress| Err(error.clone());
        assert_eq!(run(&red, &mask(), &failing, &frame), Err(error.clone()));
    }
    // Same-source native adoption: interpolated by the native model, frame untouched.
    let adopted = semantic_to_direct(&b, [0, 0]);
    let same_source = |_: &AttributeValue, _: &DynamicValueAddress| Ok(adopted.clone());
    let result = run(&red, &mask(), &same_source, &frame).unwrap();
    assert_eq!(result, semantic_to_direct(&b, [100, 4]));
    assert!(frame.calls.borrow().is_empty());
}

/// Position masks and Color masks over a non-Color underlay are never routed through the
/// Color appearance seam.
#[test]
fn non_color_whole_masks_keep_their_requirement() {
    let frame = AppearanceFrame::new(|_, _, _| panic!("Position is not Color appearance"));
    let mut mask = sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(angles(10.0, 20.0)),
        5,
    )
    .into_fix_at();
    mask.activation_mix = 0.5;
    let base = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0; 3],
    )));
    let result = compose_retained_dynamic_family(
        ProgrammingOwner::Position,
        &base,
        &[mask.into()],
        &FamilyCompositionContext::default(),
        &frame,
        &mut RetainedFamilyCompositionScratch::default(),
    );
    assert_eq!(
        result,
        Err(TransitionError::Requires(
            TransitionRequirement::LiveJointAngles
        ))
    );
}

/// Provenance: a frame-sampled crossing is an appearance blend with unknown field transfer,
/// not an exact Whole write of the mask's native components.
#[test]
fn frame_sampled_crossing_is_traced_as_an_unknown_transfer_not_exact_attribution() {
    let b = model_with(12);
    let direct_b = semantic_to_direct(&b, [200, 7]);
    let red = rgb_semantic(1.0, 0.0);
    let frame = AppearanceFrame::new(|_, _, progress| Ok(rgb_semantic(1.0 - progress, progress)));
    let samples = [FamilyCompositionSample::from(whole_mask(
        direct_b,
        Some(b),
        0.5,
    ))];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        &red,
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            resolve_adoption: Some(&foreign),
            ..Default::default()
        },
        &frame,
        &mut scratch,
    )
    .unwrap();
    let trace = scratch.family_trace();
    let root = trace.root().unwrap();
    assert_eq!(
        trace.sources_for_component(root, ProgrammingComponent::Color(ColorComponent::Red)),
        None,
        "appearance interpolation has no exact native-component attribution"
    );
}
