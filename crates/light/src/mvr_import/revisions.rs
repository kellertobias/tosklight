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
pub fn mvr_profile_identity(mut profile: serde_json::Value) -> Result<String, ActionError> {
    // Compare the whole profile, including retained source and its verification state.
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
}
