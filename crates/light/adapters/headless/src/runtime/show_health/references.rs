use super::*;

pub(super) fn findings(
    document: &PortableShowDocument,
    runtime: &EngineSnapshot,
) -> Vec<ShowHealthFinding> {
    let fixtures = runtime
        .fixtures
        .iter()
        .flat_map(|fixture| {
            std::iter::once(fixture.fixture_id.0.to_string()).chain(
                fixture
                    .logical_heads
                    .iter()
                    .map(|head| head.fixture_id.0.to_string()),
            )
        })
        .collect::<HashSet<_>>();
    let groups = document
        .objects_of_kind("group")
        .map(|group| group.key().id().to_string())
        .collect::<HashSet<_>>();
    let mut result = Vec::new();
    let presets = document
        .objects_of_kind("preset")
        .filter_map(|object| object.body()["instance_id"].as_str().map(str::to_string))
        .collect::<HashSet<_>>();
    for object in document.objects() {
        match object.key().kind() {
            "group" => group_findings(object, &fixtures, &groups, &mut result),
            "cue_list" => {
                cue_findings(object, &fixtures, &groups, &mut result);
                preset_reference_findings(object, &presets, &mut result);
            }
            "preset" => preset_findings(object, &fixtures, &groups, &mut result),
            _ => {}
        }
    }
    result
}

fn missing(
    object: &PortableShowObject,
    key: &str,
    id: &str,
    kind: &str,
    known: &HashSet<String>,
    fix: bool,
    result: &mut Vec<ShowHealthFinding>,
) {
    if !known.contains(id) {
        result.push(finding(
            object,
            &format!("{key}:{id}"),
            ShowHealthSeverity::Error,
            format!(
                "{} {} refers to missing {kind} {id}.",
                object.key().kind().replace('_', " "),
                object.key().id()
            ),
            None,
            fix,
        ));
    }
}

fn group_findings(
    object: &PortableShowObject,
    fixtures: &HashSet<String>,
    groups: &HashSet<String>,
    result: &mut Vec<ShowHealthFinding>,
) {
    let body = object.body();
    let source = &body["source"];
    if let Some(id) = body["frozen_from"]["source_group_id"].as_str()
        && !groups.contains(id)
    {
        result.push(finding(object,"frozen-source",ShowHealthSeverity::Info,format!("Frozen Group {} has no source Group {id}; its captured membership remains available.",object.key().id()),None,false));
    }
    if source["type"] == "references" {
        for reference in source["references"].as_array().into_iter().flatten() {
            if let Some(id) = reference["group_id"].as_str() {
                missing(object, "group-source", id, "Group", groups, false, result);
            }
        }
    } else if source.is_null() && !body["derived_from"].is_null() {
        if let Some(id) = body["derived_from"]["source_group_id"].as_str() {
            missing(object, "group-source", id, "Group", groups, false, result);
        }
    } else {
        let members = if source["type"] == "explicit" {
            &source["fixture_ids"]
        } else {
            &body["fixtures"]
        };
        let fix = body["frozen_from"].is_null();
        for id in members
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
        {
            missing(object, "group-member", id, "fixture", fixtures, fix, result);
        }
    }
}

fn preset_reference_findings(
    object: &PortableShowObject,
    presets: &HashSet<String>,
    result: &mut Vec<ShowHealthFinding>,
) {
    for cue in object.body()["cues"].as_array().into_iter().flatten() {
        for field in ["changes", "group_changes"] {
            for change in cue[field].as_array().into_iter().flatten() {
                if let Some(id) = change["preset_reference"]["preset_instance_id"].as_str()
                    && !presets.contains(id)
                {
                    let number = cue["number"].as_str().unwrap_or("?");
                    result.push(finding(object,&format!("cue:{number}:preset:{id}"),ShowHealthSeverity::Warning,format!("Cue {number} refers to a missing Preset source {id}; its recorded fallback values remain available."),None,false));
                }
            }
        }
    }
}

fn cue_findings(
    object: &PortableShowObject,
    fixtures: &HashSet<String>,
    groups: &HashSet<String>,
    result: &mut Vec<ShowHealthFinding>,
) {
    for cue in object.body()["cues"].as_array().into_iter().flatten() {
        let number = cue["number"].as_str().unwrap_or("?");
        for field in ["changes", "dynamic_changes"] {
            for change in cue[field].as_array().into_iter().flatten() {
                if let Some(id) = change["fixture_id"].as_str() {
                    missing(
                        object,
                        &format!("cue:{number}:{field}"),
                        id,
                        "fixture",
                        fixtures,
                        false,
                        result,
                    );
                }
            }
        }
        for change in cue["group_changes"].as_array().into_iter().flatten() {
            if let Some(id) = change["group_id"].as_str() {
                missing(
                    object,
                    &format!("cue:{number}:group"),
                    id,
                    "Group",
                    groups,
                    false,
                    result,
                );
            }
        }
    }
}

fn preset_findings(
    object: &PortableShowObject,
    fixtures: &HashSet<String>,
    groups: &HashSet<String>,
    result: &mut Vec<ShowHealthFinding>,
) {
    for (field, kind, known) in [
        ("values", "fixture", fixtures),
        ("group_values", "Group", groups),
    ] {
        if let Some(values) = object.body()[field].as_object() {
            for id in values.keys() {
                missing(object, field, id, kind, known, false, result);
            }
        }
    }
}
