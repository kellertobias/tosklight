use light_core::{AttributeValue, FixtureId, NativeColorIdentity, programming::*};
use light_fixture::{ChannelFunctionBehavior, FixtureProfile};
use light_show::PortableShowCandidate;
use std::{cell::RefCell, collections::HashMap};

/// Cold storage validation uses the original portable profile, never the replacement lamp.
/// This verifies a raw template before promoting it to fallback; prediction remains in the
/// native model adapter. Missing profiles leave existing retained data untouched.
pub(in crate::show_compiler) struct NativeTemplateValidator<'a> {
    candidate: PortableShowCandidate<'a>,
    profiles: RefCell<HashMap<(uuid::Uuid, u32), Option<FixtureProfile>>>,
    identities: RefCell<Vec<(NativeColorIdentity, bool)>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{AttributeKey, NativeColorBinding, NativeColorValue, PhysicalDataQuality};
    use light_dynamics::{
        DynamicFamilyRepresentation, DynamicPresetGroupTemplate, DynamicPresetTemplate,
        DynamicValueAddress,
    };
    use light_fixture::{ChannelFunction, ColorPhysicalModel, HeadOpticalPath, OpticalSource};
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn fallback_requires_the_complete_original_native_path_and_valid_curve() {
        let mut profile = FixtureProfile::blank();
        profile.revision = 1;
        profile.manufacturer = "Test".into();
        profile.name = "Retained source".into();
        let mode = &mut profile.modes[0];
        let head = mode.heads[0].id;
        mode.channels = ["color.red", "color.uv"].map(|attribute| {
            serde_json::from_value(json!({
                "id": Uuid::new_v4(), "head_id":head, "split":1,
                "fixture_attribute":attribute, "attribute":attribute, "resolution":"u8", "default_raw":0, "highlight_raw":255,
                "functions":[ChannelFunction::continuous(attribute, AttributeKey(attribute.into()),255)]
            })).unwrap()
        }).into();
        mode.splits[0].footprint = 2;
        mode.color_physical = Some(ColorPhysicalModel {
            version: 1,
            revision: 1,
            paths: vec![HeadOpticalPath {
                id: Uuid::new_v4(),
                head_id: head,
                controls: mode.channels.iter().map(|channel| channel.id).collect(),
                source: OpticalSource::Unknown,
                filters: vec![],
                measurements: vec![],
            }],
        });
        let mode_id = mode.id;
        let source = profile.native_color_identity(mode_id, head).unwrap();
        let channels = profile.modes[0]
            .channels
            .iter()
            .map(|channel| NativeColorValue {
                channel_id: channel.id,
                function_id: channel.functions[0].id,
                raw: 100,
            })
            .collect::<Vec<_>>();
        let binding = NativeColorBinding {
            channel_id: channels[0].channel_id,
            function_id: channels[0].function_id,
        };
        let make = |recipe: NativeColorRecipe| {
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                recipe,
                portable: PortableColorEstimate {
                    model_revision: 1,
                    visible: None,
                    uv: None,
                    quality: PhysicalDataQuality::Estimated,
                    limitations: vec![],
                },
            }))
        };
        let recipe = NativeColorRecipe {
            source: source.clone(),
            channels,
            spreads: vec![NativeColorSpread {
                binding,
                points: vec![0, 255],
            }],
        };
        let valid = make(recipe.clone());
        let (store, _) =
            light_show::ShowStore::create(":memory:", "Native source retention").unwrap();
        store
            .insert_fixture_profile_revision(
                &light_show::FixtureProfileRevision::from_profile(
                    serde_json::to_value(profile).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        let document = store.portable_document().unwrap();
        let transaction = document.transaction();
        let validator = NativeTemplateValidator::new(document.candidate(&transaction).unwrap());
        assert!(validator.allows(&valid));
        let address = DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor { source },
            component: Some(ProgrammingComponent::NativeColor(binding)),
        };
        let mut original = DynamicPresetTemplate {
            groups: vec![DynamicPresetGroupTemplate {
                group_id: "front".into(),
                value: valid.clone(),
            }],
            ..Default::default()
        };
        original.retain_fallback_verified(&address, None, &|value| validator.allows(value));
        for changed in [
            {
                let mut r = recipe.clone();
                r.channels.pop();
                r
            },
            {
                let mut r = recipe.clone();
                r.channels[1].raw = 256;
                r
            },
            {
                let mut r = recipe.clone();
                r.spreads[0].points[1] = 256;
                r
            },
            {
                let mut r = recipe.clone();
                r.source.profile_digest = "wrong-original-profile".into();
                r
            },
        ] {
            let invalid = make(changed);
            assert!(!validator.allows(&invalid));
            let mut next = DynamicPresetTemplate {
                groups: vec![DynamicPresetGroupTemplate {
                    group_id: "front".into(),
                    value: invalid,
                }],
                ..Default::default()
            };
            next.retain_fallback_verified(&address, Some(&original), &|value| {
                validator.allows(value)
            });
            assert_eq!(next.fallback.as_ref().unwrap().groups[0].value, valid);
        }

        // Reopening or importing without the original profile must neither promote an
        // unverified recipe nor erase the last verified template (including every UV byte).
        let (missing_store, _) =
            light_show::ShowStore::create(":memory:", "Missing original").unwrap();
        let missing_document = missing_store.portable_document().unwrap();
        let missing_transaction = missing_document.transaction();
        let missing =
            NativeTemplateValidator::new(missing_document.candidate(&missing_transaction).unwrap());
        assert!(!missing.allows(&valid));
        let mut pending = DynamicPresetTemplate {
            groups: vec![DynamicPresetGroupTemplate {
                group_id: "front".into(),
                value: valid.clone(),
            }],
            ..Default::default()
        };
        pending.retain_fallback_verified(&address, Some(&original), &|value| missing.allows(value));
        let reopened: DynamicPresetTemplate =
            serde_json::from_value(serde_json::to_value(&pending).unwrap()).unwrap();
        assert_eq!(reopened.groups[0].value, valid);
        assert_eq!(reopened.fallback.as_ref().unwrap().groups[0].value, valid);
    }
}

