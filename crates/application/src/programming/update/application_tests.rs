use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    ActiveShowObjectKind, ActiveShowPorts, ActiveShowService, ActiveShowUnitOfWork, BackupIdentity,
    EventBus, ExecutionPolicy, ProgrammingExecution, ProgrammingLifecycleCompletion,
    ProgrammingLifecycleTarget, ProgrammingPorts, ProgrammingReconciliation, ProgrammingService,
};
use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId, SessionId, ShowId, UserId};
use light_engine::EngineSnapshot;
use light_playback::{
    Cue, CueChange, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode,
};
use light_programmer::{
    GroupDefinition, HighlightRegistry, Preset, PresetFamily, ProgrammerRegistry,
};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn preset_update_is_lossless_single_event_and_replay_safe() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    rig.seed("preset", "1.1", preset_body(fixture, 0.2));
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let preview = rig.preview(preset_request(), "preview").unwrap();
    let command = command_from_preview(&rig, preset_target(), preview);
    let first = rig.apply(command.clone(), "apply").unwrap();

    assert!(!first.replayed);
    assert_eq!(first.outcome.summary.changed_count, 1);
    assert_eq!(first.outcome.projection.raw_body["future"]["keep"], true);
    assert_eq!(first.outcome.event_sequence, 1);
    assert_eq!(
        rig.steps().iter().filter(|step| **step == "commit").count(),
        1
    );

    let replay = rig.apply(command, "apply").unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome.event_sequence, first.outcome.event_sequence);
    assert_eq!(
        rig.steps().iter().filter(|step| **step == "commit").count(),
        1
    );
    assert_eq!(rig.active_show.events().latest_sequence(), 1);
}

#[test]
fn no_op_and_stale_revision_stop_before_serialization_backup_or_event() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    rig.seed("preset", "1.1", preset_body(fixture, 0.5));
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let preview = rig.preview(preset_request(), "preview").unwrap();
    rig.clear_steps();
    let error = rig
        .apply(
            command_from_preview(&rig, preset_target(), preview.clone()),
            "noop",
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(!rig.steps().contains(&"backup"));
    assert!(!rig.steps().contains(&"prepare"));
    assert_eq!(rig.active_show.events().latest_sequence(), 0);

    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let changed = rig.preview(preset_request(), "changed-preview").unwrap();
    let mut stale = command_from_preview(&rig, preset_target(), changed);
    stale.expected_object_revision = Some(stale.expected_object_revision.unwrap() + 1);
    rig.clear_steps();
    let error = rig.apply(stale, "stale").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(!rig.steps().contains(&"backup"));
    assert_eq!(rig.active_show.events().latest_sequence(), 0);
}

#[test]
fn same_user_desks_share_values_but_group_preview_uses_exact_desk_selection() {
    let rig = TestRig::new();
    let second_session = SessionId::new();
    let second_desk = Uuid::from_u128(22);
    rig.registry.start(second_session, rig.user);
    assert!(
        rig.registry
            .attach_command_context(second_session, SessionId(second_desk))
    );
    let first = FixtureId::new();
    let second = FixtureId::new();
    rig.registry.select(rig.session, [first]);
    rig.registry.select(second_session, [second]);
    rig.seed("group", "front", group_body());

    let first_preview = rig.preview(group_request(), "first-group").unwrap();
    let second_preview = rig
        .preview_as(
            group_request(),
            "second-group",
            second_desk,
            second_session,
            rig.user,
        )
        .unwrap();

    assert!(matches!(
        first_preview.preview.items[0].address,
        UpdateAddress::GroupMembership { fixture_id } if fixture_id == first
    ));
    assert!(matches!(
        second_preview.preview.items[0].address,
        UpdateAddress::GroupMembership { fixture_id } if fixture_id == second
    ));
}

#[test]
fn foreign_user_is_rejected_before_target_or_show_lookup() {
    let rig = TestRig::new();
    let envelope = ActionEnvelope {
        context: ActionContext::operator(
            rig.desk,
            UserId::new().0,
            rig.session.0,
            ActionSource::Http,
        )
        .with_request_id("foreign"),
        command: preset_request(),
    };
    let error = rig
        .service
        .preview_update(envelope, &rig.active_show, &rig.ports)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Forbidden);
    assert!(!rig.steps().contains(&"begin"));
}

