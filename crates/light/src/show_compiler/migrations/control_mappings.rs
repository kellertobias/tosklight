use super::candidate;
use crate::ActionError;
use light_show::{PortableShowDocument, PortableShowTransaction};
use serde_json::{Value, json};

const REPORT_KIND: &str = "compatibility_report";
const REPORT_ID: &str = "removed-built-in-midi-control-mappings-v1";

pub(super) fn stage_removal_report(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
) -> Result<(), ActionError> {
    let projected = candidate(document, transaction)?;
    let mut removed = projected
        .objects_of_kind("control_mapping")
        .filter(|object| {
            object
                .body()
                .pointer("/trigger/type")
                .and_then(Value::as_str)
                == Some("midi")
        })
        .map(|object| {
            json!({
                "object_id": object.key().id(),
                "object_revision": object.revision(),
                "name": object.body().get("name").cloned().unwrap_or(Value::Null),
                "status": object.body().pointer("/trigger/status").cloned().unwrap_or(Value::Null),
                "data1": object.body().pointer("/trigger/data1").cloned().unwrap_or(Value::Null),
                "action": object.body().get("action").cloned().unwrap_or(Value::Null),
                "original": object.body(),
            })
        })
        .collect::<Vec<_>>();
    if removed.is_empty() {
        return Ok(());
    }
    removed.sort_by(|left, right| left["object_id"].as_str().cmp(&right["object_id"].as_str()));
    let existing_entries = projected
        .object(REPORT_KIND, REPORT_ID)
        .and_then(|object| object.body().get("removed_mappings"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let source_show_id = projected.id();
    let source_revision = projected.base_revision().value();
    let removed_ids = removed
        .iter()
        .filter_map(|entry| entry["object_id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let mut combined = existing_entries;
    for entry in removed {
        if !combined.iter().any(|existing| {
            existing["object_id"] == entry["object_id"]
                && existing["object_revision"] == entry["object_revision"]
        }) {
            combined.push(entry);
        }
    }
    combined.sort_by(|left, right| {
        left["object_id"]
            .as_str()
            .cmp(&right["object_id"].as_str())
            .then_with(|| {
                left["object_revision"]
                    .as_u64()
                    .cmp(&right["object_revision"].as_u64())
            })
    });
    for id in removed_ids {
        transaction.delete("control_mapping", id);
    }
    transaction.put(REPORT_KIND, REPORT_ID, json!({
        "schema_version": 1,
        "migration": "removed_builtin_midi_control_mappings",
        "source_show_id": source_show_id,
        "source_revision": source_revision,
        "summary": "Built-in MIDI and RTP-MIDI were removed. These mappings were preserved for manual recreation in an approved native extension.",
        "removed_mappings": combined,
    }));
    Ok(())
}
