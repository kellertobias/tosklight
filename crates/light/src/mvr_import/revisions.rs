//! Immutable profile-revision reservation shared by Control and Architect.
use super::MvrDefinitions;
use crate::{ActionError, ActionErrorKind};
use light_fixture::{FixtureProfile, fixture_profile_content_digest};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;
pub type MvrProfileSlots = BTreeMap<(Uuid, u64), BTreeSet<String>>;
fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}
pub fn mvr_profile_identity(profile: serde_json::Value) -> Result<String, ActionError> {
    // Match immutable library publication: compare through the current compatibility reader
    // so legacy defaults and optical-field migrations do not create false conflicts. Current
    // calibration, notes, retained source and its verification state remain part of identity.
    let profile: FixtureProfile =
        serde_json::from_value(profile).map_err(|error| invalid(error.to_string()))?;
    let mut profile = serde_json::to_value(profile).map_err(|error| invalid(error.to_string()))?;
    profile
        .as_object_mut()
        .ok_or_else(|| invalid("invalid profile object"))?
        .remove("revision");
    fixture_profile_content_digest(&profile).map_err(|error| invalid(error.to_string()))
}

fn typed_identity(profile: &FixtureProfile) -> Result<String, ActionError> {
    mvr_profile_identity(serde_json::to_value(profile).map_err(|error| invalid(error.to_string()))?)
}

