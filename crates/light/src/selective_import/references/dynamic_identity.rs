//! Preserve derived Angle partner identities when an imported Dynamic gets a new ID.
//! The imported body is lossless JSON: only the known generated lane IDs may change.

use light_dynamics::DynamicDefinition;
use serde_json::Value;
use uuid::Uuid;

/// Call after the Dynamic's `/id` has been rewritten, using the source and destination IDs.
/// Malformed pool content remains importable for later operator repair.
pub(super) fn reidentify_automatic_angle_partners(
    body: &mut Value,
    source_id: Uuid,
    destination_id: Uuid,
) {
    if source_id == destination_id {
        return;
    }
    // The primary identity has already been rewritten by the ordinary reference pass. Parse a
    // source-identity copy so derived partner detection uses the original namespace, then patch
    // only the known lane IDs in the lossless destination body.
    let mut source_body = body.clone();
    let Some(object) = source_body.as_object_mut() else {
        return;
    };
    object.insert("id".into(), source_id.to_string().into());
    if let Ok(changed) = rewrite_definition(&mut source_body, source_id, destination_id) {
        rewrite_lane_ids(body, &changed);
    }
}

/// An embedded fallback is part of a rewritten live reference, so malformed known content must
/// block import instead of silently keeping a stale definition namespace.
pub(super) fn reidentify_embedded_references(
    original: &Value,
    rewritten: &mut Value,
    owner_kind: &str,
) -> Result<(), String> {
    match owner_kind {
        "cue_list" => {
            let count = original
                .pointer("/cues")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            for cue in 0..count {
                let changes = original
                    .pointer(&format!("/cues/{cue}/dynamic_changes"))
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                for change in 0..changes {
                    let prefix = format!("/cues/{cue}/dynamic_changes/{change}/value");
                    if original
                        .pointer(&format!("{prefix}/type"))
                        .and_then(Value::as_str)
                        != Some("dynamic_on")
                    {
                        continue;
                    }
                    let dynamic = format!("{prefix}/dynamic");
                    let Some((_, destination)) = changed_live_id(original, rewritten, &dynamic)?
                    else {
                        continue;
                    };
                    let fallback = format!("{dynamic}/embedded_fallback/definition");
                    let old_fallback = original
                        .pointer(&format!("{fallback}/id"))
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("missing embedded Dynamic identity at {fallback}"))?
                        .parse::<Uuid>()
                        .map_err(|_| format!("invalid embedded Dynamic identity at {fallback}"))?;
                    let body = rewritten
                        .pointer_mut(&fallback)
                        .ok_or_else(|| format!("missing embedded Dynamic at {fallback}"))?;
                    let changed = rewrite_definition(body, old_fallback, destination)?;
                    let lane_pointer = format!("{prefix}/lane_id");
                    let lane = original
                        .pointer(&lane_pointer)
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("missing Cue Dynamic lane at {lane_pointer}"))?
                        .parse::<Uuid>()
                        .map_err(|_| format!("invalid Cue Dynamic lane at {lane_pointer}"))?;
                    if let Some((_, replacement)) = changed.iter().find(|(old, _)| *old == lane) {
                        let lane_text = lane.to_string();
                        let current = rewritten
                            .pointer_mut(&lane_pointer)
                            .ok_or_else(|| format!("missing Cue Dynamic lane at {lane_pointer}"))?;
                        if current.as_str() != Some(lane_text.as_str()) {
                            return Err(format!(
                                "Cue Dynamic lane changed before rewrite at {lane_pointer}"
                            ));
                        }
                        *current = replacement.to_string().into();
                    }
                }
            }
        }
        "playback" => {
            if original.pointer("/target/type").and_then(Value::as_str) == Some("dynamic") {
                let dynamic = "/target/assignment/dynamic";
                if let Some((_, destination)) = changed_live_id(original, rewritten, dynamic)? {
                    let fallback = format!("{dynamic}/embedded_fallback/definition");
                    let old_fallback = original
                        .pointer(&format!("{fallback}/id"))
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("missing embedded Dynamic identity at {fallback}"))?
                        .parse::<Uuid>()
                        .map_err(|_| format!("invalid embedded Dynamic identity at {fallback}"))?;
                    let body = rewritten
                        .pointer_mut(&fallback)
                        .ok_or_else(|| format!("missing embedded Dynamic at {fallback}"))?;
                    rewrite_definition(body, old_fallback, destination)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn changed_live_id(
    original: &Value,
    rewritten: &Value,
    dynamic: &str,
) -> Result<Option<(Uuid, Uuid)>, String> {
    let pointer = format!("{dynamic}/dynamic_id");
    let Some(before) = original.pointer(&pointer).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let before = before
        .as_str()
        .ok_or_else(|| format!("invalid live Dynamic identity at {pointer}"))?
        .parse::<Uuid>()
        .map_err(|_| format!("invalid live Dynamic identity at {pointer}"))?;
    let after = rewritten
        .pointer(&pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing rewritten Dynamic identity at {pointer}"))?
        .parse::<Uuid>()
        .map_err(|_| format!("invalid rewritten Dynamic identity at {pointer}"))?;
    Ok((before != after).then_some((before, after)))
}

fn rewrite_definition(
    body: &mut Value,
    source_id: Uuid,
    destination_id: Uuid,
) -> Result<Vec<(Uuid, Uuid)>, String> {
    let raw_id = body
        .get("id")
        .and_then(Value::as_str)
        .ok_or("embedded Dynamic definition has no identity")?;
    if raw_id != source_id.to_string() {
        return Err("embedded Dynamic definition identity changed before rewrite".into());
    }
    let mut definition: DynamicDefinition = serde_json::from_value(body.clone())
        .map_err(|error| format!("invalid embedded Dynamic definition: {error}"))?;
    let changed = definition.reidentify(destination_id);
    body.as_object_mut()
        .ok_or("embedded Dynamic definition is not an object")?
        .insert("id".into(), destination_id.to_string().into());
    rewrite_lane_ids(body, &changed);
    Ok(changed)
}

fn rewrite_lane_ids(body: &mut Value, changed: &[(Uuid, Uuid)]) {
    let Some(lanes) = body.get_mut("lanes").and_then(Value::as_array_mut) else {
        return;
    };
    for lane in lanes {
        let Some(id) = lane
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            continue;
        };
        if let Some((_, replacement)) = changed.iter().find(|(old, _)| *old == id)
            && let Some(object) = lane.as_object_mut()
        {
            object.insert("id".into(), replacement.to_string().into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn angle_fallback(id: Uuid) -> Value {
        let authored_pan = Uuid::new_v4();
        let raw = json!({
            "id": id, "pool_number": 7, "revision": 1, "name": "Angle fallback",
            "target_binding": {"type":"targetless"},
            "lanes": [{
                "id": authored_pan,
                "programming": {
                    "address": {"representation":{"kind":"angles"},"component":{"kind":"pan"}},
                    "configuration": {"mode":"keyframes","configuration":{
                        "points": [{"position":0.0,"source":{"kind":"value","value":{"kind":"scalar","value":45.0}},"interpolation":"linear"}],
                        "size":1.0
                    }}
                },
                "speed_multiplier":{"numerator":1,"denominator":1}, "width":1.0
            }],
            "phase": {"ordering":{"type":"selection"},"offset_degrees":0.0,
                "span_degrees":360.0,"block_size":1,"repeats":1,"wings":false},
            "speed":{"type":"fixed","duration_millis":1000},
            "default_activation":"start_now"
        });
        let definition: DynamicDefinition = serde_json::from_value(raw).unwrap();
        assert_eq!(definition.lanes.len(), 2);
        let mut raw = serde_json::to_value(definition).unwrap();
        raw["lanes"][1]["future_lane"] = json!({"keep": true});
        raw["future_definition"] = json!({"keep": true});
        raw
    }

    #[test]
    fn duplicated_cue_and_playback_rebind_embedded_generated_lanes_losslessly() {
        let source = Uuid::new_v4();
        let destination = Uuid::new_v4();
        let fallback = angle_fallback(source);
        let authored = fallback["lanes"][0]["id"].clone();
        let generated = fallback["lanes"][1]["id"].clone();
        let cue = json!({
            "cues":[{"dynamic_changes":[{"value":{
                "type":"dynamic_on", "dynamic":{
                    "dynamic_id": source,
                    "embedded_fallback":{"definition":fallback.clone()}
                },
                "lane_id": generated,
                "overrides":{"future_override":{"keep":true}}
            }}]}],
            "future_cue":{"keep":true}
        });
        let mut rewritten_cue = cue.clone();
        rewritten_cue["cues"][0]["dynamic_changes"][0]["value"]["dynamic"]["dynamic_id"] =
            json!(destination);
        reidentify_embedded_references(&cue, &mut rewritten_cue, "cue_list").unwrap();
        let changed = rewritten_cue
            .pointer("/cues/0/dynamic_changes/0/value")
            .unwrap();
        let definition = &changed["dynamic"]["embedded_fallback"]["definition"];
        assert_eq!(definition["id"], json!(destination));
        assert_eq!(definition["lanes"][0]["id"], authored);
        assert_ne!(definition["lanes"][1]["id"], generated);
        assert_eq!(changed["lane_id"], definition["lanes"][1]["id"]);
        assert_eq!(definition["lanes"][1]["future_lane"], json!({"keep":true}));
        assert_eq!(definition["future_definition"], json!({"keep":true}));
        assert_eq!(
            changed["overrides"]["future_override"],
            json!({"keep":true})
        );
        assert_eq!(rewritten_cue["future_cue"], json!({"keep":true}));

        let playback = json!({"target":{"type":"dynamic","assignment":{"dynamic":{
            "dynamic_id": source, "embedded_fallback":{"definition":fallback}
        }}}});
        let mut rewritten_playback = playback.clone();
        rewritten_playback["target"]["assignment"]["dynamic"]["dynamic_id"] = json!(destination);
        reidentify_embedded_references(&playback, &mut rewritten_playback, "playback").unwrap();
        let definition = rewritten_playback
            .pointer("/target/assignment/dynamic/embedded_fallback/definition")
            .unwrap();
        assert_eq!(definition["id"], json!(destination));
        assert_eq!(definition["lanes"][0]["id"], authored);
        assert_eq!(definition["lanes"][1]["id"], changed["lane_id"]);
        assert_eq!(definition["lanes"][1]["future_lane"], json!({"keep":true}));
        assert_eq!(definition["future_definition"], json!({"keep":true}));
    }

    #[test]
    fn deleted_dynamic_reference_does_not_reidentify_standalone_fallback() {
        let source = Uuid::new_v4();
        let cue = json!({"cues":[{"dynamic_changes":[{"value":{
            "type":"dynamic_on", "dynamic":{"dynamic_id":null,
                "embedded_fallback":{"definition":angle_fallback(source)}},
            "lane_id":Uuid::new_v4()}}]}]});
        let mut rewritten = cue.clone();
        reidentify_embedded_references(&cue, &mut rewritten, "cue_list").unwrap();
        assert_eq!(rewritten, cue);
    }

    #[test]
    fn duplicated_dynamic_keeps_raw_extensions_and_reidentifies_only_automatic_partner() {
        let source_id = Uuid::new_v4();
        let destination_id = Uuid::new_v4();
        let authored_pan = Uuid::new_v4();
        let source = json!({
            "id": source_id,
            "pool_number": 7,
            "revision": 1,
            "name": "Angle pair",
            "target_binding": {"type":"targetless"},
            "lanes": [{
                "id": authored_pan,
                "programming": {
                    "address": {"representation":{"kind":"angles"},"component":{"kind":"pan"}},
                    "configuration": {"mode":"keyframes","configuration":{
                        "points": [
                            {"position":0.0,"source":{"kind":"value","value":{"kind":"scalar","value":-90.0}},"interpolation":"linear"},
                            {"position":0.5,"source":{"kind":"value","value":{"kind":"scalar","value":90.0}},"interpolation":"linear"}
                        ],
                        "size":1.0
                    }}
                },
                "speed_multiplier":{"numerator":1,"denominator":1},
                "width":1.0
            }],
            "phase": {
                "ordering": {"type":"selection"},
                "offset_degrees": 0.0,
                "span_degrees": 360.0,
                "block_size": 1,
                "repeats": 1,
                "wings": false,
                "anchors_degrees": []
            },
            "speed": {"type":"fixed","duration_millis":1000},
            "default_activation": "start_now"
        });
        let normalized: DynamicDefinition = serde_json::from_value(source).unwrap();
        let mut raw = serde_json::to_value(normalized).unwrap();
        let source_partner = raw["lanes"][1]["id"].as_str().unwrap().to_owned();
        raw["lanes"][1]["future_lane"] = json!({"keep":"verbatim"});
        raw["future_definition"] = json!({"keep":true});
        raw["id"] = json!(destination_id);

        reidentify_automatic_angle_partners(&mut raw, source_id, destination_id);

        assert_eq!(raw["lanes"][0]["id"], json!(authored_pan));
        assert_ne!(raw["lanes"][1]["id"], json!(source_partner));
        assert_eq!(raw["lanes"][1]["future_lane"], json!({"keep":"verbatim"}));
        assert_eq!(raw["future_definition"], json!({"keep":true}));
        let mut copied: DynamicDefinition = serde_json::from_value(raw).unwrap();
        assert!(copied.is_automatic_angle_partner(&copied.lanes[1]));
        copied.lanes.retain(|lane| lane.id != authored_pan);
        copied.normalize_angle_pair();
        assert!(copied.lanes.is_empty());
    }
}
