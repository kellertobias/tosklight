use super::{PatchFixturesCommand, PatchModeProjection, ShowPatchPorts};
use crate::{ActionError, ActionErrorKind};
use light_core::{FixtureId, Revision};
use light_fixture::{FixtureSplit, PatchedFixtureProfileReference};
use light_show::{FixtureProfileRevision, PortableShowDocument, PortableShowTransaction};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) type ProfileKey = (Uuid, Revision);
pub(super) type ModeKey = (Uuid, Revision, Uuid);

pub(super) struct ResolvedProfiles {
    missing: BTreeMap<ProfileKey, FixtureProfileRevision>,
    modes: ResolvedModes,
}

pub(super) struct ResolvedModes {
    by_reference: BTreeMap<ModeKey, ResolvedMode>,
}

pub(super) struct ResolvedMode {
    logical_heads: Vec<ResolvedLogicalHead>,
    projection: PatchModeProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolvedLogicalHead {
    pub(super) profile_head_id: Uuid,
    pub(super) head_index: u16,
}

impl ResolvedProfiles {
    pub(super) fn resolve<P: ShowPatchPorts>(
        document: &PortableShowDocument,
        command: &PatchFixturesCommand,
        materialized: BTreeMap<ProfileKey, FixtureProfileRevision>,
        ports: &P,
    ) -> Result<Self, ActionError> {
        let keys = command
            .fixtures
            .iter()
            .map(|fixture| profile_key(fixture.profile))
            .collect::<BTreeSet<_>>();
        let mut missing = materialized;
        for (profile_id, revision) in keys {
            let id = FixtureId(profile_id);
            if document.fixture_profile_revision(id, revision).is_none()
                && !missing.contains_key(&(profile_id, revision))
            {
                let profile = ports.resolve_profile_revision(id, revision)?;
                ensure_identity(&profile, profile_id, revision)?;
                missing.insert((profile_id, revision), profile);
            }
        }
        let modes = resolve_selected_modes(
            command.fixtures.iter().map(|fixture| fixture.profile),
            |key| {
                let profile = resolved_profile(document, &missing, profile_key_from_mode(key))
                    .ok_or_else(|| invalid("fixture profile revision is unavailable"))?;
                ResolvedMode::from_profile(profile.profile(), key.2)
            },
        )?;
        Ok(Self { missing, modes })
    }

    pub(super) fn mode(
        &self,
        reference: PatchedFixtureProfileReference,
    ) -> Result<&ResolvedMode, ActionError> {
        self.modes.get(reference)
    }

    pub(super) fn stage(
        self,
        transaction: &mut PortableShowTransaction,
    ) -> Result<ResolvedModes, ActionError> {
        for profile in self.missing.into_values() {
            transaction
                .put_fixture_profile_revision(profile)
                .map_err(store_error)?;
        }
        Ok(self.modes)
    }
}

impl ResolvedModes {
    pub(super) fn get(
        &self,
        reference: PatchedFixtureProfileReference,
    ) -> Result<&ResolvedMode, ActionError> {
        self.by_reference
            .get(&mode_key(reference))
            .ok_or_else(|| invalid("selected fixture mode was not resolved"))
    }
}

impl ResolvedMode {
    pub(super) fn from_profile(profile: &Value, mode_id: Uuid) -> Result<Self, ActionError> {
        let mode = referenced_mode(profile, mode_id)?;
        let heads = mode
            .get("heads")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("selected fixture mode has no head array"))?;
        let logical_heads = heads
            .iter()
            .enumerate()
            .filter(|(_, head)| !head["master_shared"].as_bool().unwrap_or(false))
            .map(|(index, head)| {
                Ok(ResolvedLogicalHead {
                    profile_head_id: required_uuid(head, "id")?,
                    head_index: u16::try_from(index)
                        .map_err(|_| invalid("selected fixture mode has too many logical heads"))?,
                })
            })
            .collect::<Result<_, _>>()?;
        let splits = mode
            .get("splits")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("selected fixture mode has no split array"))?
            .iter()
            .map(split_projection)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            logical_heads,
            projection: PatchModeProjection {
                mode_id,
                name: required_string(mode, "name")?,
                splits,
            },
        })
    }

    pub(super) fn logical_heads(&self) -> &[ResolvedLogicalHead] {
        &self.logical_heads
    }

    pub(super) fn projection(&self) -> &PatchModeProjection {
        &self.projection
    }
}

