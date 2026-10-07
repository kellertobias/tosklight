use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicFamilySourceProjection, DynamicFamilySourceUnknown,
};
use light_core::programming::ProgrammingTraceField as Field;

fn color() -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }))
}

fn red_group(target: FixtureId, origins: &mut DynamicSourceOrigins) -> DynamicFamilySampleGroup {
    let rank = FamilySampleRank {
        priority: 100,
        changed_at_millis: 1000,
        changed_at_submillis_nanos: 0,
        stable_order: 1,
        identity: light_dynamics::FamilySampleIdentity::Dynamic {
            instance_id: Uuid::new_v4(),
            controller_id: Uuid::new_v4(),
            lane_id: Uuid::new_v4(),
        },
    };
    let occurrence = origins
        .bind(
            DynamicSourceBinding::Authored {
                instance_id: rank.dynamic_identity().unwrap().instance_id,
                controller_id: rank.dynamic_identity().unwrap().controller_id,
                target,
                lane_id: rank.dynamic_identity().unwrap().lane_id,
            },
            DynamicSourceOrigin::Playback {
                identity: PlaybackIdentity::physical(5).unwrap(),
                activated_at: chrono::DateTime::from_timestamp_millis(1000).unwrap(),
            },
        )
        .unwrap();
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Recipe,
        },
        component: Some(ProgrammingComponent::Color(ColorComponent::Red)),
    };
    let compiled = Arc::new(CompiledDynamicValueAddress::new(address.clone(), None).unwrap());
    let expression = Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(address),
        value: DynamicValue::Scalar(0.2),
        occurrence: Some(occurrence),
        dependency_occurrence: None,
    });
    let expression = Arc::new(CompiledComponentExpression::new(expression, compiled).unwrap());
    DynamicFamilySampleGroup {
        target,
        owner: ProgrammingOwner::Color,
        samples: vec![
            FamilySample::retained_component(expression, rank, 1.0)
                .unwrap()
                .into(),
        ],
    }
}

#[test]
fn captured_composition_projects_base_and_dynamic_from_one_value_and_retains_observations() {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    let target = FixtureId::new();
    programmers.start(session);
    programmers.set(session, target, ProgrammingOwner::Color.key(), color());
    let engine = Engine::with_programming_contract_support(
        programmers.clone(),
        PROGRAMMING_CONTRACT_VERSION,
    );
    let capture = engine.prepare_output_frame(Default::default());
    let sources = TickSources::prepared(&engine, &capture, &[]);
    let mut origins = DynamicSourceOrigins::default();
    let group = red_group(target, &mut origins);
    let typed = CapturedProgrammingSources::new(&sources, &no_adoption, None)
        .with_source_transaction(&mut origins);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let context = FamilyCompositionContext {
        edit: FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        },
        ..Default::default()
    };
    let calls = Cell::new(0);
    // Neither lazy source observation nor provenance may observe this later edit.
    clock.advance_millis(100);
    programmers.clear(session);
    let (value, red, green) = typed
        .compose_family(
            &group,
            &context,
            &NoFrameConversion,
            &mut scratch,
            |observation| {
                calls.set(calls.get() + 1);
                let mut projection = DynamicFamilySourceProjection::default();
                observation.project_fields(
                    &ProgrammingFieldScope::new([Field::ColorRecipeRed]),
                    &mut projection,
                )?;
                let red = projection.clone();
                observation.project_fields(
                    &ProgrammingFieldScope::new([Field::ColorRecipeGreen]),
                    &mut projection,
                )?;
                Ok((observation.value().clone(), red, projection))
            },
        )
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(sources.values.get().is_some());
    assert_eq!(red.entries().unwrap().len(), 1);
    assert!(red.entries().unwrap()[0].static_source().is_none());
    assert!(matches!(
        red.entries().unwrap()[0].record().origin,
        DynamicSourceOrigin::Playback { .. }
    ));
    let base = green.entries().unwrap();
    assert_eq!(base.len(), 1);
    assert_eq!(
        base[0]
            .static_source()
            .unwrap()
            .changed_at
            .timestamp_millis(),
        1000
    );
    assert_eq!(
        base[0].fields(),
        &ProgrammingFieldScope::new([Field::ColorRecipeGreen])
    );
    assert!(
        matches!(&value, AttributeValue::ColorProgram(program) if matches!(program.as_ref(), ColorProgram::Semantic { intent } if (intent.recipe.rgb[0] - 0.2).abs() < 1e-6))
    );
    let empty = DynamicFamilySampleGroup {
        target,
        owner: ProgrammingOwner::Color,
        samples: Vec::new(),
    };
    typed
        .compose_family(
            &empty,
            &context,
            &NoFrameConversion,
            &mut scratch,
            |observation| {
                assert_eq!(observation.value(), &color());
                Ok(())
            },
        )
        .unwrap();
    typed.finish_source_bindings().unwrap();
    drop(typed);
    assert_eq!(red.entries().unwrap()[0].role(), FamilyTraceRole::Authored);
    assert_eq!(
        green.entries().unwrap()[0].fields(),
        &ProgrammingFieldScope::new([Field::ColorRecipeGreen])
    );
    assert!(
        origins
            .get(green.entries().unwrap()[0].record().occurrence_id)
            .is_some()
    );
}

#[test]
fn composing_without_a_catalogue_keeps_successful_values_and_unknown_source_status() {
    struct Source {
        target: FixtureId,
        value: AttributeValue,
    }
    impl ScalarSourceResolver for Source {
        fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
            None
        }
        fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
            None
        }
    }
    impl DynamicTickSource for Source {
        fn value(&self, target: FixtureId, key: &AttributeKey) -> Option<&AttributeValue> {
            (target == self.target && *key == ProgrammingOwner::Color.key()).then_some(&self.value)
        }
    }
    let source = Source {
        target: FixtureId::new(),
        value: color(),
    };
    let typed = CapturedProgrammingSources::new(&source, &no_adoption, None);
    let group = DynamicFamilySampleGroup {
        target: source.target,
        owner: ProgrammingOwner::Color,
        samples: Vec::new(),
    };
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let result = typed
        .compose_family(
            &group,
            &Default::default(),
            &NoFrameConversion,
            &mut scratch,
            |observation| {
                let mut projection = DynamicFamilySourceProjection::default();
                observation
                    .project_fields(&ProgrammingFieldScope::new([Field::Uv]), &mut projection)?;
                assert_eq!(observation.value(), &source.value);
                Ok(projection)
            },
        )
        .unwrap();
    assert_eq!(result.unknown(), Some(DynamicFamilySourceUnknown::Identity));
    assert!(result.entries().is_none());
}
