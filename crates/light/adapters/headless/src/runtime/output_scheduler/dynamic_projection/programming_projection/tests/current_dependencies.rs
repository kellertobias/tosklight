use super::*;
use light_engine::{
    ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint,
    ContributionFamilyRole, ContributionSourceId,
};

struct FamilySource {
    target: FixtureId,
    owner: ProgrammingOwner,
    value: AttributeValue,
    evidence: Arc<ContributionFamilyEvidence>,
}

impl FamilySource {
    fn new(owner: ProgrammingOwner, value: AttributeValue) -> Self {
        Self {
            target: FixtureId::new(),
            owner,
            value,
            evidence: Arc::new(ContributionFamilyEvidence::new(vec![
                ContributionFamilyEntry::new(
                    ContributionSourceId::programmer(light_core::ProgrammerId::new()),
                    light_core::ProgrammerEditStamp {
                        changed_at: chrono::Utc::now(),
                        programmer_order: 9,
                    },
                    ContributionFamilyFootprint::Whole,
                    ContributionFamilyRole::Authored,
                ),
            ])),
        }
    }
}

impl DynamicTickSource for FamilySource {
    fn value(&self, target: FixtureId, key: &AttributeKey) -> Option<&AttributeValue> {
        (target == self.target && *key == self.owner.key()).then_some(&self.value)
    }
    fn family_evidence(
        &self,
        target: FixtureId,
        key: &AttributeKey,
    ) -> Option<&Arc<ContributionFamilyEvidence>> {
        self.value(target, key).map(|_| &self.evidence)
    }
}
impl ScalarSourceResolver for FamilySource {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

#[test]
fn adopted_current_keeps_original_occurrence_but_cannot_claim_an_identity_transfer() {
    let source = FamilySource::new(ProgrammingOwner::Position, target_source().family);
    let calls = Cell::new(0);
    let adopt = |_, _: &AttributeValue, _: &DynamicValueAddress| {
        calls.set(calls.get() + 1);
        Ok(position(450., -30.))
    };
    let mut origins = DynamicSourceOrigins::default();
    let captured = CapturedProgrammingSources::new(&source, &adopt, None)
        .with_source_transaction(&mut origins);
    let address = axis(ProgrammingComponent::Pan);
    let original = captured
        .current_family_occurrence(source.target, &address)
        .unwrap();
    for _ in 0..3 {
        let dependency = captured.current_dependency(source.target, &address);
        assert_eq!(dependency.occurrence, Some(original));
        assert!(matches!(
            dependency.transfer,
            DynamicSourceTransfer::Unknown
        ));
        assert_eq!(
            captured.current(source.target, &address),
            Some(DynamicValue::Scalar(450.))
        );
    }
    assert_eq!(
        calls.get(),
        1,
        "trace lookup must reuse the same physical conversion"
    );
    assert_eq!(
        captured.current_family_base(source.target, &address),
        Some(source.value.clone())
    );
    captured.check().unwrap();
    captured.finish_source_bindings().unwrap();
    drop(captured);
    assert!(origins.get(original).is_some());
}

#[test]
fn compatible_color_current_tracks_recipe_reads_and_preserves_unknown_identity() {
    use ProgrammingTraceField as Field;
    let source = FamilySource::new(
        ProgrammingOwner::Color,
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent::default(),
        })),
    );
    let mut origins = DynamicSourceOrigins::default();
    let captured = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_source_transaction(&mut origins);
    let mut original = None;
    for (component, fields) in [
        (ColorComponent::Red, vec![Field::ColorRecipeRed]),
        (
            ColorComponent::Hue,
            vec![
                Field::ColorRecipeRed,
                Field::ColorRecipeGreen,
                Field::ColorRecipeBlue,
            ],
        ),
        (
            ColorComponent::Saturation,
            vec![
                Field::ColorRecipeRed,
                Field::ColorRecipeGreen,
                Field::ColorRecipeBlue,
            ],
        ),
    ] {
        let address = DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: if component == ColorComponent::Red {
                    DynamicSemanticColorBasis::Recipe
                } else {
                    DynamicSemanticColorBasis::HueSaturation
                },
            },
            component: Some(ProgrammingComponent::Color(component)),
        };
        assert!(captured.current(source.target, &address).is_some());
        let dependency = captured.current_dependency(source.target, &address);
        assert_eq!(
            *original.get_or_insert(dependency.occurrence.unwrap()),
            dependency.occurrence.unwrap()
        );
        let DynamicSourceTransfer::Mapped(transfer) = dependency.transfer else {
            panic!("component read must describe its actual recipe inputs")
        };
        let repeated = captured.current_dependency(source.target, &address);
        let DynamicSourceTransfer::Mapped(repeated) = repeated.transfer else {
            panic!()
        };
        assert!(
            Arc::ptr_eq(&transfer.remap, &repeated.remap),
            "repeated Current queries reuse the compiled field mapping"
        );
        assert_eq!(
            transfer.reverse(&ProgrammingFieldScope::new([Field::ColorXyz])),
            ProgrammingFieldScope::new(fields)
        );

        // A compatible value can exist without source history. Preserve that dependency as
        // present with unknown identity, rather than dropping it as if Current were unused.
        let unbound = CapturedProgrammingSources::new(&source, &no_adoption, None);
        let unknown = unbound.current_dependency(source.target, &address);
        assert!(unknown.occurrence.is_none());
        assert!(matches!(unknown.transfer, DynamicSourceTransfer::Mapped(_)));
    }
    captured.check().unwrap();
}