#[test]
fn request_id_collision_is_rejected_before_capture_or_show_lookup() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    rig.seed("preset", "1.1", preset_body(fixture, 0.2));
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let preview = rig.preview(preset_request(), "preview").unwrap();
    let command = command_from_preview(&rig, preset_target(), preview);
    rig.apply(command.clone(), "collision").unwrap();

    let mut conflicting = command;
    conflicting.expected_object_revision = Some(conflicting.expected_object_revision.unwrap() + 1);
    rig.clear_steps();
    let error = rig.apply(conflicting, "collision").unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(!rig.steps().contains(&"begin"));
    assert_eq!(rig.active_show.events().latest_sequence(), 1);
}

#[test]
fn lifecycle_replacement_invalidates_update_replay() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    rig.seed("preset", "1.1", preset_body(fixture, 0.2));
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let preview = rig.preview(preset_request(), "preview").unwrap();
    let command = command_from_preview(&rig, preset_target(), preview);
    rig.apply(command.clone(), "before-replacement").unwrap();

    let lifecycle_ports = LifecyclePorts;
    let actor = ActionContext::operator(rig.desk, rig.user.0, rig.session.0, ActionSource::Http);
    rig.service
        .replace_user_programmer(
            &actor,
            &lifecycle_ports,
            ProgrammingLifecycleTarget::new(rig.user, rig.session, vec![rig.desk]),
            || {
                assert!(rig.registry.clear(rig.session));
                rig.registry.start(rig.session, rig.user);
                assert!(
                    rig.registry
                        .attach_command_context(rig.session, SessionId(rig.desk))
                );
                ProgrammingLifecycleCompletion::new((), Some(rig.session))
            },
        )
        .unwrap();

    rig.clear_steps();
    let error = rig.apply(command, "before-replacement").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(!rig.steps().contains(&"begin"));
}

#[test]
fn preset_fingerprint_ignores_selection_but_rejects_value_changes() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    rig.seed("preset", "1.1", preset_body(fixture, 0.2));
    rig.seed("preset", "1.2", preset_body_number(fixture, 0.3, 2));
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );

    let preview = rig.preview(preset_request(), "selection-preview").unwrap();
    rig.registry.select(rig.session, [FixtureId::new()]);
    rig.apply(
        command_from_preview(&rig, preset_target(), preview),
        "selection-apply",
    )
    .unwrap();

    let request = preset_request_for("1.2");
    let preview = rig.preview(request, "values-preview").unwrap();
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    );
    rig.clear_steps();
    let error = rig
        .apply(
            command_from_preview(&rig, preset_target_for("1.2"), preview),
            "values-apply",
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(!rig.steps().contains(&"begin"));
}

