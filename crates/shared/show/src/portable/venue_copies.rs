//! Venue objects no longer carry multi-patch copies.
//!
//! A copy shared its primary's profile and identity but was placed, and would have to be sized, on
//! its own — so a Venue object is now placed one object at a time and the patch refuses copies on
//! it. A show written before that keeps loading: every copy of a visual-only fixture becomes a
//! Venue object of its own, keeping the copy's identity, name, placement, size and appearance, and
//! numbered after the show's existing `0.x` objects.

use crate::StoreError;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use uuid::Uuid;

/// Fields a copy holds for itself; everything else it inherits from its primary.
const COPY_OWNED_FIELDS: &[&str] = &[
    "name",
    "location",
    "rotation",
    "scenery_size_metres",
    "bracket_angle",
    "shaper_angle",
    "installed_appearance",
    "invert_pan",
    "invert_tilt",
];

pub(super) fn separate_venue_multipatch_copies(conn: &Connection) -> Result<(), StoreError> {
    let rows = patched_fixtures(conn)?;
    let mut next_virtual_number = rows
        .iter()
        .filter_map(|(_, body)| body.get("virtual_fixture_number")?.as_u64())
        .max()
        .unwrap_or(0)
        + 1;
    let updated_at = Utc::now().to_rfc3339();
    for (id, mut body) in rows {
        let copies = match body.get("multipatch").and_then(Value::as_array) {
            Some(copies) if !copies.is_empty() => copies.clone(),
            _ => continue,
        };
        if !is_visual_only(conn, &body)? {
            continue;
        }
        body["multipatch"] = json!([]);
        for copy in copies {
            let Some(copy_id) = copy.get("id").and_then(Value::as_str) else {
                continue;
            };
            let object = separated_copy(&body, &copy, copy_id, next_virtual_number);
            next_virtual_number += 1;
            conn.execute(
                "INSERT OR IGNORE INTO objects(kind,id,body_json,revision,updated_at)
                 VALUES ('patched_fixture',?1,?2,1,?3)",
                params![copy_id, object.to_string(), updated_at],
            )?;
        }
        conn.execute(
            "UPDATE objects SET body_json=?1, revision=revision+1, updated_at=?2
              WHERE kind='patched_fixture' AND id=?3",
            params![body.to_string(), updated_at, id],
        )?;
    }
    Ok(())
}

fn patched_fixtures(conn: &Connection) -> Result<Vec<(String, Value)>, StoreError> {
    let mut statement =
        conn.prepare("SELECT id, body_json FROM objects WHERE kind='patched_fixture' ORDER BY id")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, json)| Some((id, serde_json::from_str(&json).ok()?)))
        .collect())
}

/// The fixture's patch policy, from its profile revision or, for a legacy inline record, from the
/// profile snapshot it carries.
fn is_visual_only(conn: &Connection, body: &Value) -> Result<bool, StoreError> {
    let referenced = match (
        body.get("profile_id").and_then(Value::as_str),
        body.get("profile_revision").and_then(Value::as_i64),
    ) {
        (Some(profile_id), Some(revision)) => conn
            .query_row(
                "SELECT json_extract(profile_json, '$.patch_policy')
                   FROM fixture_profile_revisions WHERE profile_id=?1 AND revision=?2",
                params![profile_id, revision],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten(),
        _ => None,
    };
    let policy = referenced.or_else(|| {
        body.pointer("/definition/profile_snapshot/patch_policy")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    Ok(policy.as_deref() == Some("visual_only"))
}

fn separated_copy(primary: &Value, copy: &Value, copy_id: &str, virtual_number: u64) -> Value {
    let mut object = primary.clone();
    object["fixture_id"] = json!(copy_id);
    object["fixture_number"] = Value::Null;
    object["virtual_fixture_number"] = json!(virtual_number);
    object["multipatch"] = json!([]);
    if let Some(splits) = copy.get("split_patches") {
        object["split_patches"] = splits.clone();
    }
    for field in COPY_OWNED_FIELDS {
        match copy.get(*field) {
            Some(value) if !value.is_null() => object[*field] = value.clone(),
            _ if *field == "scenery_size_metres" => {
                if let Some(fields) = object.as_object_mut() {
                    fields.remove(*field);
                }
            }
            _ => {}
        }
    }
    if object["name"].as_str().is_none_or(str::is_empty) {
        object["name"] = primary["name"].clone();
    }
    // A logical head is a selectable identity of its own; the new object's heads cannot share them.
    if let Some(heads) = object
        .get_mut("logical_heads")
        .and_then(Value::as_array_mut)
    {
        for head in heads {
            head["fixture_id"] = json!(Uuid::new_v4().to_string());
        }
    }
    object
}
