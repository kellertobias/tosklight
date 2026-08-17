use super::*;

pub(super) fn persist_programmer(state: &AppState, session: &Session) -> Result<(), ApiError> {
    let programmer = state
        .programming
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let connected = programmer.connected;
    state
        .installation
        .defer_session_save(DeferredProgrammerPersistence {
            id: session.id,
            user_id: session.user.id,
            token: session.token.clone(),
            programmer,
            connected,
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
        .map_err(ApiError::internal)
}

pub(super) fn active_playbacks_setting(show_id: light_core::ShowId) -> String {
    format!("active_playbacks:{}", show_id.0)
}

pub(super) fn output_runtime_setting(show_id: light_core::ShowId) -> String {
    format!("output_runtime:{}", show_id.0)
}

pub(super) fn load_output_runtime_for_show(
    state: &AppState,
    show_id: light_core::ShowId,
) -> Result<PersistedOutputRuntime, ApiError> {
    let Some(serialized) = state
        .installation
        .setting(&output_runtime_setting(show_id))
        .map_err(ApiError::store)?
    else {
        return Ok(PersistedOutputRuntime::default());
    };
    match serde_json::from_str::<PersistedOutputRuntime>(&serialized) {
        Ok(runtime) if runtime.is_valid() => Ok(runtime),
        Ok(_) => {
            tracing::warn!(?show_id, "ignoring invalid persisted output runtime");
            Ok(PersistedOutputRuntime::default())
        }
        Err(error) => {
            tracing::warn!(?show_id, %error, "ignoring invalid persisted output runtime");
            Ok(PersistedOutputRuntime::default())
        }
    }
}

pub(super) fn restore_output_runtime_for_show(
    state: &AppState,
    show_id: light_core::ShowId,
    runtime: PersistedOutputRuntime,
) {
    debug_assert_eq!(
        state.active_show.current().as_ref().map(|show| show.id),
        Some(show_id)
    );
    restore_output_group_masters(state, &runtime);
    state
        .output
        .execute_playback(EnginePlaybackCommand::RestoreDynamicsPausedSince(
            runtime.dynamics_paused_at,
        ))
        .expect("restoring dynamics pause state is infallible");
    state
        .output
        .execute_playback(EnginePlaybackCommand::RestoreActiveDynamics(
            runtime.dynamic_playbacks.clone(),
        ))
        .expect("restoring validated Dynamic Playback state is infallible");
    let snapshot = runtime.dynamic_runtime.clone().unwrap_or_default();
    if let Err(error) = state.output.restore_dynamic_runtime_snapshot(snapshot) {
        tracing::warn!(%error, "ignoring invalid persisted Dynamic runtime");
        state
            .output
            .restore_dynamic_runtime_snapshot(Default::default())
            .expect("an empty Dynamic runtime snapshot is always valid");
    }
    state.output.restore_runtime_control(&runtime);
    state.output.clear_runtime_replay();
}

pub(super) fn restore_output_group_masters(state: &AppState, runtime: &PersistedOutputRuntime) {
    for (group_id, master) in &runtime.group_masters {
        if let Err(error) = state.output.set_group_master(group_id, *master) {
            tracing::warn!(%group_id, %error, "ignoring unassigned persisted Group Master");
        }
    }
}

pub(super) fn persist_output_runtime(state: &AppState) -> Result<(), ApiError> {
    #[cfg(test)]
    {
        state.output.record_runtime_persistence_attempt()?;
    }
    let Some(show) = state.active_show.current().clone() else {
        return Ok(());
    };
    let control = state.output.control_projection();
    let runtime = PersistedOutputRuntime {
        revision: control.revision,
        grand_master: control.grand_master,
        blackout: control.blackout,
        dynamics_paused_at: state.output.playback_dynamics().paused_since,
        dynamic_playbacks: state.output.active_dynamic_playbacks_for_persistence(),
        dynamic_runtime: Some(state.output.dynamic_runtime_snapshot()),
        group_masters: state
            .output
            .snapshot()
            .groups
            .iter()
            .filter_map(|group| {
                state
                    .output
                    .group_master_for_persistence(&group.id)
                    .map(|master| (group.id.clone(), master))
            })
            .collect(),
    };
    let serialized =
        serde_json::to_string(&runtime).map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .installation
        .set_setting(&output_runtime_setting(show.id), &serialized)
        .map_err(ApiError::store)
}

pub(super) fn persist_active_playbacks(state: &AppState) -> Result<(), ApiError> {
    let Some(show) = state.active_show.current().clone() else {
        return Ok(());
    };
    let runtime = state.output.playback_runtime();
    let serialized = serialize_active_playbacks(&runtime)?;
    state
        .installation
        .set_setting(&active_playbacks_setting(show.id), &serialized)
        .map_err(ApiError::store)
}

pub(super) fn serialize_active_playbacks(
    runtime: &[light_playback::ActivePlayback],
) -> Result<String, ApiError> {
    let persisted_runtime = runtime
        .iter()
        .cloned()
        .map(|mut runtime| {
            runtime.playback_number = None;
            runtime.playback_identity = None;
            runtime
        })
        .collect::<Vec<_>>();
    let persisted = persisted_runtime
        .iter()
        .zip(runtime.iter())
        .map(|(runtime, source)| PersistedActivePlayback {
            runtime,
            playback_number: source.playback_number,
            playback_identity: source.playback_identity.as_ref().filter(|identity| {
                matches!(identity, light_playback::PlaybackIdentity::Virtual(_))
            }),
            activation: source.activation.as_ref(),
            transition_ordinal: source.transition_ordinal,
            fader_zero_auto_off_armed: source.fader_zero_auto_off_armed,
        })
        .collect::<Vec<_>>();
    let serialized =
        serde_json::to_string(&persisted).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut persisted = serde_json::from_str::<Vec<serde_json::Value>>(&serialized)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    for value in &mut persisted {
        let Some(object) = value.as_object_mut() else {
            continue;
        };
        object.remove("fader_position");
        object.remove("fader_pickup_required");
        object.remove("fader_pickup_target");
    }
    serde_json::to_string(&persisted).map_err(|error| ApiError::internal(error.to_string()))
}

#[derive(serde::Serialize)]
struct PersistedActivePlayback<'a> {
    #[serde(flatten)]
    runtime: &'a light_playback::ActivePlayback,
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_number: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_identity: Option<&'a light_playback::PlaybackIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activation: Option<&'a light_playback::PlaybackActivationProvenance>,
    #[serde(skip_serializing_if = "is_zero")]
    transition_ordinal: u64,
    #[serde(skip_serializing_if = "is_false")]
    fader_zero_auto_off_armed: bool,
}

const fn is_zero(value: &u64) -> bool {
    *value == 0
}
const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod auto_off_persistence_tests {
    use super::*;

    #[test]
    fn fader_zero_restart_origin_survives_the_private_runtime_sidecar() {
        let mut runtime: light_playback::ActivePlayback =
            serde_json::from_value(serde_json::json!({
                "cue_list_id": light_core::CueListId::new(),
                "cue_index": 0,
                "previous_index": null,
                "paused": false,
                "activated_at": chrono::Utc::now(),
                "paused_at": null
            }))
            .unwrap();
        runtime.fader_zero_auto_off_armed = true;

        let serialized = serialize_active_playbacks(&[runtime]).unwrap();
        let restored: Vec<light_playback::ActivePlayback> =
            serde_json::from_str(&serialized).unwrap();
        assert!(restored[0].fader_zero_auto_off_armed);
    }
}
pub(super) fn emit(state: &AppState, kind: &str, payload: serde_json::Value) -> u64 {
    let revision = state.events.record_audit(kind, payload.clone());
    if let Some(event) = typed_capability_event(revision, kind, &payload) {
        state.events.publish(event);
    }
    announce_show_to_the_network(state, kind);
    revision
}

/// Keep what this desk advertises equal to what it is running.
///
/// Every path that changes the active show ends in one of these events, so this is the one place
/// that has to know rather than seven. A peer decides from the record, and a record naming the
/// show that was loaded at startup is worse than none.
fn announce_show_to_the_network(state: &AppState, kind: &str) {
    if !matches!(
        kind,
        "show_opened" | "show_created" | "show_uploaded" | "show_renamed" | "show_rolled_back"
    ) {
        return;
    }
    let show = state
        .installation
        .active_show()
        .ok()
        .flatten()
        .map(|entry| entry.name);
    state.discovery.announce_show(show);
}

fn typed_capability_event(
    revision: u64,
    kind: &str,
    payload: &serde_json::Value,
) -> Option<light_application::EventDraft> {
    use light_application::{
        EventDraft, FixtureLibraryNotification, FixtureLibraryNotificationKind,
        HardwareConnectionNotification, MediaNotification, MediaNotificationKind,
        NotificationRevision, ScreenNotification, ScreenNotificationKind,
        ShowLibraryNotificationKind,
    };
    let signal = NotificationRevision { revision };
    Some(match kind {
        // Desk configuration, both of them: the second is what a connected visualizer follows.
        "server_configuration_changed" | "visualizer_view_changed" => {
            EventDraft::configuration_changed(signal)
        }
        "screen_configuration_changed" => EventDraft::screens_changed(ScreenNotification {
            revision,
            kind: ScreenNotificationKind::Configuration,
        }),
        "screen_page_changed" => EventDraft::screens_changed(ScreenNotification {
            revision,
            kind: ScreenNotificationKind::ScreenPage,
        }),
        "playback_page_changed" => EventDraft::screens_changed(ScreenNotification {
            revision,
            kind: ScreenNotificationKind::PlaybackPage,
        }),
        "show_opened" => show_library_event(revision, ShowLibraryNotificationKind::ShowOpened),
        "show_renamed" => show_library_event(revision, ShowLibraryNotificationKind::ShowRenamed),
        "show_rolled_back" => {
            show_library_event(revision, ShowLibraryNotificationKind::ShowRolledBack)
        }
        "show_uploaded" => show_library_event(revision, ShowLibraryNotificationKind::ShowUploaded),
        "show_deleted" => show_library_event(revision, ShowLibraryNotificationKind::ShowDeleted),
        "fixture_library_changed" => {
            EventDraft::fixture_library_changed(FixtureLibraryNotification {
                revision,
                kind: FixtureLibraryNotificationKind::Library,
            })
        }
        "fixture_profile_changed" => {
            EventDraft::fixture_library_changed(FixtureLibraryNotification {
                revision,
                kind: FixtureLibraryNotificationKind::Profile,
            })
        }
        "media_thumbnails_refreshed" => EventDraft::media_changed(MediaNotification {
            revision,
            kind: MediaNotificationKind::ThumbnailsRefreshed,
        }),
        "media_preview_refreshed" => EventDraft::media_changed(MediaNotification {
            revision,
            kind: MediaNotificationKind::PreviewRefreshed,
        }),
        "media_server_offline" => EventDraft::media_changed(MediaNotification {
            revision,
            kind: MediaNotificationKind::ServerOffline,
        }),
        "hardware_connection_changed" => {
            #[derive(serde::Deserialize)]
            struct HardwarePayload {
                connected: bool,
            }
            let payload: HardwarePayload = decode(payload)?;
            EventDraft::hardware_connection_changed(HardwareConnectionNotification {
                revision,
                connected: payload.connected,
            })
        }
        "desk_action" => EventDraft::system_event(operator_desk_action(revision, payload)?),
        "file_input_action" => EventDraft::system_event(operator_file_input(revision, payload)?),
        "file_operation_completed" => {
            EventDraft::system_event(operator_file_operation(revision, payload)?)
        }
        "group_configuration_requested" => {
            EventDraft::system_event(operator_group_configuration(revision, payload)?)
        }
        "playback_configuration_requested" => {
            EventDraft::system_event(operator_playback_configuration(revision, payload)?)
        }
        "update_armed"
        | "update_target_requested"
        | "update_target_rejected"
        | "update_targets_requested"
        | "update_settings_requested" => {
            EventDraft::system_event(operator_update(revision, kind, payload)?)
        }
        "command_history" => EventDraft::system_event(operator_command_history(revision, payload)?),
        _ => return None,
    })
}

fn show_library_event(
    revision: u64,
    kind: light_application::ShowLibraryNotificationKind,
) -> light_application::EventDraft {
    light_application::EventDraft::show_library_changed(
        light_application::ShowLibraryNotification { revision, kind },
    )
}

fn decode<T: serde::de::DeserializeOwned>(payload: &serde_json::Value) -> Option<T> {
    match serde_json::from_value(payload.clone()) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(%error, "audit notification payload did not match its typed event");
            None
        }
    }
}