#[test]
fn group_fingerprint_ignores_values_and_other_desk_selection_only() {
    let rig = TestRig::new();
    let second_session = SessionId::new();
    let second_desk = Uuid::from_u128(22);
    rig.registry.start(second_session, rig.user);
    assert!(
        rig.registry
            .attach_command_context(second_session, SessionId(second_desk))
    );
    let first = FixtureId::new();
    let second = FixtureId::new();
    let third = FixtureId::new();
    rig.registry.select(rig.session, [first]);
    rig.registry.select(second_session, [second]);
    rig.seed("group", "front", group_body());
    rig.seed("group", "back", group_body_with_id("back"));

    let preview = rig.preview(group_request(), "irrelevant-preview").unwrap();
    rig.registry.set(
        rig.session,
        first,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
    );
    rig.registry.select(second_session, [third]);
    rig.apply(
        command_from_preview(&rig, group_target("front"), preview),
        "irrelevant-apply",
    )
    .unwrap();

    let preview = rig
        .preview(group_request_for("back"), "local-preview")
        .unwrap();
    rig.registry.select(rig.session, [second]);
    rig.clear_steps();
    let error = rig
        .apply(
            command_from_preview(&rig, group_target("back"), preview),
            "local-apply",
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert!(!rig.steps().contains(&"begin"));
}

#[test]
fn confirmed_live_cue_update_rejects_an_unpinned_preview_target() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    let cue_list_id = CueListId(Uuid::from_u128(899));
    let mut first = Cue::new(1.0);
    first.changes.push(CueChange::set(
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    ));
    let mut second = Cue::new(2.0);
    second.changes.push(CueChange::set(
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.3),
    ));
    let mut body = cue_list_body(cue_list_id, first.clone());
    body["cues"].as_array_mut().unwrap().push(json!(second));
    rig.seed("cue_list", &cue_list_id.0.to_string(), body);
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    rig.set_active_contexts(vec![ActiveCueContext {
        playback_number: 7,
        cue_list_id,
        cue_id: first.id,
        cue_number: first.number,
    }]);
    let target = ProgrammingUpdateTargetRequest::Cue {
        cue_list_id,
        playback_number: Some(7),
        cue_id: None,
        cue_number: None,
        validate_active_context: true,
    };
    let preview = rig
        .preview(
            ProgrammingUpdatePreviewRequest {
                show_id: rig.show_id,
                target: target.clone(),
                mode: UpdateMode::Cue(CueUpdateMode::ExistingInCurrentCue),
            },
            "unpinned-preview",
        )
        .unwrap();
    rig.set_active_contexts(vec![ActiveCueContext {
        playback_number: 7,
        cue_list_id,
        cue_id: second.id,
        cue_number: second.number,
    }]);

    let error = rig
        .apply(
            command_from_preview(&rig, target, preview),
            "unpinned-confirmation",
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.active_show.events().latest_sequence(), 0);
    assert!(!rig.steps().contains(&"backup"));
}

#[test]
fn cue_update_preserves_legacy_storage_identity_and_revalidates_live_context() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    let cue_list_id = CueListId(Uuid::from_u128(900));
    let mut cue = Cue::new(1.0);
    cue.changes.push(CueChange::set(
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    ));
    let cue_id = cue.id;
    let mut body = cue_list_body(cue_list_id, cue);
    body["cues"][0]["future_cue"] = json!({"keep":"nested"});
    body["cues"][0]["changes"][0]["future_change"] = json!({"keep":42});
    rig.seed("cue_list", "legacy-cuelist", body);
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    let active = ActiveCueContext {
        playback_number: 7,
        cue_list_id,
        cue_id,
        cue_number: 1.0,
    };
    rig.set_active_contexts(vec![active.clone()]);
    let target = cue_target(cue_list_id, cue_id);
    let request = ProgrammingUpdatePreviewRequest {
        show_id: rig.show_id,
        target: target.clone(),
        mode: UpdateMode::Cue(CueUpdateMode::ExistingInCurrentCue),
    };
    let preview = rig.preview(request, "cue-preview").unwrap();
    assert_eq!(preview.preview.target.object_id, cue_list_id.0.to_string());
    assert_eq!(preview.object.kind, ActiveShowObjectKind::CueList);
    assert_eq!(preview.object.object_id, "legacy-cuelist");
    assert_eq!(preview.object.object_revision, preview.object_revision);
    let command = command_from_preview(&rig, target, preview);
    rig.registry.select(rig.session, [FixtureId::new()]);

    rig.set_active_contexts(vec![ActiveCueContext {
        cue_number: 2.0,
        ..active.clone()
    }]);
    let error = rig.apply(command.clone(), "cue-conflict").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);

    rig.set_active_contexts(vec![active]);
    let result = rig.apply(command, "cue-apply").unwrap();
    assert_eq!(result.outcome.projection.object_id, "legacy-cuelist");
    assert_eq!(
        result.outcome.projection.raw_body["cues"][0]["future_cue"]["keep"],
        "nested"
    );
    assert_eq!(
        result.outcome.projection.raw_body["cues"][0]["changes"][0]["future_change"]["keep"],
        42
    );
    assert_eq!(result.outcome.summary.changed_count, 1);
    assert_eq!(rig.active_show.events().latest_sequence(), 1);
}