pub(super) const fn profile_key(reference: PatchedFixtureProfileReference) -> ProfileKey {
    (reference.profile_id.0, reference.profile_revision)
}

fn mode_key(reference: PatchedFixtureProfileReference) -> ModeKey {
    (
        reference.profile_id.0,
        reference.profile_revision,
        reference.mode_id,
    )
}

const fn profile_key_from_mode(key: ModeKey) -> ProfileKey {
    (key.0, key.1)
}

fn resolved_profile<'a>(
    document: &'a PortableShowDocument,
    missing: &'a BTreeMap<ProfileKey, FixtureProfileRevision>,
    key: ProfileKey,
) -> Option<&'a FixtureProfileRevision> {
    missing
        .get(&key)
        .or_else(|| document.fixture_profile_revision(FixtureId(key.0), key.1))
}

pub(super) fn resolve_selected_modes<I, F>(
    references: I,
    mut resolve: F,
) -> Result<ResolvedModes, ActionError>
where
    I: IntoIterator<Item = PatchedFixtureProfileReference>,
    F: FnMut(ModeKey) -> Result<ResolvedMode, ActionError>,
{
    let keys = references
        .into_iter()
        .map(mode_key)
        .collect::<BTreeSet<_>>();
    let by_reference = keys
        .into_iter()
        .map(|key| resolve(key).map(|mode| (key, mode)))
        .collect::<Result<_, _>>()?;
    Ok(ResolvedModes { by_reference })
}

fn referenced_mode(profile: &Value, mode_id: Uuid) -> Result<&Value, ActionError> {
    let mode_id = mode_id.to_string();
    profile
        .get("modes")
        .and_then(Value::as_array)
        .and_then(|modes| {
            modes
                .iter()
                .find(|mode| mode["id"].as_str() == Some(&mode_id))
        })
        .ok_or_else(|| invalid("fixture profile does not contain the selected mode"))
}

fn split_projection(split: &Value) -> Result<FixtureSplit, ActionError> {
    Ok(FixtureSplit {
        number: required_u16(split, "number")?,
        footprint: required_u16(split, "footprint")?,
    })
}

fn required_string(value: &Value, field: &str) -> Result<String, ActionError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("fixture profile {field} must be a string")))
}

fn required_u16(value: &Value, field: &str) -> Result<u16, ActionError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| invalid(format!("fixture profile {field} must be a 16-bit integer")))
}

fn required_uuid(value: &Value, field: &str) -> Result<Uuid, ActionError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| invalid(format!("fixture profile {field} must be a UUID")))
}

fn ensure_identity(
    profile: &FixtureProfileRevision,
    profile_id: Uuid,
    revision: Revision,
) -> Result<(), ActionError> {
    let actual = profile.id();
    if actual.profile_id().0 == profile_id && actual.revision() == revision {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::Invalid,
            "fixture library returned a different profile revision than requested",
        ))
    }
}

fn store_error(error: light_show::StoreError) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shared_reference_scans_a_large_mode_list_once() {
        let profile_id = FixtureId::new();
        let selected_mode_id = Uuid::from_u128(11_000);
        let reference = PatchedFixtureProfileReference {
            profile_id,
            profile_revision: 7,
            mode_id: selected_mode_id,
        };
        let profile = json!({
            "modes": (0..2_000)
                .map(|index| json!({
                    "id": Uuid::from_u128(10_000 + index).to_string(),
                    "name": format!("Mode {index}"),
                    "splits": [{"number": 1, "footprint": 1}],
                    "heads": [],
                }))
                .collect::<Vec<_>>()
        });
        let mut lookups = 0;

        let modes = resolve_selected_modes(std::iter::repeat_n(reference, 100), |key| {
            lookups += 1;
            ResolvedMode::from_profile(&profile, key.2)
        })
        .unwrap();

        assert_eq!(lookups, 1);
        assert_eq!(modes.by_reference.len(), 1);
        assert_eq!(
            modes.get(reference).unwrap().projection().mode_id,
            selected_mode_id
        );
    }
}