fn operator_desk_action(
    revision: u64,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::DeskAction {
            revision,
            notification: decode(payload)?,
        },
    ))
}

fn operator_file_input(
    revision: u64,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::FileInput {
            revision,
            notification: decode(payload)?,
        },
    ))
}

fn operator_file_operation(
    revision: u64,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::FileOperation {
            revision,
            notification: decode(payload)?,
        },
    ))
}

fn operator_group_configuration(
    revision: u64,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::GroupConfiguration {
            revision,
            notification: decode(payload)?,
        },
    ))
}

fn operator_playback_configuration(
    revision: u64,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::PlaybackConfiguration {
            revision,
            notification: decode(payload)?,
        },
    ))
}

#[derive(serde::Deserialize)]
struct UpdatePayload {
    desk_id: String,
    #[serde(default)]
    armed: Option<bool>,
    #[serde(default)]
    target: Option<UpdateTargetPayload>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct UpdateTargetPayload {
    family: UpdateTargetFamilyPayload,
    object_id: String,
    playback_number: Option<u16>,
    cue_id: Option<String>,
    cue_number: Option<String>,
    validate_active_context: Option<bool>,
}

#[derive(serde::Deserialize)]
struct UpdateTargetFamilyPayload {
    #[serde(rename = "type")]
    kind: String,
}

impl UpdateTargetPayload {
    fn into_notification(self) -> Option<light_application::UpdateTargetNotification> {
        let family = match self.family.kind.as_str() {
            "cue" => light_application::UpdateTargetFamilyNotification::Cue,
            "preset" => light_application::UpdateTargetFamilyNotification::Preset,
            "group" => light_application::UpdateTargetFamilyNotification::Group,
            _ => return None,
        };
        Some(light_application::UpdateTargetNotification {
            family,
            object_id: self.object_id,
            playback_number: self.playback_number,
            cue_id: self.cue_id,
            cue_number: self.cue_number,
            validate_active_context: self.validate_active_context,
        })
    }
}

fn operator_update(
    revision: u64,
    kind: &str,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    let payload: UpdatePayload = decode(payload)?;
    let notification = match kind {
        "update_armed" => light_application::UpdateWorkflowNotification::Armed {
            desk_id: payload.desk_id,
            armed: payload.armed.unwrap_or(true),
        },
        "update_target_requested" => {
            light_application::UpdateWorkflowNotification::TargetRequested {
                desk_id: payload.desk_id,
                target: payload.target?.into_notification()?,
            }
        }
        "update_target_rejected" => light_application::UpdateWorkflowNotification::TargetRejected {
            desk_id: payload.desk_id,
            error: payload.error,
        },
        "update_targets_requested" => {
            light_application::UpdateWorkflowNotification::TargetsRequested {
                desk_id: payload.desk_id,
            }
        }
        "update_settings_requested" => {
            light_application::UpdateWorkflowNotification::SettingsRequested {
                desk_id: payload.desk_id,
            }
        }
        _ => return None,
    };
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::UpdateWorkflow {
            revision,
            notification,
        },
    ))
}