#[test]
fn target_menu_uses_one_capture_one_document_and_distinct_playback_contexts() {
    let rig = TestRig::new();
    let fixture = FixtureId::new();
    let cue_list_id = CueListId(Uuid::from_u128(901));
    let mut cue = Cue::new(1.0);
    cue.changes.push(CueChange::set(
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    ));
    let cue_id = cue.id;
    rig.seed(
        "cue_list",
        "legacy-menu-cuelist",
        cue_list_body(cue_list_id, cue),
    );
    rig.seed("preset", "1.1", preset_body(fixture, 0.2));
    rig.seed("group", "front", group_body());
    rig.registry.set(
        rig.session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );
    assert!(rig.registry.set_group(
        rig.session,
        "front".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    ));
    rig.registry.select_expression(
        rig.session,
        vec![fixture],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "front".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    assert!(rig.registry.set_modes(
        rig.session,
        None,
        None,
        None,
        Some(Some("preset:1.1".into())),
    ));
    rig.set_active_contexts(vec![
        ActiveCueContext {
            playback_number: 7,
            cue_list_id,
            cue_id,
            cue_number: 1.0,
        },
        ActiveCueContext {
            playback_number: 8,
            cue_list_id,
            cue_id,
            cue_number: 1.0,
        },
    ]);

    rig.clear_steps();
    let all = rig
        .targets(UpdateTargetFilter::ShowAllActive, "all-targets")
        .unwrap();
    assert_eq!(all.request_id, "all-targets");
    assert_eq!(all.entries.len(), 4);
    let cue_entries = all
        .entries
        .iter()
        .filter(|entry| matches!(entry.target, ProgrammingUpdateTargetRequest::Cue { .. }))
        .collect::<Vec<_>>();
    assert_eq!(cue_entries.len(), 2);
    assert_eq!(cue_entries[0].object.object_id, "legacy-menu-cuelist");
    assert_eq!(cue_entries[1].object.object_id, "legacy-menu-cuelist");
    assert_ne!(
        cue_entries[0].existing_preview.target.playback_number,
        cue_entries[1].existing_preview.target.playback_number
    );
    let preset = all
        .entries
        .iter()
        .find(|entry| matches!(entry.target, ProgrammingUpdateTargetRequest::Preset { .. }))
        .unwrap();
    let group = all
        .entries
        .iter()
        .find(|entry| matches!(entry.target, ProgrammingUpdateTargetRequest::Group { .. }))
        .unwrap();
    assert_eq!(preset.object.kind, ActiveShowObjectKind::Preset);
    assert_eq!(group.object.kind, ActiveShowObjectKind::Group);
    assert_eq!(
        cue_entries[0].programmer_revision,
        preset.programmer_revision
    );
    assert_ne!(preset.programmer_revision, group.programmer_revision);
    assert_single_target_query(&rig.steps());

    rig.clear_steps();
    let eligible = rig
        .targets(
            UpdateTargetFilter::EligibleForUpdateExisting,
            "eligible-targets",
        )
        .unwrap();
    assert_eq!(eligible.entries.len(), 3);
    assert!(
        eligible
            .entries
            .iter()
            .all(|entry| !matches!(entry.target, ProgrammingUpdateTargetRequest::Group { .. }))
    );
    assert_single_target_query(&rig.steps());
}

fn assert_single_target_query(steps: &[&'static str]) {
    assert_eq!(steps.iter().filter(|step| **step == "authorize").count(), 1);
    assert_eq!(steps.iter().filter(|step| **step == "begin").count(), 1);
    assert_eq!(steps.iter().filter(|step| **step == "document").count(), 1);
    assert_eq!(steps.iter().filter(|step| **step == "contexts").count(), 1);
    assert!(!steps.contains(&"prepare"));
    assert!(!steps.contains(&"backup"));
}

fn command_from_preview(
    rig: &TestRig,
    target: ProgrammingUpdateTargetRequest,
    preview: ProgrammingUpdatePreviewResult,
) -> ProgrammingUpdateCommand {
    ProgrammingUpdateCommand {
        show_id: rig.show_id,
        target,
        mode: preview.preview.mode,
        expected_object_revision: Some(preview.object_revision),
        expected_programmer_revision: Some(preview.programmer_revision),
        expected_show_revision: Some(preview.show_revision),
    }
}

fn preset_request() -> ProgrammingUpdatePreviewRequest {
    preset_request_for("1.1")
}

fn preset_request_for(object_id: &str) -> ProgrammingUpdatePreviewRequest {
    ProgrammingUpdatePreviewRequest {
        show_id: ShowId(Uuid::nil()),
        target: preset_target_for(object_id),
        mode: UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
    }
}

fn group_request() -> ProgrammingUpdatePreviewRequest {
    group_request_for("front")
}

fn group_request_for(object_id: &str) -> ProgrammingUpdatePreviewRequest {
    ProgrammingUpdatePreviewRequest {
        show_id: ShowId(Uuid::nil()),
        target: group_target(object_id),
        mode: UpdateMode::ExistingContent(ExistingContentMode::AddNew),
    }
}

fn preset_target() -> ProgrammingUpdateTargetRequest {
    preset_target_for("1.1")
}

fn preset_target_for(object_id: &str) -> ProgrammingUpdateTargetRequest {
    ProgrammingUpdateTargetRequest::Preset {
        object_id: object_id.into(),
    }
}

fn group_target(object_id: &str) -> ProgrammingUpdateTargetRequest {
    ProgrammingUpdateTargetRequest::Group {
        object_id: object_id.into(),
    }
}

fn cue_target(cue_list_id: CueListId, cue_id: Uuid) -> ProgrammingUpdateTargetRequest {
    ProgrammingUpdateTargetRequest::Cue {
        cue_list_id,
        playback_number: Some(7),
        cue_id: Some(cue_id),
        cue_number: Some(1.0),
        validate_active_context: true,
    }
}

struct TestRig {
    service: ProgrammingService,
    active_show: ActiveShowService,
    registry: ProgrammerRegistry,
    ports: TestPorts,
    show_id: ShowId,
    user: UserId,
    desk: Uuid,
    session: SessionId,
}

impl TestRig {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("light-update-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "Update test").unwrap();
        drop(store);
        let registry = ProgrammerRegistry::default();
        let user = UserId::new();
        let desk = Uuid::from_u128(11);
        let session = SessionId::new();
        registry.start(session, user);
        assert!(registry.attach_command_context(session, SessionId(desk)));
        let events = EventBus::new(16);
        Self {
            service: ProgrammingService::new(
                registry.clone(),
                events.clone(),
                Arc::new(HighlightRegistry::default()),
            ),
            active_show: ActiveShowService::new(events),
            registry,
            ports: TestPorts {
                path,
                show_id,
                steps: Arc::default(),
                active_contexts: Arc::default(),
            },
            show_id,
            user,
            desk,
            session,
        }
    }

    fn seed(&self, kind: &str, id: &str, mut body: Value) {
        body["future"] = json!({"keep":true});
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object(kind, id, &body, 0)
            .unwrap();
    }

    fn preview(
        &self,
        mut request: ProgrammingUpdatePreviewRequest,
        request_id: &str,
    ) -> Result<ProgrammingUpdatePreviewResult, ActionError> {
        request.show_id = self.show_id;
        self.preview_as(request, request_id, self.desk, self.session, self.user)
    }

    fn preview_as(
        &self,
        mut request: ProgrammingUpdatePreviewRequest,
        request_id: &str,
        desk: Uuid,
        session: SessionId,
        user: UserId,
    ) -> Result<ProgrammingUpdatePreviewResult, ActionError> {
        request.show_id = self.show_id;
        self.service.preview_update(
            ActionEnvelope {
                context: ActionContext::operator(desk, user.0, session.0, ActionSource::Http)
                    .with_request_id(request_id),
                command: request,
            },
            &self.active_show,
            &self.ports,
        )
    }

    fn apply(
        &self,
        command: ProgrammingUpdateCommand,
        request_id: &str,
    ) -> Result<ProgrammingUpdateResult, ActionError> {
        self.service.handle_update(
            ActionEnvelope {
                context: ActionContext::operator(
                    self.desk,
                    self.user.0,
                    self.session.0,
                    ActionSource::Http,
                )
                .with_request_id(request_id),
                command,
            },
            &self.active_show,
            &self.ports,
        )
    }

    fn targets(
        &self,
        filter: UpdateTargetFilter,
        request_id: &str,
    ) -> Result<ProgrammingUpdateTargetsResult, ActionError> {
        self.service.update_targets(
            ActionEnvelope {
                context: ActionContext::operator(
                    self.desk,
                    self.user.0,
                    self.session.0,
                    ActionSource::Http,
                )
                .with_request_id(request_id),
                command: ProgrammingUpdateTargetsRequest {
                    show_id: self.show_id,
                    filter,
                },
            },
            &self.active_show,
            &self.ports,
        )
    }

    fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }

    fn clear_steps(&self) {
        self.ports.steps.lock().clear();
    }

    fn set_active_contexts(&self, active: Vec<ActiveCueContext>) {
        *self.ports.active_contexts.lock() = active;
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.ports.path.display(), suffix));
        }
    }
}