impl<'a> NativeTemplateValidator<'a> {
    pub fn new(candidate: PortableShowCandidate<'a>) -> Self {
        Self {
            candidate,
            profiles: Default::default(),
            identities: Default::default(),
        }
    }

    pub fn allows(&self, value: &AttributeValue) -> bool {
        let AttributeValue::ColorProgram(program) = value else {
            return true;
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            return true;
        };
        let source = &recipe.source;
        let mut profiles = self.profiles.borrow_mut();
        let Some(profile) = profiles
            .entry((source.profile_id, source.profile_revision))
            .or_insert_with(|| {
                self.candidate
                    .fixture_profile_revision(
                        FixtureId(source.profile_id),
                        source.profile_revision.into(),
                    )
                    .and_then(|revision| serde_json::from_value(revision.profile().clone()).ok())
            })
            .as_ref()
        else {
            return false;
        };
        let mut identities = self.identities.borrow_mut();
        let verified = if let Some((_, verified)) =
            identities.iter().find(|(identity, _)| identity == source)
        {
            *verified
        } else {
            let verified = profile
                .native_color_identity(source.mode_id, source.head_id)
                .is_ok_and(|identity| identity == *source);
            identities.push((source.clone(), verified));
            verified
        };
        if !verified {
            return false;
        }
        let Some(mode) = profile.mode(source.mode_id) else {
            return false;
        };
        let Some(path) = mode.color_physical.as_ref().and_then(|model| {
            model
                .paths
                .iter()
                .find(|path| path.id == source.path_id && path.head_id == source.head_id)
        }) else {
            return false;
        };
        if mode
            .validate_native_color_recipe(path, &recipe.channels)
            .is_err()
        {
            return false;
        }
        recipe.spreads.iter().all(|spread| {
            let Some(channel) = mode
                .channels
                .iter()
                .find(|channel| channel.id == spread.binding.channel_id)
            else {
                return false;
            };
            let Some(function) = channel
                .functions
                .iter()
                .find(|function| function.id == spread.binding.function_id)
            else {
                return false;
            };
            spread
                .validate(NativeColorComponentDescriptor {
                    binding: spread.binding,
                    raw_from: function.dmx_from,
                    raw_to: function.dmx_to,
                    continuous: matches!(
                        function.behavior,
                        ChannelFunctionBehavior::Continuous { .. }
                    ) && function.dmx_from < function.dmx_to,
                })
                .is_ok()
        })
    }
}
