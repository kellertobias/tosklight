use super::*;
use light_engine::{
    ContributionFamilyRole, ContributionProgrammerLane, ContributionSourceDescriptor,
};
use light_wire::v2::visualization::{FixtureValueSource, FixtureValueSourcesSnapshot};

#[derive(Deserialize)]
pub(in crate::runtime) struct SourceQuery {
    pub fixture_id: Uuid,
}

pub(super) fn snapshot(
    state: &AppState,
    session: &Session,
    fixture_id: Uuid,
) -> Result<FixtureValueSourcesSnapshot, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| {
            fixture.fixture_id.0 == fixture_id
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id.0 == fixture_id)
        })
        .ok_or_else(|| ApiError::not_found("Fixture not found"))?;
    // A deliberate inspection must observe this read, not the sheet's throttled cache.
    let sources = state.output.engine().observe_held_parameter_values();
    let mut values = Vec::new();
    for ((owner, attribute), value) in sources.values() {
        if owner.0 != fixture_id {
            continue;
        }
        let mut labels = Vec::new();
        if fixture
            .freeze
            .targets
            .get(owner)
            .is_some_and(|target| target.values.contains_key(attribute))
        {
            labels.push("Freeze (held output)".into());
        } else if let Some(origin) = sources.contribution_origin(*owner, attribute) {
            labels.push(source_label(origin.source().descriptor(), &snapshot));
            if let ContributionSourceDescriptor::Playback(source) = origin.source().descriptor() {
                let status = source
                    .playback_address
                    .and_then(|address| state.output.playback_runtime_status_at(address))
                    .or_else(|| {
                        state
                            .output
                            .playback_runtime_status_for_direct_cue_list(source.cue_list_id)
                    });
                if let Some(status) = status
                    && status.source == source
                    && let Some(cue) = snapshot
                        .cue_lists
                        .iter()
                        .find(|list| list.id == source.cue_list_id)
                        .and_then(|list| {
                            list.cues
                                .iter()
                                .find(|cue| Some(cue.id) == status.playback.current_cue_id)
                        })
                {
                    labels.push(format!(
                        "Current Cue {} {} (may contain tracked values)",
                        cue.number, cue.name
                    ));
                }
            }
            if let Some(evidence) = sources.contribution_family_evidence(*owner, attribute) {
                for entry in evidence
                    .entries()
                    .iter()
                    .filter(|entry| entry.role() == ContributionFamilyRole::Authored)
                {
                    let mut label = source_label(entry.source().descriptor(), &snapshot);
                    if let ContributionSourceDescriptor::Playback(playback) =
                        entry.source().descriptor()
                        && let Some(cue) = snapshot
                            .cue_lists
                            .iter()
                            .find(|list| list.id == playback.cue_list_id)
                            .and_then(|list| {
                                list.cues
                                    .iter()
                                    .find(|cue| Some(cue.id) == entry.authored_cue_id())
                            })
                    {
                        label.push_str(&format!(" · authored Cue {} {}", cue.number, cue.name));
                    }
                    if !labels.contains(&label) {
                        labels.push(label);
                    }
                }
            }
        } else {
            labels.push("Source unavailable (no retained ordinary authorship)".into());
        }
        values.push(FixtureValueSource {
            attribute: attribute.0.to_string(),
            value: serde_json::from_value(
                serde_json::to_value(value)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
            )
            .map_err(|error| ApiError::internal(error.to_string()))?,
            sources: labels,
        });
    }
    values.sort_by(|a, b| a.attribute.cmp(&b.attribute));
    let dynamic = resolved_visualization_content(
        state,
        session,
        false,
        true,
        true,
        None,
        VisualizationValueContent::FixtureSheet,
    )?;
    let dynamics = dynamic["dynamic_stack"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry["fixture_id"].as_str() == Some(&fixture_id.to_string())
                && entry["entry_type"] != "ordinary_static"
        })
        .map(|entry| {
            format!(
                "{}: {} · {} ({})",
                entry["attribute"].as_str().unwrap_or(""),
                entry["name"].as_str().unwrap_or(""),
                entry["source"].as_str().unwrap_or(""),
                if entry["winning"] == true {
                    "winning"
                } else {
                    "not winning"
                }
            )
        })
        .collect();
    let options = state.output.render_options();
    Ok(FixtureValueSourcesSnapshot {
        fixture_id,
        generated_at: chrono::Utc::now().to_rfc3339(),
        grand_master: options.grand_master,
        blackout: options.blackout,
        highlighted: state
            .output
            .engine()
            .highlighted_fixtures()
            .iter()
            .any(|owner| owner.0 == fixture_id || *owner == fixture.fixture_id),
        values,
        dynamics,
    })
}

fn source_label(source: ContributionSourceDescriptor<'_>, snapshot: &EngineSnapshot) -> String {
    match source {
        ContributionSourceDescriptor::Programmer { lane, .. } => match lane {
            ContributionProgrammerLane::Live => "Programmer".into(),
            ContributionProgrammerLane::Preload => "Programmer Preload".into(),
            ContributionProgrammerLane::Transient(name) => format!("Programmer transient {name}"),
            ContributionProgrammerLane::Group(group) => format!("Programmer Group {group}"),
            ContributionProgrammerLane::PreloadGroup(group) => {
                format!("Programmer Preload Group {group}")
            }
        },
        ContributionSourceDescriptor::Playback(source) => {
            let name = snapshot
                .cue_lists
                .iter()
                .find(|list| list.id == source.cue_list_id)
                .map(|list| list.name.as_str())
                .unwrap_or("Unknown Cuelist");
            let address = source
                .playback_address
                .map(|address| format!("Playback {}.{}", address.page(), address.position()))
                .unwrap_or_else(|| "Direct Cuelist".into());
            format!(
                "{address} · {name}{}",
                if source.temporary {
                    " · temporary"
                } else {
                    ""
                }
            )
        }
    }
}