struct TestPorts {
    path: PathBuf,
    show_id: ShowId,
    steps: Arc<Mutex<Vec<&'static str>>>,
    active_contexts: Arc<Mutex<Vec<ActiveCueContext>>>,
}

struct TestUnit {
    store: ShowStore,
    document: PortableShowDocument,
    steps: Arc<Mutex<Vec<&'static str>>>,
}

impl ActiveShowUnitOfWork for TestUnit {
    fn document(&self) -> &PortableShowDocument {
        self.steps.lock().push("document");
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        self.steps.lock().push("backup");
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.steps.lock().push("commit");
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))
    }
}

impl ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnit;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.steps.lock().push("begin");
        if show_id != self.show_id {
            return Err(ActionError::new(ActionErrorKind::NotFound, "inactive show"));
        }
        let store = ShowStore::open(&self.path)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        let document = store
            .portable_document()
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        Ok(TestUnit {
            store,
            document,
            steps: Arc::clone(&self.steps),
        })
    }

    fn prepare_object_undo(
        &self,
        _unit: &Self::UnitOfWork,
        _kind: &str,
        _object_id: &str,
        _expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unreachable!("Update does not use object Undo")
    }

    fn prepare_runtime(&self, snapshot: EngineSnapshot) -> Result<EngineSnapshot, ActionError> {
        self.steps.lock().push("prepare");
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _context: &ActionContext, _prepared: EngineSnapshot) {
        self.steps.lock().push("install");
    }
}