#[derive(serde::Deserialize)]
struct DeskPayload {
    desk_id: String,
}

fn operator_command_history(
    revision: u64,
    payload: &serde_json::Value,
) -> Option<light_application::SystemEvent> {
    let payload: DeskPayload = decode(payload)?;
    Some(light_application::SystemEvent::Operator(
        light_application::OperatorNotification::CommandHistoryChanged {
            revision,
            desk_id: payload.desk_id,
        },
    ))
}

pub(super) fn record_command_history(
    state: &AppState,
    session: &Session,
    command: &str,
    status: &str,
    feedback: &str,
    source: &str,
    request_id: Option<&str>,
) {
    let (retained_command, sensitive) = command_audit_projection(command);
    if retained_command.is_empty() {
        return;
    }
    let retained_feedback = if sensitive {
        "Sensitive input omitted".into()
    } else {
        feedback.chars().take(1_000).collect::<String>()
    };
    let entry = CommandHistoryEntry {
        id: Uuid::new_v4().to_string(),
        desk_id: session.desk.id,
        session_id: session.id,
        command: retained_command,
        status: status.into(),
        feedback: retained_feedback,
        source: source.into(),
        request_id: request_id.map(str::to_owned),
        at: chrono::Utc::now().to_rfc3339(),
    };
    state
        .programming
        .record_command_history(entry.clone(), COMMAND_HISTORY_LIMIT);
    emit(
        state,
        "command_history",
        serde_json::to_value(entry).expect("command history entries serialize"),
    );
}

