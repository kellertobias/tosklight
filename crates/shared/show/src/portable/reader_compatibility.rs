//! Promote the reader floor when an object uses features that older readers cannot preserve.
use crate::StoreError;
use rusqlite::Transaction;
use serde_json::Value;

pub(super) fn stamp(tx: &Transaction<'_>, kind: &str, body: &Value) -> Result<(), StoreError> {
    let required_version = match kind {
        "patched_fixture" if contains_architecture(body) => 11,
        "group" if body.get("group_master_mode").and_then(Value::as_str) == Some("lotp") => 12,
        _ => return Ok(()),
    };
    tx.execute(
        "UPDATE schema_info SET version=MAX(version,?1)",
        [required_version],
    )?;
    Ok(())
}

// Look only at placement-owned fields; profile snapshots and opaque plugin data do not
// author venue construction, and ordinary fixture writes must not traverse their geometry.
fn contains_architecture(body: &Value) -> bool {
    body.get("scenery_options")
        .and_then(|options| options.get("architecture"))
        .is_some_and(Value::is_object)
        || body
            .get("multipatch")
            .and_then(Value::as_array)
            .is_some_and(|copies| copies.iter().any(contains_architecture))
}

#[cfg(test)]
mod tests;
