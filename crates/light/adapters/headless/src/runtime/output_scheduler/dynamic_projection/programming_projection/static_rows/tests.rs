use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::color::tests::{
    magenta, program, warm_white,
};

use light_dynamics::{
    FamilyCompositionContext, FamilyExpressionOperation, RetainedFamilyCompositionScratch,
    WholeFamilyExpressionFrameResolver, compose_retained_dynamic_family_traced,
};

/// A frame that must never be consulted: composition without samples solves nothing.
struct UntouchedFrame;

impl WholeFamilyExpressionFrameResolver for UntouchedFrame {
    fn adopt_position_angles(
        &self,
        _original: &AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("a static-only composition adopted through the frame");
    }

    fn resolve(
        &self,
        _requirement: TransitionRequirement,
        _from: &AttributeValue,
        _to: &AttributeValue,
        _operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("a static-only composition resolved a frame transition");
    }
}

fn compose(
    base: &AttributeValue,
    scratch: &mut RetainedFamilyCompositionScratch,
) -> AttributeValue {
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        base,
        &[],
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        &UntouchedFrame,
        scratch,
    )
    .unwrap()
}

#[test]
fn a_kept_row_is_the_composition_of_its_base_and_recomposes_only_when_the_base_changes() {
    let mut rows = StaticFamilyRows::default();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let key = (FixtureId::new(), ProgrammingOwner::Color);
    let (first, second) = (program(&magenta()), program(&warm_white()));
    let mut compositions = 0;
    let mut row = |rows: &mut StaticFamilyRows, base: &AttributeValue| {
        let row = rows
            .row(key, base, &mut scratch, |scratch| {
                compositions += 1;
                Ok(compose(base, scratch))
            })
            .unwrap();
        (row.value.clone(), row.trace.root().is_some())
    };
    let kept = row(&mut rows, &first);
    assert_eq!(
        row(&mut rows, &first.clone()),
        kept,
        "an equal base is kept"
    );
    let mut fresh = RetainedFamilyCompositionScratch::default();
    assert_eq!(
        kept.0,
        compose(&first, &mut fresh),
        "kept equals a fresh composition"
    );
    assert!(kept.1, "the kept trace has the composition's root");
    let changed = row(&mut rows, &second);
    assert_eq!(changed.0, compose(&second, &mut fresh));
    drop(row);
    assert_eq!(compositions, 2);
}

#[test]
fn a_failed_composition_keeps_nothing_and_unused_rows_leave_with_their_cohort() {
    let mut rows = StaticFamilyRows::default();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let (used, unused) = (
        (FixtureId::new(), ProgrammingOwner::Color),
        (FixtureId::new(), ProgrammingOwner::Color),
    );
    let base = program(&magenta());
    let failed = rows.row(used, &base, &mut scratch, |_| {
        Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ))
    });
    assert!(failed.is_err());
    assert!(rows.is_empty());
    for key in [used, unused] {
        rows.row(key, &base, &mut scratch, |scratch| {
            Ok(compose(&base, scratch))
        })
        .unwrap();
    }
    rows.finish_cohort();
    rows.row(used, &base, &mut scratch, |_| panic!("kept"))
        .unwrap();
    rows.finish_cohort();
    assert!(rows.contains(&used));
    assert!(!rows.contains(&unused), "a row no cohort used is dropped");
}