pub fn reserve_mvr_profiles(
    staged: &mut MvrDefinitions,
    slots: &mut MvrProfileSlots,
    resolutions: &HashMap<Uuid, super::MvrImportResolution>,
) -> Result<(), ActionError> {
    // Native and installed snapshots have immutable revisions. Never reinterpret an existing
    // native calibration/source identity to make a conflicting profile appear compatible.
    for (uuid, definition) in &staged.definitions {
        if staged.imported_profiles.contains_key(uuid)
            || matches!(
                resolutions.get(uuid),
                Some(super::MvrImportResolution::Skip)
            )
        {
            continue;
        }
        if let Some(profile) = definition.profile_snapshot.as_deref() {
            let key = (profile.id.0, u64::from(profile.revision));
            let digest = typed_identity(profile)?;
            let existing = slots.entry(key).or_default();
            if existing.iter().any(|value| value != &digest) {
                return Err(conflict(format!(
                    "MVR profile {} revision {} conflicts with an existing immutable profile; resolve the profile identity before importing",
                    key.0, key.1
                )));
            }
            existing.insert(digest);
        }
    }
    let used = staged
        .imported_profiles
        .iter()
        .filter(|(uuid, _)| {
            !matches!(
                resolutions.get(uuid),
                Some(super::MvrImportResolution::Skip)
            )
        })
        .map(|(_, index)| *index)
        .collect::<BTreeSet<_>>();
    let mut published = Vec::new();
    for index in used {
        let profile = &mut staged.new_profiles[index];
        let digest = typed_identity(profile)?;
        let existing = slots
            .iter()
            .find(|((id, revision), contents)| {
                *id == profile.id.0
                    && *revision > 0
                    && contents.len() == 1
                    && contents.contains(&digest)
            })
            .map(|((_, revision), _)| *revision);
        let revision = match existing {
            Some(revision) => revision,
            None => slots
                .keys()
                .filter(|(id, _)| *id == profile.id.0)
                .map(|(_, revision)| *revision)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| conflict("fixture profile revision space exhausted"))?,
        };
        profile.revision = u32::try_from(revision)
            .map_err(|_| conflict("fixture profile revision space exhausted"))?;
        slots
            .entry((profile.id.0, revision))
            .or_default()
            .insert(digest);
        let mut projections = HashMap::new();
        for (uuid, source_index) in &staged.imported_profiles {
            if *source_index != index
                || matches!(
                    resolutions.get(uuid),
                    Some(super::MvrImportResolution::Skip)
                )
            {
                continue;
            }
            let definition = staged
                .definitions
                .get_mut(uuid)
                .expect("imported UUID has a definition");
            let mode = definition.mode_id.expect("canonical profile mode");
            if let std::collections::hash_map::Entry::Vacant(entry) = projections.entry(mode) {
                entry.insert(
                    profile
                        .resolved_definition(mode)
                        .map_err(|error| invalid(error.to_string()))?,
                );
            }
            *definition = projections[&mode].clone();
        }
        if !published.iter().any(|other: &FixtureProfile| {
            other.id == profile.id && u64::from(other.revision) == revision
        }) {
            published.push(profile.clone());
        }
    }
    staged.new_profiles = published;
    staged.imported_profiles.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> FixtureProfile {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Immutable".into();
        profile.short_name = "Immutable".into();
        profile
    }

    #[test]
    fn mvr_reservations_include_show_only_revisions_and_do_not_reuse_conflicting_slots() {
        let first = profile();
        let mut second = first.clone();
        second.notes = "Second source".into();
        let mut staged = MvrDefinitions::default();
        for (index, source) in [first.clone(), second].into_iter().enumerate() {
            let uuid = Uuid::from_u128(index as u128 + 1);
            staged.definitions.insert(
                uuid,
                source.resolved_definition(source.modes[0].id).unwrap(),
            );
            staged.imported_profiles.insert(uuid, index);
            staged.new_profiles.push(source);
        }
        let mut slots = MvrProfileSlots::from([(
            (first.id.0, 7),
            BTreeSet::from([
                typed_identity(&first).unwrap(),
                "other destination contents".into(),
            ]),
        )]);
        reserve_mvr_profiles(&mut staged, &mut slots, &HashMap::new()).unwrap();
        assert_eq!(
            staged
                .new_profiles
                .iter()
                .map(|profile| profile.revision)
                .collect::<Vec<_>>(),
            vec![8, 9]
        );
        assert_eq!(staged.definitions[&Uuid::from_u128(1)].revision, 8);
        assert_eq!(
            staged.definitions[&Uuid::from_u128(2)]
                .profile_snapshot
                .as_ref()
                .unwrap()
                .revision,
            9
        );
    }

    #[test]
    fn mvr_native_revision_conflicts_fail_before_any_reinterpretation() {
        let mut profile = profile();
        profile.revision = 7;
        let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
        let mut staged = MvrDefinitions {
            definitions: HashMap::from([(Uuid::from_u128(1), definition)]),
            ..Default::default()
        };
        let mut slots =
            MvrProfileSlots::from([((profile.id.0, 7), BTreeSet::from(["different".into()]))]);
        assert_eq!(
            reserve_mvr_profiles(&mut staged, &mut slots, &HashMap::new())
                .unwrap_err()
                .kind,
            ActionErrorKind::Conflict
        );
        assert_eq!(staged.definitions[&Uuid::from_u128(1)].revision, 7);
    }
    fn legacy_profile_document() -> serde_json::Value {
        let mut profile = profile();
        profile.revision = 1;
        profile.geometry = Default::default();
        let mode = &mut profile.modes[0];
        let attribute = light_core::AttributeKey("intensity".into());
        mode.channels.push(light_fixture::FixtureChannel {
            id: Uuid::new_v4(),
            head_id: mode.heads[0].id,
            split: 1,
            fixture_attribute: attribute.clone(),
            attribute: attribute.clone(),
            canonical_transform: Default::default(),
            resolution: light_fixture::ChannelResolution::U8,
            secondary_slots: Vec::new(),
            default_raw: 0,
            highlight_raw: 255,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            virtual_intensity_inverted: false,
            behavior: light_fixture::ChannelBehavior::Controlled,
            functions: vec![light_fixture::ChannelFunction::continuous(
                "Dimmer", attribute, 255,
            )],
        });
        let mut legacy = serde_json::to_value(profile).unwrap();
        legacy.as_object_mut().unwrap().remove("geometry");
        legacy.as_object_mut().unwrap().remove("mounting");
        let mode = legacy["modes"][0].as_object_mut().unwrap();
        mode.remove("emitter_heads");
        mode.remove("motion_attributes");
        let channel = mode["channels"][0].as_object_mut().unwrap();
        channel.remove("virtual_intensity_inverted");
        for field in [
            "reacts_to_grand_master",
            "reacts_to_group_master",
            "reacts_to_sequence_master",
        ] {
            channel.insert(field.into(), serde_json::json!(false));
        }
        let physical = legacy["physical"].as_object_mut().unwrap();
        physical.insert("color_temperature_kelvin".into(), serde_json::json!(3200.0));
        physical.insert("luminous_output_lumens".into(), serde_json::json!(7500.0));
        physical.insert("beam_angle_degrees".into(), serde_json::json!(26.0));
        legacy
    }

    #[test]
    fn mvr_native_legacy_defaults_and_optical_lift_share_existing_revision_identity() {
        let legacy = legacy_profile_document();
        let incoming: FixtureProfile = serde_json::from_value(legacy.clone()).unwrap();
        incoming.validate().unwrap();
        assert_eq!(incoming.optics.color_temperature_kelvin, Some(3200.0));
        assert_eq!(incoming.optics.luminous_output_lumens, Some(7500.0));
        let identity = mvr_profile_identity(legacy.clone()).unwrap();
        assert_eq!(identity, typed_identity(&incoming).unwrap());
        let mut slots = MvrProfileSlots::from([((incoming.id.0, 1), BTreeSet::from([identity]))]);
        let uuid = Uuid::new_v4();
        let mut staged = MvrDefinitions {
            definitions: HashMap::from([(
                uuid,
                incoming.resolved_definition(incoming.modes[0].id).unwrap(),
            )]),
            ..Default::default()
        };
        reserve_mvr_profiles(&mut staged, &mut slots, &HashMap::new()).unwrap();
        assert_eq!(staged.definitions[&uuid].revision, 1);
        assert!(
            staged.new_profiles.is_empty(),
            "no replacement library revision is published"
        );
        assert!(
            legacy.get("geometry").is_none(),
            "raw source documents are not rewritten"
        );
    }

    #[test]
    fn mvr_native_real_notes_calibration_and_source_changes_still_conflict() {
        let legacy = legacy_profile_document();
        let original: FixtureProfile = serde_json::from_value(legacy.clone()).unwrap();
        let mut notes = original.clone();
        notes.notes = "Changed calibration note".into();
        let mut calibration = original.clone();
        calibration.modes[0].channels[0].functions[0].physical_mapping =
            Some(light_fixture::PhysicalMappingCalibration {
                revision: 1,
                source: Some("Measured bench calibration".into()),
                ..Default::default()
            });
        let mut source = original.clone();
        let archive = light_fixture::gdtf::profile::package_profile(&original).unwrap();
        source.source_gdtf =
            Some(light_fixture::ProfileGdtfSource::associate(&source, &archive).unwrap());
        for changed in [notes, calibration, source] {
            changed.validate().unwrap();
            let original_digest = mvr_profile_identity(legacy.clone()).unwrap();
            assert_ne!(original_digest, typed_identity(&changed).unwrap());
            let mut slots =
                MvrProfileSlots::from([((original.id.0, 1), BTreeSet::from([original_digest]))]);
            let before = slots.clone();
            let uuid = Uuid::new_v4();
            let mut staged = MvrDefinitions {
                definitions: HashMap::from([(
                    uuid,
                    changed.resolved_definition(changed.modes[0].id).unwrap(),
                )]),
                ..Default::default()
            };
            assert_eq!(
                reserve_mvr_profiles(&mut staged, &mut slots, &HashMap::new())
                    .unwrap_err()
                    .kind,
                ActionErrorKind::Conflict
            );
            assert_eq!(
                slots, before,
                "immutable destination identities are preserved"
            );
            assert_eq!(staged.definitions[&uuid].revision, 1);
        }
    }
}
