//! TL-714 deliberately rejects the retired pool/virtual Playback format before any write.
//! This is a content boundary, not a migration: fixture-only shows remain usable unchanged.
use crate::StoreError;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::Path;

pub const PLAYBACK_CONTRACT_METADATA_KEY: &str = "light.playback_contract";
pub const PLAYBACK_CONTRACT_VERSION: u16 = 1;

fn rejected(detail: impl std::fmt::Display) -> StoreError {
    StoreError::Invalid(format!(
        "{detail}. This show uses the retired regular/virtual Playback format (before TL-714). \
         Nothing was changed. Open a show recorded with unified Playbacks or create a separate empty show."
    ))
}

/// Refuses obsolete identities even when a tolerant decoder would otherwise ignore them.
pub fn validate_playback_object(kind: &str, id: &str, body: &Value) -> Result<(), StoreError> {
    if kind == "group" && (body.get("master").is_some() || body.get("playback_fader").is_some()) {
        return Err(rejected(format!(
            "Group {id} has retired Group-owned Playback controls"
        )));
    }
    if contains_retired_dynamic_assignment(body) {
        return Err(rejected(format!(
            "Unsupported targetless or scoped Dynamic Playback assignment in {kind} {id}"
        )));
    }
    match kind {
        "playback" | "virtual_playback_exclusion_zones" => {
            Err(rejected(format!("Unsupported {kind} object {id}")))
        }
        "playback_page" => validate_page(id, body),
        "playback_solo_regions" => {
            if contains_retired_identity(body) {
                Err(rejected(format!(
                    "Unsupported Solo Region identity in {id}"
                )))
            } else {
                Ok(())
            }
        }
        _ if contains_retired_identity(body) => Err(rejected(format!(
            "Unsupported Playback identity in {kind} {id}"
        ))),
        _ => Ok(()),
    }
}

fn validate_page(id: &str, body: &Value) -> Result<(), StoreError> {
    let page = body.get("number").and_then(Value::as_u64);
    let Some(assignments) = body.get("assignments").and_then(Value::as_object) else {
        return Err(rejected(format!(
            "Playback page {id} has no unified assignments"
        )));
    };
    if !page.is_some_and(|number| (1..=99).contains(&number))
        || body.get("slots").is_some()
        || body.get("virtual_playbacks").is_some()
    {
        return Err(rejected(format!(
            "Playback page {id} has an unsupported address format"
        )));
    }
    for (position, definition) in assignments {
        if definition.pointer("/target/type").and_then(Value::as_str) == Some("speed_group")
            && definition.get("fader").and_then(Value::as_str) == Some("speed")
        {
            return Err(rejected(format!(
                "Playback page {id} assignment {position} has retired Speed Group fader speed; use learned_percentage"
            )));
        }
        if !position
            .parse::<u16>()
            .is_ok_and(|number| (1..=1000).contains(&number))
            || !definition.is_object()
            || definition.get("number").is_some()
            || contains_retired_identity(definition)
        {
            return Err(rejected(format!(
                "Playback page {id} assignment {position} is incompatible"
            )));
        }
    }
    Ok(())
}

/// The rejection happens using a read-only connection, before SQLite schema upgrades.
pub fn validate_show_playback_contract(path: impl AsRef<Path>) -> Result<(), StoreError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let marker: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key=?1",
            [PLAYBACK_CONTRACT_METADATA_KEY],
            |row| row.get(0),
        )
        .optional()?;
    if marker.as_deref().is_some_and(|value| value != "1") {
        return Err(StoreError::Invalid(format!(
            "Unsupported Playback contract {}. Nothing was changed; open a compatible show or create a separate empty show.",
            marker.unwrap()
        )));
    }
    for table in ["objects", "object_history", "object_redo"] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            continue;
        }
        let mut statement = conn.prepare(&format!(
            "SELECT kind,id,body_json FROM {table} ORDER BY kind,id"
        ))?;
        let objects = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for object in objects {
            let (kind, id, serialized) = object?;
            validate_serialized_playback_object(&kind, &id, &serialized, |raw| {
                serde_json::from_str(raw)
            })?;
        }
    }
    Ok(())
}

