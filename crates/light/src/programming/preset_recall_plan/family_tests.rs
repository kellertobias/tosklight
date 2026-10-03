use super::*;
use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality};
use std::{collections::BTreeMap, sync::Arc};

struct NativeModel {
    identity: NativeColorIdentity,
    binding: NativeColorBinding,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.identity
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        if recipe.source != self.identity || recipe.channels.len() != 1 {
            return Err(IntentError(
                "prediction requires the complete original source".into(),
            ));
        }
        let channel = &recipe.channels[0];
        if (channel.channel_id, channel.function_id)
            != (self.binding.channel_id, self.binding.function_id)
        {
            return Err(IntentError("native binding changed".into()));
        }
        Ok(PortableColorEstimate {
            model_revision: self.identity.model_revision,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.75,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![format!("original raw {}", channel.raw)],
        })
    }
}
struct Models(Arc<NativeModel>);
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        if source != &self.0.identity {
            return Err(IntentError("original source unavailable".into()));
        }
        Ok(self.0.clone())
    }
}
struct MismatchedModels(Arc<NativeModel>);
impl DynamicNativeModelResolver for MismatchedModels {
    fn resolve(
        &self,
        _: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        Ok(self.0.clone())
    }
}
fn native_model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        identity: NativeColorIdentity {
            profile_id: uuid::Uuid::new_v4(),
            profile_revision: 3,
            profile_digest: "immutable-original-source".into(),
            mode_id: uuid::Uuid::new_v4(),
            head_id: uuid::Uuid::new_v4(),
            path_id: uuid::Uuid::new_v4(),
            model_revision: 4,
            native_layout_signature: "one-u32-uv-channel".into(),
        },
        binding: NativeColorBinding {
            channel_id: uuid::Uuid::new_v4(),
            function_id: uuid::Uuid::new_v4(),
        },
    })
}
fn native_spread(model: &NativeModel) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.identity.clone(),
            channels: vec![NativeColorValue {
                channel_id: model.binding.channel_id,
                function_id: model.binding.function_id,
                raw: u32::MAX - 2,
            }],
            spreads: vec![NativeColorSpread {
                binding: model.binding,
                points: vec![u32::MAX - 2, u32::MAX],
            }],
        },
        portable: PortableColorEstimate {
            model_revision: 4,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }))
}

#[test]
fn native_universal_spread_uses_original_source_and_predicts_each_exact_u32_and_uv() {
    let original = native_model();
    let targets = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let preset = Preset {
        family: light_programmer::PresetFamily::Color,
        number: 1,
        universal_values: HashMap::from([(
            ProgrammingOwner::Color.key(),
            native_spread(&original),
        )]),
        ..Default::default()
    };
    let models = Models(original.clone());
    let planned = materialize_preset_fixture_values_with_native_models(
        &preset,
        &targets,
        &HashMap::new(),
        &HashMap::new(),
        &models,
    )
    .unwrap();
    assert_eq!(planned.len(), targets.len());
    for (rank, mutation) in planned.iter().enumerate() {
        let NormalProgrammerValueMutation::SetFixture {
            fixture_id, value, ..
        } = mutation
        else {
            panic!("one fixture write per selection rank");
        };
        assert_eq!(*fixture_id, targets[rank]);
        let AttributeValue::ColorProgram(program) = value else {
            panic!("Direct Color result");
        };
        let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
            panic!("native recipe result");
        };
        let raw = u32::MAX - 2 + rank as u32;
        assert_eq!(recipe.source, original.identity);
        assert_eq!(recipe.channels[0].raw, raw);
        assert!(recipe.spreads.is_empty());
        assert_eq!(portable.uv.unwrap().amount, 0.75);
        assert_eq!(portable.quality, PhysicalDataQuality::Estimated);
        assert_eq!(portable.limitations, vec![format!("original raw {raw}")]);
    }
    assert!(
        materialize_preset_fixture_values(&preset, &targets, &HashMap::new(), &HashMap::new(),)
            .is_err(),
        "the legacy wrapper keeps its missing-model behavior"
    );

    let unavailable = Models(Arc::new(NativeModel {
        identity: NativeColorIdentity {
            profile_id: uuid::Uuid::new_v4(),
            ..original.identity.clone()
        },
        binding: original.binding,
    }));
    assert!(
        materialize_preset_fixture_values_with_native_models(
            &preset,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &unavailable,
        )
        .unwrap()
        .is_empty(),
        "empty selection must not require any native source resolution"
    );
    assert!(
        materialize_preset_fixture_values_with_native_models(
            &preset,
            &targets,
            &HashMap::new(),
            &HashMap::new(),
            &unavailable,
        )
        .is_err(),
        "a destination-only resolver cannot replace the original"
    );
    assert!(
        materialize_preset_fixture_values_with_native_models(
            &preset,
            &targets,
            &HashMap::new(),
            &HashMap::new(),
            &MismatchedModels(unavailable.0),
        )
        .is_err(),
        "a resolver returning a different source identity must fail"
    );
}

