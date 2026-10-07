use super::*;
use std::sync::Arc;
use uuid::Uuid;

fn angles(points: Vec<f32>) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: if points.len() == 1 {
            ScalarIntent::Value(points[0])
        } else {
            ScalarIntent::Spread(points)
        },
        tilt_degrees: ScalarIntent::Value(0.0),
    }))
}
fn address() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    }
}
fn source(
    template: DynamicPresetTemplate,
    targets: Vec<FixtureId>,
) -> DynamicInstancePresetSources {
    DynamicInstancePresetSources {
        instance_id: Uuid::new_v4(),
        dependency_generation: Uuid::new_v4(),
        ordered_targets: targets,
        last_valid: vec![],
        sources: vec![Arc::new(DynamicPresetSourceBinding {
            id: Uuid::new_v4(),
            preset_id: "3.1".into(),
            address: address(),
            retained: Some(Arc::new(template)),
            occurrence: Some(DynamicPresetSourceOccurrence {
                lane_id: Uuid::new_v4(),
                slot: DynamicPresetSourceSlot::Maximum,
            }),
        })],
    }
}
fn values(
    instance: &DynamicInstancePresetSources,
    groups: &HashMap<String, GroupDefinition>,
) -> HashMap<FixtureId, DynamicValue> {
    compile_dynamic_preset_sources(instance, groups, &HashMap::new(), None)
        .unwrap()
        .values[0]
        .values
        .iter()
        .map(|value| (value.target, value.value.clone()))
        .collect()
}

#[test]
fn universal_spread_uses_each_instance_order_and_groups_keep_full_ranks_and_precedence() {
    let a = FixtureId::new();
    let b = FixtureId::new();
    let c = FixtureId::new();
    let template = DynamicPresetTemplate {
        universal: Some(angles(vec![-720.0, 720.0])),
        ..Default::default()
    };
    let first = source(template.clone(), vec![a, b]);
    let second = source(template, vec![b, a, c]);
    assert_eq!(
        values(&first, &HashMap::new())[&a],
        DynamicValue::Scalar(-720.0)
    );
    assert_eq!(
        values(&second, &HashMap::new())[&a],
        DynamicValue::Scalar(0.0)
    );
    let group = |id: &str| GroupDefinition {
        id: id.into(),
        fixtures: vec![a, b, c],
        ..Default::default()
    };
    let groups = [("a".into(), group("a")), ("z".into(), group("z"))].into();
    let template = DynamicPresetTemplate {
        universal: Some(angles(vec![900.0])),
        fixtures: vec![DynamicPresetFixtureTemplate {
            fixture_id: c,
            value: angles(vec![800.0]),
        }],
        groups: vec![
            DynamicPresetGroupTemplate {
                group_id: "z".into(),
                value: angles(vec![-180.0, 180.0]),
            },
            DynamicPresetGroupTemplate {
                group_id: "a".into(),
                value: angles(vec![-360.0, 360.0]),
            },
        ],
        fallback: None,
    };
    let result = values(&source(template, vec![c, b]), &groups);
    assert_eq!(result[&c], DynamicValue::Scalar(180.0));
    assert_eq!(result[&b], DynamicValue::Scalar(0.0));
}

#[test]
fn incompatible_group_members_use_retained_intent_and_new_members_use_the_template_after_reload() {
    let a = FixtureId::new();
    let b = FixtureId::new();
    let c = FixtureId::new();
    let old = DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: angles(vec![-360.0, 360.0]),
        }],
        ..Default::default()
    };
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0, 2.0, 3.0],
    )));
    let mut latest = DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
                owner: ProgrammingOwner::Position,
                template: target.clone(),
                members: [(b.0, angles(vec![42.0]))].into(),
            })),
        }],
        ..Default::default()
    };
    latest.retain_fallback(&address(), Some(&old));
    // Only the saved semantic source remains; there is no live Preset or previous c value.
    let saved = serde_json::to_vec(&latest).unwrap();
    let retained = serde_json::from_slice(&saved).unwrap();
    let groups = [(
        "front".into(),
        GroupDefinition {
            id: "front".into(),
            fixtures: vec![a, b, c],
            ..Default::default()
        },
    )]
    .into();
    let manifest = source(retained, vec![b, c]);
    let result = values(&manifest, &groups);
    assert_eq!(result[&b], DynamicValue::Scalar(42.0));
    assert_eq!(result[&c], DynamicValue::Scalar(360.0));
    let mut absent = manifest.clone();
    absent.last_valid = compile_dynamic_preset_sources(&manifest, &groups, &HashMap::new(), None)
        .unwrap()
        .values;
    Arc::make_mut(&mut absent.sources[0]).retained =
        Some(Arc::new(DynamicPresetTemplate::default()));
    assert_eq!(
        values(&absent, &groups),
        result,
        "an existing instance keeps its exact last valid values"
    );
}