impl ProgrammingUpdatePorts for TestPorts {
    fn authorize_programming_update(&self, _context: &ActionContext) -> Result<(), ActionError> {
        self.steps.lock().push("authorize");
        Ok(())
    }

    fn active_update_cue_contexts(
        &self,
        _context: &ActionContext,
    ) -> Result<Vec<ActiveCueContext>, ActionError> {
        self.steps.lock().push("contexts");
        Ok(self.active_contexts.lock().clone())
    }

    fn reconcile_programming_update(&self, _projection: &ProgrammingUpdateProjection) {
        self.steps.lock().push("reconcile");
    }
}

fn preset_body(fixture: FixtureId, level: f32) -> Value {
    preset_body_number(fixture, level, 1)
}

fn preset_body_number(fixture: FixtureId, level: f32, number: u32) -> Value {
    let preset = Preset {
        name: format!("Intensity {number}"),
        family: PresetFamily::Intensity,
        number,
        values: HashMap::from([(
            fixture,
            HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(level))]),
        )]),
        group_values: HashMap::new(),
    };
    serde_json::to_value(preset).unwrap()
}

fn group_body() -> Value {
    group_body_with_id("front")
}

fn group_body_with_id(id: &str) -> Value {
    serde_json::to_value(GroupDefinition {
        id: id.into(),
        name: id.into(),
        ..GroupDefinition::default()
    })
    .unwrap()
}

fn cue_list_body(id: CueListId, cue: Cue) -> Value {
    serde_json::to_value(CueList {
        id,
        name: "Cuelist 1".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    })
    .unwrap()
}

struct LifecyclePorts;

impl ProgrammingPorts for LifecyclePorts {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        unreachable!("Update lifecycle test does not execute legacy commands")
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}