pub(super) fn command_audit_projection(command: &str) -> (String, bool) {
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = normalized.to_ascii_uppercase();
    let sensitive = [
        "PASSWORD",
        "PASSCODE",
        "TOKEN",
        "SECRET",
        "AUTHORIZATION",
        "API_KEY",
    ]
    .iter()
    .any(|term| upper.split_whitespace().any(|token| token.contains(term)));
    if sensitive {
        ("[REDACTED SENSITIVE COMMAND]".into(), true)
    } else {
        (normalized.chars().take(512).collect(), false)
    }
}
pub(super) fn validate_show_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 100 || name.contains(['/', '\\']) {
        Err(ApiError::bad_request(
            "show name must be a plain name up to 100 characters",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn available_show_name(state: &AppState, stem: &str) -> Result<String, ApiError> {
    let existing = state
        .installation
        .show_library()
        .map_err(ApiError::store)?
        .into_iter()
        .map(|show| show.name.to_lowercase())
        .collect::<HashSet<_>>();
    for number in 1..=10_000 {
        let candidate = if number == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {number}")
        };
        let path = state
            .installation
            .data_dir()
            .join("shows")
            .join(format!("{candidate}.show"));
        if !existing.contains(&candidate.to_lowercase()) && !path.exists() {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict("no available show name remains"))
}

pub(super) fn revision_copy_name(
    state: &AppState,
    source_name: &str,
    revision: u64,
    copied_on: chrono::NaiveDate,
) -> Result<String, ApiError> {
    let existing = state
        .installation
        .show_library()
        .map_err(ApiError::store)?
        .into_iter()
        .map(|show| show.name.to_lowercase())
        .collect::<HashSet<_>>();
    let stem_suffix = format!("-rev-{revision}-{copied_on}");
    for number in 1..=10_000 {
        let disambiguator = if number == 1 {
            String::new()
        } else {
            format!("-{number}")
        };
        let available = 100usize.saturating_sub(stem_suffix.len() + disambiguator.len());
        let mut boundary = source_name.len().min(available);
        while !source_name.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let candidate = format!(
            "{}{}{}",
            &source_name[..boundary],
            stem_suffix,
            disambiguator
        );
        let path = state
            .installation
            .data_dir()
            .join("shows")
            .join(format!("{candidate}.show"));
        if !existing.contains(&candidate.to_lowercase()) && !path.exists() {
            return Ok(candidate);
        }
    }
    Err(ApiError::conflict(
        "no unused name is available for the revision copy",
    ))
}

#[cfg(test)]
mod event_publication_tests {
    use super::typed_capability_event;

    #[test]
    fn audit_only_kinds_do_not_create_application_events() {
        for kind in [
            "highlight_changed",
            "programmer_changed",
            "show_object_changed",
            "preload_stored",
        ] {
            assert!(
                typed_capability_event(1, kind, &serde_json::json!({})).is_none(),
                "{kind} must remain audit-only at the generic emit boundary"
            );
        }
    }

    #[test]
    fn hardware_state_is_a_typed_desk_event() {
        let draft = typed_capability_event(
            7,
            "hardware_connection_changed",
            &serde_json::json!({
                "connected": true
            }),
        )
        .expect("hardware state must publish");
        assert_eq!(
            draft.object,
            Some(light_application::EventObject::new(
                light_application::EventCapability::Desk,
                "hardware-connections",
            ))
        );
        let light_application::ApplicationEvent::Desk(
            light_application::DeskEvent::HardwareConnectionChanged(change),
        ) = draft.payload
        else {
            panic!("expected a typed Desk hardware event");
        };
        assert_eq!(change.revision, 7);
        assert!(change.connected);
    }

    #[test]
    fn dotted_update_target_is_preserved_in_the_typed_operator_event() {
        let draft = typed_capability_event(
            8,
            "update_target_requested",
            &serde_json::json!({
                "desk_id": "desk",
                "target": {
                    "family": {"type": "cue"},
                    "object_id": "cue-list",
                    "playback_number": 1,
                    "cue_id": "cue",
                    "cue_number": "2.5",
                    "validate_active_context": true
                }
            }),
        )
        .expect("valid Update target must publish");
        let light_application::ApplicationEvent::System(light_application::SystemEvent::Operator(
            light_application::OperatorNotification::UpdateWorkflow {
                notification:
                    light_application::UpdateWorkflowNotification::TargetRequested { target, .. },
                ..
            },
        )) = draft.payload
        else {
            panic!("expected a typed operator Update event");
        };
        assert_eq!(
            target.family,
            light_application::UpdateTargetFamilyNotification::Cue
        );
        assert_eq!(target.cue_number, Some("2.5".into()));
    }
}