/// A negative lexical check only: candidate text always uses the existing JSON validator.
/// All recursive retired identities require playback/virtual/physical, and retired Dynamic
/// assignments require dynamic. Without JSON escapes, decoding cannot create an absent ASCII
/// token. Keep this necessary-token set in sync when adding a compatibility rejection.
/// Group and Playback kinds always parse because their structural rules need no such token.
/// Malformed unrelated JSON already belongs to its own reader, not this compatibility guard.
pub(crate) fn validate_serialized_playback_object(
    kind: &str,
    id: &str,
    serialized: &str,
    parse: impl FnOnce(&str) -> Result<Value, serde_json::Error>,
) -> Result<(), StoreError> {
    if !matches!(
        kind,
        "group"
            | "playback"
            | "playback_page"
            | "playback_solo_regions"
            | "virtual_playback_exclusion_zones"
    ) && !serialized.contains('\\')
        && !["playback", "virtual", "physical", "dynamic"]
            .iter()
            .any(|token| serialized.contains(*token))
    {
        return Ok(());
    }
    match parse(serialized) {
        Ok(body) => validate_playback_object(kind, id, &body),
        Err(error)
            if matches!(
                kind,
                "playback"
                    | "playback_page"
                    | "playback_solo_regions"
                    | "virtual_playback_exclusion_zones"
            ) =>
        {
            Err(error.into())
        }
        // Preserve the original policy for malformed unrelated objects, including Groups.
        Err(_) => Ok(()),
    }
}

use rusqlite::OptionalExtension;

/// Applies to persisted live state and programmer history, not unrelated media transport state.
pub fn validate_playback_runtime(body: &Value) -> Result<(), StoreError> {
    if contains_retired_identity(body) || contains_retired_dynamic_assignment(body) {
        Err(rejected(
            "The stored runtime has an obsolete Playback identity",
        ))
    } else {
        Ok(())
    }
}

fn contains_retired_dynamic_assignment(body: &Value) -> bool {
    match body {
        Value::Object(fields) => {
            let obsolete = fields.get("type").and_then(Value::as_str) == Some("dynamic")
                && fields.get("assignment").is_some_and(|assignment| {
                    assignment
                        .get("target_scope")
                        .is_some_and(|scope| !scope.is_null())
                        || assignment
                            .pointer("/dynamic/embedded_fallback/definition/target_binding/type")
                            .and_then(Value::as_str)
                            == Some("targetless")
                });
            obsolete || fields.values().any(contains_retired_dynamic_assignment)
        }
        Value::Array(values) => values.iter().any(contains_retired_dynamic_assignment),
        _ => false,
    }
}

fn contains_retired_identity(body: &Value) -> bool {
    match body {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "playback_number"
                    | "playback_identity"
                    | "virtual_playbacks"
                    | "virtual_playback_number"
                    | "virtualPlaybackRows"
                    | "virtualPlaybackColumns"
                    | "virtualPlaybackPageMode"
                    | "virtualPlaybackPinnedPage"
            ) || (key == "kind"
                && matches!(
                    value.as_str(),
                    Some("physical" | "virtual" | "virtual_playbacks")
                ))
                || contains_retired_identity(value)
        }),
        Value::Array(values) => values.iter().any(contains_retired_identity),
        _ => false,
    }
}

impl crate::PortableShowTransaction {
    pub fn check_playback_contract(&self) -> Result<(), StoreError> {
        for (key, body) in self.object_writes() {
            validate_playback_object(key.kind(), key.id(), body)?;
        }
        Ok(())
    }

    pub fn stamp_playback_contract(&mut self) {
        if self
            .changed_object_kinds()
            .any(|kind| matches!(kind, "playback_page" | "playback_solo_regions"))
        {
            self.set_metadata(
                PLAYBACK_CONTRACT_METADATA_KEY,
                PLAYBACK_CONTRACT_VERSION.to_string(),
            );
        }
    }
}

pub(crate) fn stamp_direct_playback_write(
    connection: &Connection,
    kinds: impl Iterator<Item = impl AsRef<str>>,
) -> Result<(), StoreError> {
    if kinds
        .into_iter()
        .any(|kind| matches!(kind.as_ref(), "playback_page" | "playback_solo_regions"))
    {
        connection.execute("INSERT INTO metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [PLAYBACK_CONTRACT_METADATA_KEY, "1"])?;
    }
    Ok(())
}
