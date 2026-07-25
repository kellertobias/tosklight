use super::invalid_object;
use crate::ActionError;
use light_core::FixtureId;
use light_fixture::{PatchedFixtureProfileReference, PatchedHead};
use light_show::{PortableShowCandidate, PortableShowCandidateObject};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

type StableHeadIds = HashMap<Uuid, FixtureId>;
type LegacyHeadIndices = HashMap<u16, FixtureId>;

#[derive(Clone, Copy)]
struct ResolvedHead {
    profile_head_id: Uuid,
    head_index: u16,
}

pub(super) struct ProfileHeadResolver<'a> {
    candidate: PortableShowCandidate<'a>,
    modes: HashMap<PatchedFixtureProfileReference, Vec<ResolvedHead>>,
}

impl<'a> ProfileHeadResolver<'a> {
    pub(super) fn new(candidate: PortableShowCandidate<'a>) -> Self {
        Self {
            candidate,
            modes: HashMap::new(),
        }
    }

    pub(super) fn reconcile(
        &mut self,
        object: PortableShowCandidateObject<'_>,
        reference: PatchedFixtureProfileReference,
        existing: Vec<PatchedHead>,
    ) -> Result<Vec<PatchedHead>, ActionError> {
        if !self.modes.contains_key(&reference) {
            let heads = resolve_mode_heads(self.candidate, object, reference)?;
            self.modes.insert(reference, heads);
        }
        reconcile_heads(
            self.modes
                .get(&reference)
                .expect("resolved mode was inserted"),
            existing,
            object,
        )
    }
}

fn resolve_mode_heads(
    candidate: PortableShowCandidate<'_>,
    object: PortableShowCandidateObject<'_>,
    reference: PatchedFixtureProfileReference,
) -> Result<Vec<ResolvedHead>, ActionError> {
    let profile = candidate
        .fixture_profile_revision(reference.profile_id, reference.profile_revision)
        .ok_or_else(|| invalid_object(object, "referenced fixture profile revision is missing"))?;
    let modes = profile
        .profile()
        .get("modes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_object(object, "fixture profile modes must be an array"))?;
    let mode = modes
        .iter()
        .find(|mode| uuid_field(mode, "id").ok() == Some(reference.mode_id))
        .ok_or_else(|| invalid_object(object, "selected fixture profile mode is missing"))?;
    let heads = mode
        .get("heads")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_object(object, "fixture profile mode heads must be an array"))?;
    heads
        .iter()
        .enumerate()
        .filter(|(_, head)| {
            !head
                .get("master_shared")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|(index, head)| {
            Ok(ResolvedHead {
                profile_head_id: uuid_field(head, "id")
                    .map_err(|error| invalid_object(object, error))?,
                head_index: u16::try_from(index)
                    .map_err(|_| invalid_object(object, "fixture profile has too many heads"))?,
            })
        })
        .collect()
}

fn uuid_field(value: &Value, field: &str) -> Result<Uuid, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("fixture profile {field} must be a UUID string"))
        .and_then(|value| Uuid::parse_str(value).map_err(|error| error.to_string()))
}

fn reconcile_heads(
    resolved: &[ResolvedHead],
    existing: Vec<PatchedHead>,
    object: PortableShowCandidateObject<'_>,
) -> Result<Vec<PatchedHead>, ActionError> {
    let (mut stable, mut legacy) = index_existing_heads(existing, object)?;
    Ok(resolved
        .iter()
        .map(|head| PatchedHead {
            profile_head_id: Some(head.profile_head_id),
            head_index: head.head_index,
            fixture_id: stable
                .remove(&head.profile_head_id)
                .or_else(|| legacy.remove(&head.head_index))
                .unwrap_or_else(FixtureId::new),
        })
        .collect())
}

fn index_existing_heads(
    existing: Vec<PatchedHead>,
    object: PortableShowCandidateObject<'_>,
) -> Result<(StableHeadIds, LegacyHeadIndices), ActionError> {
    let mut stable = HashMap::new();
    let mut legacy = HashMap::new();
    for head in existing {
        let duplicate = match head.profile_head_id {
            Some(profile_head_id) => stable.insert(profile_head_id, head.fixture_id).is_some(),
            None => legacy.insert(head.head_index, head.fixture_id).is_some(),
        };
        if duplicate {
            return Err(invalid_object(
                object,
                "patched fixture contains duplicate logical head identity",
            ));
        }
    }
    Ok((stable, legacy))
}