#[test]
fn native_group_spread_materializes_only_selected_members_in_full_group_rank_domain() {
    let original = native_model();
    let a = FixtureId::new();
    let b = FixtureId::new();
    let c = FixtureId::new();
    let preset = Preset {
        family: light_programmer::PresetFamily::Color,
        number: 2,
        group_values: HashMap::from([(
            "original-group".into(),
            HashMap::from([(ProgrammingOwner::Color.key(), native_spread(&original))]),
        )]),
        ..Default::default()
    };
    let groups = HashMap::from([(
        "original-group".into(),
        light_programmer::GroupDefinition {
            id: "original-group".into(),
            fixtures: vec![a, b, c],
            ..Default::default()
        },
    )]);
    let planned = materialize_preset_fixture_values_with_native_models(
        &preset,
        &[c, b],
        &groups,
        &HashMap::new(),
        &Models(original),
    )
    .unwrap();
    assert_eq!(planned.len(), 2);
    for (mutation, expected) in planned.iter().zip([u32::MAX, u32::MAX - 1]) {
        let NormalProgrammerValueMutation::SetFixture { value, .. } = mutation else {
            panic!()
        };
        let AttributeValue::ColorProgram(program) = value else {
            panic!()
        };
        let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
            panic!()
        };
        assert_eq!(recipe.channels[0].raw, expected);
        assert_eq!(
            portable.limitations,
            vec![format!("original raw {expected}")]
        );
    }
}
#[test]
fn group_preset_materializes_members_in_selection_order_and_retains_full_group_ranks() {
    let a = FixtureId::new();
    let b = FixtureId::new();
    let c = FixtureId::new();
    let position = ProgrammingOwner::Position.key();
    let template = AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: ScalarIntent::Spread(vec![-360.0, 360.0]),
        tilt_degrees: ScalarIntent::Value(0.0),
    }));
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0, 2.0, 3.0],
    )));
    let assignment = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
        owner: ProgrammingOwner::Position,
        template,
        members: BTreeMap::from([(b.0, target.clone())]),
    }));
    let preset = Preset {
        group_values: HashMap::from([(
            "1".into(),
            HashMap::from([(position.clone(), assignment.clone())]),
        )]),
        ..Default::default()
    };
    let groups = HashMap::from([(
        "1".into(),
        light_programmer::GroupDefinition {
            id: "1".into(),
            fixtures: vec![a, b, c],
            ..Default::default()
        },
    )]);
    let planned =
        materialize_preset_fixture_values(&preset, &[c, b], &groups, &HashMap::new()).unwrap();
    let values = planned
        .iter()
        .map(|mutation| match mutation {
            NormalProgrammerValueMutation::SetFixture {
                fixture_id, value, ..
            } => (*fixture_id, value.clone()),
            _ => panic!("fixture value expected"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            (
                c,
                AttributeValue::Position(Arc::new(PositionIntent::angles(360.0, 0.0)))
            ),
            (b, target)
        ]
    );
    let mut invalid = preset.clone();
    invalid
        .universal_values
        .insert(position.clone(), assignment.clone());
    assert!(invalid.validate_programming().is_err());
    invalid = preset.clone();
    invalid
        .values
        .insert(a, HashMap::from([(position, assignment)]));
    assert!(invalid.validate_programming().is_err());
}
#[test]
fn fixture_preset_rejects_unsampled_curves_but_universal_materializes_them() {
    let a = FixtureId::new();
    let b = FixtureId::new();
    let key = ProgrammingOwner::Position.key();
    let value = AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: ScalarIntent::Spread(vec![-360.0, 360.0]),
        tilt_degrees: ScalarIntent::Value(0.0),
    }));
    let mut preset = Preset {
        values: HashMap::from([(a, HashMap::from([(key.clone(), value.clone())]))]),
        ..Default::default()
    };
    assert!(preset.validate_programming().is_err());
    preset.values.clear();
    preset.universal_values.insert(key, value);
    assert!(preset.validate_programming().is_ok());
    let values =
        materialize_preset_fixture_values(&preset, &[b, a], &HashMap::new(), &HashMap::new())
            .unwrap();
    for (mutation, expected) in values.iter().zip([-360.0, 360.0]) {
        let NormalProgrammerValueMutation::SetFixture { value, .. } = mutation else {
            unreachable!()
        };
        assert_eq!(
            value,
            &AttributeValue::Position(Arc::new(PositionIntent::angles(expected, 0.0)))
        );
    }
}