#[test]
fn removing_a_fixture_exception_does_not_let_old_values_override_new_universal_intent() {
    let a = FixtureId::new();
    let old = DynamicPresetTemplate {
        universal: Some(angles(vec![10.0])),
        fixtures: vec![DynamicPresetFixtureTemplate {
            fixture_id: a,
            value: angles(vec![80.0]),
        }],
        ..Default::default()
    };
    let mut latest = DynamicPresetTemplate {
        universal: Some(angles(vec![30.0])),
        ..Default::default()
    };
    latest.retain_fallback(&address(), Some(&old));
    assert!(latest.fallback.as_ref().unwrap().fixtures.is_empty());
    let mut manifest = source(latest, vec![a]);
    manifest.last_valid = vec![DynamicPresetSourceValues {
        occurrence: manifest.sources[0].occurrence.unwrap(),
        preset_id: "3.1".into(),
        address: address(),
        values: vec![DynamicValueFallback {
            target: a,
            value: DynamicValue::Scalar(80.0),
        }],
    }];
    assert_eq!(
        values(&manifest, &HashMap::new())[&a],
        DynamicValue::Scalar(30.0)
    );
}

struct NativeModel {
    identity: light_core::NativeColorIdentity,
    bindings: [light_core::NativeColorBinding; 2],
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &light_core::NativeColorIdentity {
        &self.identity
    }
    fn descriptor(
        &self,
        binding: light_core::NativeColorBinding,
    ) -> Option<NativeColorComponentDescriptor> {
        self.bindings
            .contains(&binding)
            .then_some(NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: u32::MAX,
                continuous: true,
            })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        if recipe.source != self.identity
            || recipe.channels.len() != 2
            || self.bindings.iter().any(|binding| {
                !recipe.channels.iter().any(|value| {
                    value.channel_id == binding.channel_id
                        && value.function_id == binding.function_id
                })
            })
        {
            return Err(IntentError("incomplete original native path".into()));
        }
        Ok(PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: light_core::PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}
struct Models(Arc<NativeModel>);
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &light_core::NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        if source != &self.0.identity {
            return Err(IntentError("only the pinned original source exists".into()));
        }
        Ok(self.0.clone())
    }
}

#[test]
fn native_group_source_preserves_low_bits_and_complete_verified_fallback_for_new_members() {
    let model = Arc::new(NativeModel {
        identity: light_core::NativeColorIdentity {
            profile_id: Uuid::new_v4(),
            profile_revision: 1,
            profile_digest: "original-source".into(),
            mode_id: Uuid::new_v4(),
            head_id: Uuid::new_v4(),
            path_id: Uuid::new_v4(),
            model_revision: 1,
            native_layout_signature: "original-layout".into(),
        },
        bindings: [0, 1].map(|_| light_core::NativeColorBinding {
            channel_id: Uuid::new_v4(),
            function_id: Uuid::new_v4(),
        }),
    });
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: model.identity.clone(),
        },
        component: Some(ProgrammingComponent::NativeColor(model.bindings[0])),
    };
    let recipe = NativeColorRecipe {
        source: model.identity.clone(),
        channels: model
            .bindings
            .iter()
            .map(|binding| light_core::NativeColorValue {
                channel_id: binding.channel_id,
                function_id: binding.function_id,
                raw: u32::MAX - 4,
            })
            .collect(),
        spreads: vec![NativeColorSpread {
            binding: model.bindings[0],
            points: vec![u32::MAX - 4, u32::MAX],
        }],
    };
    let portable = model.predict(&recipe).unwrap();
    let mut previous = DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                recipe: recipe.clone(),
                portable: portable.clone(),
            })),
        }],
        ..Default::default()
    };
    let verify = |value: &AttributeValue| {
        let AttributeValue::ColorProgram(color) = value else {
            return false;
        };
        let ColorProgram::Direct { recipe, .. } = color.as_ref() else {
            return false;
        };
        model.predict(recipe).is_ok()
    };
    previous.retain_fallback_verified(&address, None, &verify);
    let mut invalid = recipe;
    invalid.channels.pop();
    let mut latest = DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                recipe: invalid,
                portable,
            })),
        }],
        ..Default::default()
    };
    latest.retain_fallback_verified(&address, Some(&previous), &verify);
    let a = FixtureId::new();
    let b = FixtureId::new();
    let new = FixtureId::new();
    let groups = [(
        "front".into(),
        GroupDefinition {
            id: "front".into(),
            fixtures: vec![a, b, new],
            ..Default::default()
        },
    )]
    .into();
    let mut manifest = source(latest, vec![b, new]);
    Arc::make_mut(&mut manifest.sources[0]).address = address;
    let result =
        compile_dynamic_preset_sources(&manifest, &groups, &HashMap::new(), Some(&Models(model)))
            .unwrap();
    assert_eq!(
        result.values[0].values[0].value,
        DynamicValue::Native(u32::MAX - 2)
    );
    assert_eq!(
        result.values[0].values[1].value,
        DynamicValue::Native(u32::MAX)
    );
    assert_eq!(
        result.unavailable.len(),
        2,
        "the invalid current recipe remains inspectable while retained output continues"
    );
    assert!(
        result.issues.iter().all(|issue| matches!(
            issue.reason,
            DynamicPresetSourceIssueReason::InvalidValue {
                fallback: false,
                ..
            }
        )),
        "a verified incomplete recipe is not an expected missing-model capability"
    );
}

mod native_capability;
