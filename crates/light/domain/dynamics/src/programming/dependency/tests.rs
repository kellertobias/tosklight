use super::*;
use crate::{
    DynamicFamilyRepresentation, DynamicSampleExpression, DynamicValue, RetainedExpressionTape,
};
use std::sync::Arc;
use uuid::Uuid;

fn focus(dependency: Option<DynamicSourceDependency>) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        }),
        value: DynamicValue::Scalar(0.6),
        occurrence: None,
        dependency_occurrence: dependency,
    }
}

#[test]
fn legacy_uuid_dependency_roundtrips_as_unknown_with_its_original_identity() {
    let id = DynamicSourceOccurrenceId::new(Uuid::from_u128(711)).unwrap();
    let mut legacy = serde_json::to_value(focus(None)).unwrap();
    legacy["dependency_occurrence"] = serde_json::json!(id);
    let restored: DynamicSampleExpression = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        restored,
        focus(Some(DynamicSourceDependency::unknown(Some(id))))
    );
    restored.validate().unwrap();
    let mut visited = vec![];
    restored
        .visit_source_occurrences(&mut |id| visited.push(id))
        .unwrap();
    assert_eq!(visited, vec![id]);
    let tape = RetainedExpressionTape::from_roots(&[Arc::new(restored.clone())]).unwrap();
    let mut legacy_tape = serde_json::to_value(&tape).unwrap();
    legacy_tape["nodes"][0]["dependency_occurrence"] = serde_json::json!(id);
    let tape: RetainedExpressionTape = serde_json::from_value(legacy_tape).unwrap();
    tape.validate().unwrap();
    let held = DynamicSampleExpression::Retained {
        root: tape.roots()[0],
        tape: Arc::new(tape),
    };
    assert_eq!(held, restored);
    let roundtrip: DynamicSampleExpression =
        serde_json::from_value(serde_json::to_value(&held).unwrap()).unwrap();
    assert_eq!(roundtrip, restored);
}

#[test]
fn unknown_identity_is_a_present_dependency_and_prevents_lossy_source_collapse() {
    let unknown = focus(Some(DynamicSourceDependency::unknown(None)));
    let absent = focus(None);
    assert_ne!(unknown, absent);
    assert!(unknown.has_source_occurrences());
    assert!(!absent.has_source_occurrences());
    let roundtrip: DynamicSampleExpression =
        serde_json::from_value(serde_json::to_value(&unknown).unwrap()).unwrap();
    assert_eq!(roundtrip, unknown);
    let mut ids = vec![];
    roundtrip
        .visit_source_occurrences(&mut |id| ids.push(id))
        .unwrap();
    assert!(ids.is_empty());
}

#[test]
fn retained_dependency_transfer_rejects_wrong_owner_and_nil_native_remap() {
    use ProgrammingTraceField as F;
    for remap in [
        [(F::Pan, F::Focus)],
        [(F::Focus, F::NativeColorChannel(Uuid::nil()))],
    ] {
        let expression = focus(Some(DynamicSourceDependency::mapped(
            None,
            ProgrammingFieldTransfer {
                identity: ProgrammingFieldScope::empty(),
                remap: remap.into(),
            },
        )));
        let restored: DynamicSampleExpression =
            serde_json::from_value(serde_json::to_value(&expression).unwrap()).unwrap();
        assert!(restored.validate().is_err());
        assert!(RetainedExpressionTape::from_roots(&[Arc::new(restored)]).is_err());
    }
    let valid = focus(Some(DynamicSourceDependency::mapped(
        None,
        ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([F::Focus]),
            remap: Arc::from([]),
        },
    )));
    valid.validate().unwrap();
    let nil_native = DynamicSourceDependency::mapped(
        None,
        ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::empty(),
            remap: vec![(F::NativeColorChannel(Uuid::nil()), F::ColorXyz)].into(),
        },
    );
    assert!(
        nil_native.validate(ProgrammingOwner::Color).is_err(),
        "nil native remaps are invalid even when both fields have the correct owner"
    );
}
