use super::*;
use light_core::ReplacementProjectionMap;
use light_programmer::{GroupDefinition, resolve_group};
use light_show::{PortableShowDocument, PortableShowObjectKey, PortableShowTransaction};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(in crate::show_patch) fn stage(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
    plans: &[PatchProgrammingReplacement],
) -> Result<BTreeSet<PortableShowObjectKey>, ActionError> {
    if plans.is_empty() {
        return Ok(BTreeSet::new());
    }
    let groups = document
        .objects_of_kind("group")
        .map(|object| {
            serde_json::from_value::<GroupDefinition>(object.body().clone())
                .map(|group| (group.id.clone(), group))
                .map_err(invalid)
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let membership = groups
        .keys()
        .map(|id| {
            resolve_group(id, &groups)
                .map(|members| (id.clone(), members.into_iter().collect::<HashSet<_>>()))
                .map_err(invalid)
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let mut changed = BTreeSet::new();
    for object in document
        .objects()
        .filter(|object| matches!(object.key().kind(), "cue_list" | "preset" | "group"))
    {
        let mut body = object.body().clone();
        match object.key().kind() {
            "cue_list" => migrate_cue_list(&mut body, &membership, plans)?,
            "preset" => {
                // Mutate metadata only. Preserve immutable source identity and every exact value,
                // live Group owner, unknown extension field, timing and reference.
                let fixture_values = body
                    .get("values")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                for (id, values) in fixture_values {
                    let Some(owner) = id.parse::<uuid::Uuid>().ok().map(FixtureId) else {
                        continue;
                    };
                    for key in values
                        .as_object()
                        .into_iter()
                        .flat_map(|values| values.keys())
                    {
                        let attribute = AttributeKey(key.as_str().into());
                        let existing = body
                            .get("fixture_replacement_projections")
                            .and_then(|value| value.get(&id))
                            .and_then(|value| value.get(key));
                        let mut projection: Option<ReplacementProgramProjection> = existing
                            .cloned()
                            .map(serde_json::from_value)
                            .transpose()
                            .map_err(invalid)?;
                        for plan in plans {
                            projection = plan
                                .project_existing(&attribute, owner, projection.as_ref())
                                .map_err(invalid)?;
                        }
                        if let Some(projection) = projection {
                            nested_map(&mut body, &["fixture_replacement_projections", &id])?
                                .insert(
                                    key.clone(),
                                    serde_json::to_value(projection).map_err(invalid)?,
                                );
                        }
                    }
                }
                let group_values = body
                    .get("group_values")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                for (id, values) in group_values {
                    for key in values
                        .as_object()
                        .into_iter()
                        .flat_map(|values| values.keys())
                    {
                        let attribute = AttributeKey(key.as_str().into());
                        let existing = body
                            .get("group_replacement_projections")
                            .and_then(|value| value.get(&id))
                            .and_then(|value| value.get(key));
                        let migrated =
                            member_map(existing, &attribute, membership.get(&id), plans)?;
                        if !migrated.is_empty() {
                            nested_map(&mut body, &["group_replacement_projections", &id])?.insert(
                                key.clone(),
                                serde_json::to_value(migrated).map_err(invalid)?,
                            );
                        }
                    }
                }
            }
            "group" => {
                let values = body
                    .get("programming")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let members = body
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|id| membership.get(id));
                for key in values.keys() {
                    let attribute = AttributeKey(key.as_str().into());
                    let existing = body
                        .get("replacement_projections")
                        .and_then(|value| value.get(key));
                    let migrated = member_map(existing, &attribute, members, plans)?;
                    if !migrated.is_empty() {
                        nested_map(&mut body, &["replacement_projections"])?.insert(
                            key.clone(),
                            serde_json::to_value(migrated).map_err(invalid)?,
                        );
                    }
                }
            }
            _ => unreachable!(),
        }
        if &body != object.body() {
            transaction.put(object.key().kind(), object.key().id(), body);
            changed.insert(object.key().clone());
        }
    }
    Ok(changed)
}

fn migrate_cue_list(
    body: &mut Value,
    membership: &HashMap<String, HashSet<FixtureId>>,
    plans: &[PatchProgrammingReplacement],
) -> Result<(), ActionError> {
    if let Some(cues) = body.get_mut("cues").and_then(Value::as_array_mut) {
        for cue in cues {
            if let Some(changes) = cue.get_mut("changes").and_then(Value::as_array_mut) {
                for change in changes {
                    let Some(owner) = change
                        .get("fixture_id")
                        .and_then(Value::as_str)
                        .and_then(|id| id.parse::<uuid::Uuid>().ok())
                        .map(FixtureId)
                    else {
                        continue;
                    };
                    let Some(attribute) = attribute(change.get("attribute")) else {
                        continue;
                    };
                    let mut projection: Option<ReplacementProgramProjection> = change
                        .get("replacement_projection")
                        .filter(|value| !value.is_null())
                        .cloned()
                        .map(serde_json::from_value)
                        .transpose()
                        .map_err(invalid)?;
                    for plan in plans {
                        projection = plan
                            .project_existing(&attribute, owner, projection.as_ref())
                            .map_err(invalid)?;
                    }
                    if let Some(projection) = projection {
                        map(change)?.insert(
                            "replacement_projection".into(),
                            serde_json::to_value(projection).map_err(invalid)?,
                        );
                    }
                }
            }
            if let Some(changes) = cue.get_mut("group_changes").and_then(Value::as_array_mut) {
                for change in changes {
                    let Some(attribute) = attribute(change.get("attribute")) else {
                        continue;
                    };
                    let members = change
                        .get("group_id")
                        .and_then(Value::as_str)
                        .and_then(|id| membership.get(id));
                    let existing = change.get("replacement_projections");
                    let migrated = member_map(existing, &attribute, members, plans)?;
                    if !migrated.is_empty() {
                        map(change)?.insert(
                            "replacement_projections".into(),
                            serde_json::to_value(migrated).map_err(invalid)?,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn member_map(
    existing: Option<&Value>,
    attribute: &AttributeKey,
    members: Option<&HashSet<FixtureId>>,
    plans: &[PatchProgrammingReplacement],
) -> Result<ReplacementProjectionMap, ActionError> {
    let mut map: ReplacementProjectionMap = existing
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(invalid)?
        .unwrap_or_default();
    for plan in plans {
        if !members.is_some_and(|members| members.contains(&plan.source_owner))
            && !map.contains_key(&plan.source_owner)
        {
            continue;
        }
        if let Some(projection) = plan
            .project_existing(attribute, plan.source_owner, map.get(&plan.source_owner))
            .map_err(invalid)?
        {
            map.insert(plan.source_owner, projection);
        }
    }
    Ok(map)
}
fn attribute(value: Option<&Value>) -> Option<AttributeKey> {
    value
        .and_then(Value::as_str)
        .map(|value| AttributeKey(value.into()))
}
fn map(value: &mut Value) -> Result<&mut Map<String, Value>, ActionError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid("programming object metadata is not a map"))
}
fn nested_map<'a>(
    value: &'a mut Value,
    keys: &[&str],
) -> Result<&'a mut Map<String, Value>, ActionError> {
    if let Some((key, rest)) = keys.split_first() {
        let value = map(value)?
            .entry((*key).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        nested_map(value, rest)
    } else {
        map(value)
    }
}
fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}
