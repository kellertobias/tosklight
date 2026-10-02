//! Verified fallback for an invalid authored native recipe, compiled through the candidate's
//! own original-model resolver.
use super::*;
use light_core::programming::{
    ColorProgram, IntentError, NativeColorComponentDescriptor, NativeColorEditModel,
    NativeColorRecipe, NativeColorSpread, PortableColorEstimate,
};
use light_core::{NativeColorBinding, NativeColorIdentity, NativeColorValue};
use light_dynamics::DynamicNativeModelResolver;

struct NativeModel {
    identity: NativeColorIdentity,
    bindings: [NativeColorBinding; 2],
}

impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.identity
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
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
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        if source != &self.0.identity {
            return Err(IntentError("only the pinned original source exists".into()));
        }
        Ok(self.0.clone())
    }
}

#[test]
fn invalid_authored_recipe_uses_verified_fallback_and_stays_reported_as_invalid() {
    let model = Arc::new(NativeModel {
        identity: NativeColorIdentity {
            profile_id: Uuid::new_v4(),
            profile_revision: 1,
            profile_digest: "original-source".into(),
            mode_id: Uuid::new_v4(),
            head_id: Uuid::new_v4(),
            path_id: Uuid::new_v4(),
            model_revision: 1,
            native_layout_signature: "original-layout".into(),
        },
        bindings: [0, 1].map(|_| NativeColorBinding {
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
            .map(|binding| NativeColorValue {
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
    let direct = |recipe: NativeColorRecipe| DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                recipe,
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
    let mut previous = direct(recipe.clone());
    previous.retain_fallback_verified(&address, None, &verify);
    let mut invalid = recipe;
    invalid.channels.pop();
    let mut latest = direct(invalid);
    latest.retain_fallback_verified(&address, Some(&previous), &verify);

    let [a, b, new] = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut definition = pan_definition(&[b, new], DynamicPresetTemplate::default());
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: address.clone(),
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Value {
                value: DynamicValue::Native(0),
            },
            maximum: DynamicValueSource::Preset {
                preset_id: "4.2".into(),
                address,
                last_valid_by_target: Vec::new(),
                retained: Some(Arc::new(latest.clone())),
            },
            function: PeriodicFunction::LinearUp,
            size: 1.0,
            pwm: PwmShape::default(),
        }),
    });
    let group = GroupDefinition {
        id: "front".into(),
        name: "front".into(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: vec![a, b, new],
        }),
        ..Default::default()
    };
    let show = snapshot(vec![group], []);
    let mut live = DynamicRuntime::with_native_color_models(
        light_core::programming::PROGRAMMING_CONTRACT_VERSION,
        Arc::new(Models(model)),
    );
    live.install_definitions([definition.clone()]).unwrap();
    let instance = start(&mut live, &definition);

    let mut candidate = live.fork_for_cold_install();
    let report = materialize_cold_preset_dependencies(&show, &show, &mut candidate).unwrap();

    let installed = candidate
        .preset_source_instances()
        .into_iter()
        .find(|manifest| manifest.instance_id == instance)
        .unwrap();
    let values = installed.last_valid[0]
        .values
        .iter()
        .map(|value| (value.target, value.value.clone()))
        .collect::<HashMap<_, _>>();
    assert_eq!(values[&b], DynamicValue::Native(u32::MAX - 2));
    assert_eq!(values[&new], DynamicValue::Native(u32::MAX));
    assert!(!report.source_quality.is_empty());
    assert!(report.source_quality.iter().all(|quality| matches!(
        quality.issue.reason,
        DynamicPresetSourceIssueReason::InvalidValue {
            fallback: false,
            ..
        }
    )));
    // The rejected authored recipe is not promoted into the retained source template.
    assert_eq!(installed.sources[0].retained.as_deref(), Some(&latest));
    assert!(live.preset_source_instances()[0].last_valid.is_empty());
}
