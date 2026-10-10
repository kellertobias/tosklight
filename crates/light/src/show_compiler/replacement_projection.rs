//! Portable source addresses stay authored; only compiled runtime rows may use effective heads.
use super::invalid_candidate;
use crate::ActionError;
use light_core::{FixtureId, ReplacementProgramProjection};
use light_show::PortableShowCandidate;
use serde_json::Value;

pub(super) fn validate(candidate: PortableShowCandidate<'_>) -> Result<(), ActionError> {
    for object in candidate.objects() {
        let body = object.body();
        match object.key().kind() {
            "cue_list" => {
                for cue in body
                    .get("cues")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    for change in cue
                        .get("changes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(projection) = change
                            .get("replacement_projection")
                            .filter(|value| !value.is_null())
                        {
                            let owner = change
                                .get("fixture_id")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    invalid_candidate("replacement Cue source address is missing")
                                })?;
                            envelope(projection, owner)?;
                        }
                    }
                    for change in cue
                        .get("group_changes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        member_map(change.get("replacement_projections"))?;
                    }
                }
            }
            "preset" => {
                if let Some(fixtures) = body
                    .get("fixture_replacement_projections")
                    .and_then(Value::as_object)
                {
                    for (owner, attributes) in fixtures {
                        for projection in attributes
                            .as_object()
                            .ok_or_else(|| {
                                invalid_candidate("replacement Preset address metadata is invalid")
                            })?
                            .values()
                        {
                            envelope(projection, owner)?;
                        }
                    }
                }
                if let Some(groups) = body
                    .get("group_replacement_projections")
                    .and_then(Value::as_object)
                {
                    for attributes in groups.values() {
                        for members in attributes
                            .as_object()
                            .ok_or_else(|| {
                                invalid_candidate(
                                    "replacement Group Preset address metadata is invalid",
                                )
                            })?
                            .values()
                        {
                            member_map(Some(members))?;
                        }
                    }
                }
            }
            "group" => {
                if let Some(attributes) = body
                    .get("replacement_projections")
                    .and_then(Value::as_object)
                {
                    for members in attributes.values() {
                        member_map(Some(members))?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn member_map(value: Option<&Value>) -> Result<(), ActionError> {
    let Some(value) = value else {
        return Ok(());
    };
    let map = value
        .as_object()
        .ok_or_else(|| invalid_candidate("replacement member metadata is invalid"))?;
    for (owner, projection) in map {
        envelope(projection, owner)?;
    }
    Ok(())
}
fn envelope(value: &Value, owner: &str) -> Result<(), ActionError> {
    let projection: ReplacementProgramProjection = serde_json::from_value(value.clone())
        .map_err(|error| invalid_candidate(error.to_string()))?;
    projection
        .validate()
        .map_err(|error| invalid_candidate(error.to_string()))?;
    let owner = owner
        .parse::<uuid::Uuid>()
        .map(FixtureId)
        .map_err(|_| invalid_candidate("replacement authored fixture identity is invalid"))?;
    if projection.source_owner != owner {
        return Err(invalid_candidate(
            "replacement projection does not match its authored source address",
        ));
    }
    Ok(())
}
