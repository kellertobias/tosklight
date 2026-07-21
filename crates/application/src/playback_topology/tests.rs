use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    ActiveShowObjectChange, ActiveShowObjectKind, ActiveShowPorts, ActiveShowUnitOfWork,
    ApplicationEvent, BackupIdentity, EventBus, EventFilter, EventReplay, ShowEvent,
};
use light_core::{CueListId, ShowId};
use light_engine::EngineSnapshot;
use light_playback::{
    Cue, CueList, CueListMode, FlashReleaseMode, IntensityPriorityMode, PlaybackDefinition,
    PlaybackTarget, RestartMode, WrapMode,
};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[test]
fn save_cue_list_is_lossless_and_semantic_repetition_is_no_change() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let mut raw = serde_json::to_value(cue_list(cue_list_id, "Original")).unwrap();
    raw["future_list"] = json!({"owner":"newer-desk"});
    raw["cues"][0]["future_cue"] = json!({"shape":7});
    rig.seed("cue_list", &cue_list_id.0.to_string(), &raw);
    let mut changed: CueList = serde_json::from_value(raw.clone()).unwrap();
    changed.name = "Act One".into();
    let mut request = raw;
    request["name"] = json!(changed.name);

    let result = rig
        .handle(
            "save-1",
            rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 1,
                expected_object_id: Some(cue_list_id.0.to_string()),
                cue_list: changed.clone(),
                raw_body: Arc::new(request),
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::Changed { .. }
    ));
    let projection = result.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(projection["future_list"]["owner"], "newer-desk");
    assert_eq!(projection["cues"][0]["future_cue"]["shape"], 7);
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let result = rig
        .handle(
            "save-2",
            rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 2,
                expected_object_id: Some(cue_list_id.0.to_string()),
                cue_list: changed,
                raw_body: projection.clone(),
            },
        )
        .unwrap();
    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 1);
}

#[test]
fn storage_identity_conflicts_stop_before_topology_mutation() {
    let cue_rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let cue_list = cue_list(cue_list_id, "Original");
    let cue_body = serde_json::to_value(&cue_list).unwrap();
    cue_rig.seed("cue_list", "legacy-main", &cue_body);

    let cue_error = cue_rig
        .handle(
            "stale-cue-identity",
            cue_rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 1,
                expected_object_id: Some("replacement-main".into()),
                cue_list,
                raw_body: Arc::new(cue_body),
            },
        )
        .unwrap_err();

    assert_eq!(cue_error.kind, ActionErrorKind::Conflict);
    assert_eq!(cue_error.current_related_revision, Some(1));
    assert_eq!(cue_rig.steps(), ["authorize", "begin"]);
    assert_one_event(&cue_rig, 0);

    let slot_rig = TestRig::new();
    slot_rig.seed(
        "playback",
        "legacy-seven",
        &serde_json::to_value(playback(7, "Original")).unwrap(),
    );
    slot_rig.seed(
        "playback_page",
        "legacy-page-one",
        &json!({"number":1,"name":"Main","slots":{"1":7}}),
    );
    let slot_error = slot_rig
        .handle(
            "stale-slot-identity",
            slot_rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 1,
                expected_page_revision: 1,
                expected_page_object_id: Some("legacy-page-one".into()),
                expected_playback_revision: 1,
                expected_playback_object_id: Some("replacement-seven".into()),
                playback: playback(7, "Changed"),
            },
        )
        .unwrap_err();

    assert_eq!(slot_error.kind, ActionErrorKind::Conflict);
    assert_eq!(slot_error.current_related_revision, Some(1));
    assert_eq!(slot_rig.steps(), ["authorize", "begin"]);
    assert_one_event(&slot_rig, 0);
}

#[test]
fn canonical_storage_key_collisions_stop_before_topology_mutation() {
    let cue_rig = TestRig::new();
    let requested_id = CueListId::new();
    let occupied = cue_list(CueListId::new(), "Occupied");
    cue_rig.seed(
        "cue_list",
        &requested_id.0.to_string(),
        &serde_json::to_value(&occupied).unwrap(),
    );
    let requested = cue_list(requested_id, "Requested");
    let requested_body = serde_json::to_value(&requested).unwrap();
    let cue_revision = cue_rig.show_revision();

    let cue_error = cue_rig
        .handle(
            "occupied-cue-key",
            cue_revision,
            PlaybackTopologyAction::SaveCueList {
                cue_list_id: requested_id,
                expected_revision: 0,
                expected_object_id: None,
                cue_list: requested,
                raw_body: Arc::new(requested_body),
            },
        )
        .unwrap_err();

    assert_eq!(cue_error.kind, ActionErrorKind::Conflict);
    assert_eq!(cue_error.current_related_revision, Some(1));
    assert_eq!(cue_rig.show_revision(), cue_revision);
    let cue_document = cue_rig.document();
    assert_eq!(
        cue_document
            .object("cue_list", &requested_id.0.to_string())
            .unwrap()
            .body()["id"],
        occupied.id.0.to_string()
    );
    assert_eq!(cue_rig.steps(), ["authorize", "begin"]);
    assert_one_event(&cue_rig, 0);

    let playback_rig = TestRig::new();
    playback_rig.seed(
        "playback",
        "1",
        &serde_json::to_value(playback(7, "Occupied")).unwrap(),
    );
    playback_rig.seed(
        "playback_page",
        "legacy-page-one",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let playback_revision = playback_rig.show_revision();

    let playback_error = playback_rig
        .handle(
            "occupied-playback-key",
            playback_revision,
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 1,
                expected_page_revision: 1,
                expected_page_object_id: Some("legacy-page-one".into()),
                expected_playback_revision: 0,
                expected_playback_object_id: None,
                playback: playback(999, "Requested"),
            },
        )
        .unwrap_err();

    assert_eq!(playback_error.kind, ActionErrorKind::Conflict);
    assert_eq!(playback_error.current_related_revision, Some(1));
    assert_eq!(playback_rig.show_revision(), playback_revision);
    let playback_document = playback_rig.document();
    assert_eq!(
        playback_document.object("playback", "1").unwrap().body()["number"],
        7
    );
    assert!(
        playback_document
            .object("playback_page", "legacy-page-one")
            .unwrap()
            .body()["slots"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert_eq!(playback_rig.steps(), ["authorize", "begin"]);
    assert_one_event(&playback_rig, 0);
}

#[test]
fn configure_empty_slot_allocates_once_and_commits_playback_and_page_together() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "legacy-page-one",
        &json!({"number":1,"name":"Main","slots":{},"future":{"columns":12}}),
    );

    let result = rig
        .handle(
            "configure-1",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 4,
                expected_page_revision: 1,
                expected_page_object_id: Some("legacy-page-one".into()),
                expected_playback_revision: 0,
                expected_playback_object_id: None,
                playback: playback(999, "House"),
            },
        )
        .unwrap();

    assert_eq!(
        result.outcome.resolution(),
        PlaybackTopologyResolution::PageSlot {
            page: 1,
            slot: 4,
            playback_number: Some(1),
        }
    );
    assert_eq!(result.outcome.objects().len(), 2);
    assert_eq!(
        result.outcome.objects()[0].kind(),
        ActiveShowObjectKind::Playback
    );
    assert_eq!(result.outcome.objects()[0].raw_body().unwrap()["number"], 1);
    assert_eq!(result.outcome.objects()[1].object_id(), "legacy-page-one");
    assert_eq!(
        result.outcome.objects()[1].raw_body().unwrap()["future"]["columns"],
        12
    );
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 2);
}

#[test]
fn legacy_semantic_configure_is_no_change_without_normalizing_raw_json() {
    let rig = TestRig::new();
    let legacy = json!({
        "number":7,
        "name":"Legacy",
        "target":{"type":"grand_master"},
        "future":{"keep":true}
    });
    rig.seed("playback", "legacy-seven", &legacy);
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{"2":7}}),
    );
    let decoded: PlaybackDefinition = serde_json::from_value(legacy.clone()).unwrap();

    let result = rig
        .handle(
            "legacy-noop",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
                expected_playback_revision: 1,
                expected_playback_object_id: Some("legacy-seven".into()),
                playback: decoded,
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    let playback = result
        .outcome
        .objects()
        .iter()
        .find(|projection| projection.kind() == ActiveShowObjectKind::Playback)
        .unwrap()
        .raw_body()
        .unwrap();
    assert_eq!(playback.as_ref(), &legacy);
    assert!(playback.get("buttons").is_none());
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn changed_playback_preserves_nested_extensions_and_returns_unchanged_page_authority() {
    let rig = TestRig::new();
    let mut raw = serde_json::to_value(playback(7, "Before")).unwrap();
    raw["future_playback"] = json!({"keep":true});
    raw["target"]["future_target"] = json!([1, 2, 3]);
    rig.seed("playback", "legacy-seven", &raw);
    rig.seed(
        "playback_page",
        "page-one",
        &json!({"number":1,"name":"Main","slots":{"2":7},"future_page":true}),
    );
    let mut changed: PlaybackDefinition = serde_json::from_value(raw).unwrap();
    changed.name = "After".into();

    let result = rig
        .handle(
            "change-playback",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_page_object_id: Some("page-one".into()),
                expected_playback_revision: 1,
                expected_playback_object_id: Some("legacy-seven".into()),
                playback: changed,
            },
        )
        .unwrap();

    assert_eq!(result.outcome.objects().len(), 2);
    let playback = result.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(playback["name"], "After");
    assert_eq!(playback["future_playback"]["keep"], true);
    assert_eq!(playback["target"]["future_target"], json!([1, 2, 3]));
    assert_eq!(result.outcome.objects()[1].object_id(), "page-one");
    assert_eq!(
        result.outcome.objects()[1].raw_body().unwrap()["future_page"],
        true
    );
    assert_one_event(&rig, 1);
}

#[test]
fn map_existing_is_page_only_lossless_replayable_and_semantically_idempotent() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Mapped source")).unwrap(),
    );
    let mut source = serde_json::to_value(cue_list_playback(12, cue_list_id)).unwrap();
    source["future_playback"] = json!({"keep":"source"});
    source.as_object_mut().unwrap().remove("color");
    rig.seed("playback", "legacy-twelve", &source);
    rig.seed(
        "playback_page",
        "legacy-page-two",
        &json!({"number":2,"name":"Wing","slots":{},"future_page":{"columns":8}}),
    );
    let action = map_existing_action(2, 4, 12, 1, "legacy-page-two");
    let before = rig.show_revision();

    let first = rig.handle("map-existing", before, action.clone()).unwrap();

    assert!(matches!(
        first.outcome,
        PlaybackTopologyOutcome::Changed { .. }
    ));
    assert_eq!(first.outcome.objects().len(), 1);
    assert_eq!(
        first.outcome.objects()[0].kind(),
        ActiveShowObjectKind::PlaybackPage
    );
    assert_eq!(first.outcome.objects()[0].object_id(), "legacy-page-two");
    let page = first.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(page["slots"]["4"], 12);
    assert_eq!(page["future_page"]["columns"], 8);
    let document = rig.document();
    let stored_source = document.object("playback", "legacy-twelve").unwrap();
    assert_eq!(stored_source.revision(), 1);
    assert_eq!(stored_source.body()["future_playback"]["keep"], "source");
    assert!(stored_source.body().get("color").is_none());
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let replay = rig.handle("map-existing", before, action).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let no_change = rig
        .handle(
            "map-existing-no-change",
            rig.show_revision(),
            map_existing_action(2, 4, 12, 2, "legacy-page-two"),
        )
        .unwrap();
    assert!(matches!(
        no_change.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(no_change.outcome.objects().len(), 1);
    assert_eq!(no_change.outcome.event_sequence(), None);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 1);
}

#[test]
fn map_existing_creates_default_page_without_rewriting_source() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Source")).unwrap(),
    );
    rig.seed(
        "playback",
        "source-seven",
        &serde_json::to_value(cue_list_playback(7, cue_list_id)).unwrap(),
    );

    let outcome = rig
        .handle(
            "map-new-page",
            rig.show_revision(),
            PlaybackTopologyAction::MapExistingPlayback {
                page: 3,
                slot: 6,
                playback_number: 7,
                expected_page_revision: 0,
                expected_page_object_id: None,
                expected_playback_revision: 1,
                expected_playback_object_id: Some("source-seven".into()),
            },
        )
        .unwrap();

    assert_eq!(outcome.outcome.objects().len(), 1);
    let page = outcome.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(page["number"], 3);
    assert_eq!(page["name"], "Page 3");
    assert_eq!(page["slots"]["6"], 7);
    assert_eq!(
        rig.document()
            .object("playback", "source-seven")
            .unwrap()
            .revision(),
        1
    );
    assert_one_event(&rig, 1);
}

#[test]
fn map_existing_rejects_an_occupied_default_page_storage_identity() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Source")).unwrap(),
    );
    rig.seed(
        "playback",
        "source-seven",
        &serde_json::to_value(cue_list_playback(7, cue_list_id)).unwrap(),
    );
    rig.seed(
        "playback_page",
        "3",
        &json!({"number":9,"name":"Legacy Page","slots":{}}),
    );
    let revision = rig.show_revision();

    let error = rig
        .handle(
            "map-page-key-collision",
            revision,
            PlaybackTopologyAction::MapExistingPlayback {
                page: 3,
                slot: 6,
                playback_number: 7,
                expected_page_revision: 0,
                expected_page_object_id: None,
                expected_playback_revision: 1,
                expected_playback_object_id: Some("source-seven".into()),
            },
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(revision));
    assert_eq!(error.current_related_revision, Some(1));
    assert_eq!(rig.show_revision(), revision);
    let document = rig.document();
    let occupied = document.object("playback_page", "3").unwrap();
    assert_eq!(occupied.body()["number"], 9);
    assert_eq!(occupied.revision(), 1);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn map_existing_rejects_stale_source_identity_and_non_cuelist_target() {
    let rig = TestRig::new();
    rig.seed(
        "playback",
        "legacy-seven",
        &serde_json::to_value(playback(7, "Grand master")).unwrap(),
    );
    let revision = rig.show_revision();
    let stale = PlaybackTopologyAction::MapExistingPlayback {
        page: 1,
        slot: 1,
        playback_number: 7,
        expected_page_revision: 0,
        expected_page_object_id: None,
        expected_playback_revision: 1,
        expected_playback_object_id: Some("replacement-seven".into()),
    };

    let error = rig.handle("map-stale-source", revision, stale).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_related_revision, Some(1));
    assert_one_event(&rig, 0);

    rig.clear_steps();
    let invalid = rig
        .handle(
            "map-wrong-target",
            revision,
            PlaybackTopologyAction::MapExistingPlayback {
                page: 1,
                slot: 1,
                playback_number: 7,
                expected_page_revision: 0,
                expected_page_object_id: None,
                expected_playback_revision: 1,
                expected_playback_object_id: Some("legacy-seven".into()),
            },
        )
        .unwrap_err();
    assert_eq!(invalid.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.show_revision(), revision);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn clear_removes_one_playback_from_every_page_in_one_event() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Keep source")).unwrap(),
    );
    let mut removed = playback(9, "Clear me");
    removed.target = PlaybackTarget::CueList { cue_list_id };
    removed.buttons = PlaybackDefinition::default_buttons(&removed.target);
    removed.fader = PlaybackDefinition::default_fader(&removed.target);
    rig.seed(
        "playback",
        "legacy-nine",
        &serde_json::to_value(removed).unwrap(),
    );
    rig.seed(
        "playback",
        "3",
        &serde_json::to_value(playback(3, "Keep me")).unwrap(),
    );
    rig.seed(
        "playback_page",
        "page-a",
        &json!({"number":1,"name":"One","slots":{"1":9,"2":3},"future":"a"}),
    );
    rig.seed(
        "playback_page",
        "page-b",
        &json!({"number":2,"name":"Two","slots":{"7":9},"future":"b"}),
    );
    let before = rig.show_revision();

    let result = rig
        .handle(
            "clear-1",
            before,
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 1,
                expected_page_revision: 1,
                expected_page_object_id: Some("page-a".into()),
                expected_playback_revision: 1,
                expected_playback_object_id: Some("legacy-nine".into()),
            },
        )
        .unwrap();

    assert_eq!(result.outcome.show_revision().value(), before + 1);
    assert_eq!(result.outcome.objects().len(), 3);
    assert!(result.outcome.objects().iter().any(|projection| matches!(
        projection,
        PlaybackTopologyObjectProjection::Deleted {
            kind: ActiveShowObjectKind::Playback,
            object_id,
            object_revision: 2,
        } if object_id == "legacy-nine"
    )));
    let document = rig.document();
    assert!(document.object("playback", "legacy-nine").is_none());
    assert!(
        document
            .object("cue_list", &cue_list_id.0.to_string())
            .is_some()
    );
    for page in document.objects_of_kind("playback_page") {
        let body: light_playback::PlaybackPage =
            serde_json::from_value(page.body().clone()).unwrap();
        assert!(!body.slots.values().any(|number| *number == 9));
        assert!(page.body().get("future").is_some());
    }
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 3);
}

#[test]
fn empty_clear_is_no_change_and_does_not_prepare_or_emit() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );

    let result = rig
        .handle(
            "clear-empty",
            rig.show_revision(),
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
                expected_playback_revision: 0,
                expected_playback_object_id: None,
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(result.outcome.event_sequence(), None);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn exact_replay_returns_original_authority_without_a_second_transaction_or_event() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let action = PlaybackTopologyAction::ConfigureSlot {
        page: 1,
        slot: 1,
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),
        expected_playback_revision: 0,
        expected_playback_object_id: None,
        playback: playback(0, "Replay"),
    };
    let expected = rig.show_revision();
    let first = rig.handle("replay", expected, action.clone()).unwrap();
    rig.clear_steps();

    let replay = rig.handle("replay", expected, action).unwrap();

    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(replay.correlation_id, first.correlation_id);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_one_event(&rig, 2);
}

#[test]
fn show_object_and_request_conflicts_stop_before_side_effects() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let action = PlaybackTopologyAction::ConfigureSlot {
        page: 1,
        slot: 1,
        expected_page_revision: 0,
        expected_page_object_id: Some("1".into()),
        expected_playback_revision: 0,
        expected_playback_object_id: None,
        playback: playback(0, "Conflict"),
    };
    let error = rig
        .handle("object-conflict", rig.show_revision(), action)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(rig.show_revision()));
    assert_eq!(error.current_related_revision, Some(1));
    assert_eq!(rig.steps(), ["authorize", "begin"]);

    rig.clear_steps();
    let stale = rig
        .handle(
            "show-conflict",
            0,
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 1,
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
                expected_playback_revision: 0,
                expected_playback_object_id: None,
            },
        )
        .unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(rig.show_revision()));
    assert_eq!(rig.steps(), ["authorize", "begin"]);

    rig.clear_steps();
    let current = rig.show_revision();
    let noop = PlaybackTopologyAction::ClearMappedPlayback {
        page: 1,
        slot: 1,
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),
        expected_playback_revision: 0,
        expected_playback_object_id: None,
    };
    rig.handle("collision", current, noop.clone()).unwrap();
    rig.clear_steps();
    let collision = rig
        .handle(
            "collision",
            current,
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
                expected_playback_revision: 0,
                expected_playback_object_id: None,
            },
        )
        .unwrap_err();
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.steps(), ["authorize"]);
}

#[test]
fn replay_identity_isolated_by_user_desk_and_session() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let action = PlaybackTopologyAction::ClearMappedPlayback {
        page: 1,
        slot: 1,
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),
        expected_playback_revision: 0,
        expected_playback_object_id: None,
    };
    let show_revision = rig.show_revision();
    for (user, desk, session) in [(2, 1, 3), (20, 1, 3), (2, 10, 3), (2, 1, 30)] {
        let result = rig
            .handle_as(
                "shared-id",
                show_revision,
                action.clone(),
                user,
                desk,
                session,
            )
            .unwrap();
        assert!(!result.replayed);
    }
    assert_eq!(
        rig.steps(),
        [
            "authorize",
            "begin",
            "authorize",
            "begin",
            "authorize",
            "begin",
            "authorize",
            "begin"
        ]
    );
}

#[test]
fn request_actor_session_and_exact_show_revision_are_required_before_opening_show() {
    let rig = TestRig::new();
    let action = PlaybackTopologyAction::ClearMappedPlayback {
        page: 1,
        slot: 1,
        expected_page_revision: 0,
        expected_page_object_id: None,
        expected_playback_revision: 0,
        expected_playback_object_id: None,
    };
    let operator = ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        ActionSource::Http,
    );
    let missing_request = rig
        .handle_context(operator.clone(), action.clone())
        .unwrap_err();
    assert_eq!(missing_request.kind, ActionErrorKind::Invalid);
    rig.clear_steps();
    let missing_revision = rig
        .handle_context(operator.with_request_id("missing-revision"), action.clone())
        .unwrap_err();
    assert_eq!(missing_revision.kind, ActionErrorKind::Invalid);
    rig.clear_steps();
    let missing_actor = rig
        .handle_context(
            ActionContext::system(Uuid::from_u128(1), ActionSource::System)
                .with_request_id("system")
                .with_expected_revision(0),
            action,
        )
        .unwrap_err();
    assert_eq!(missing_actor.kind, ActionErrorKind::Unauthorized);
    assert_eq!(rig.steps(), ["authorize"]);
}

fn mutation_steps() -> [&'static str; 7] {
    [
        "authorize",
        "begin",
        "prepare",
        "backup",
        "commit",
        "install",
        "reconcile",
    ]
}

fn assert_one_event(rig: &TestRig, object_changes: usize) {
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained events")
    };
    if object_changes == 0 {
        assert!(events.is_empty());
        return;
    }
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) = &events[0].payload else {
        panic!("expected Show Objects event")
    };
    assert_eq!(change.changes.len(), object_changes);
}

struct TestRig {
    service: PlaybackTopologyService,
    ports: TestPorts,
    show_id: ShowId,
}

impl TestRig {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("light-playback-topology-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "Playback topology test").unwrap();
        drop(store);
        let active_show = crate::ActiveShowService::new(EventBus::new(32));
        Self {
            service: PlaybackTopologyService::new(active_show),
            ports: TestPorts {
                path,
                show_id,
                steps: Arc::default(),
            },
            show_id,
        }
    }

    fn seed(&self, kind: &str, id: &str, body: &Value) {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object(kind, id, body, 0)
            .unwrap();
    }

    fn document(&self) -> PortableShowDocument {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn show_revision(&self) -> u64 {
        self.document().revision().value()
    }

    fn handle(
        &self,
        request_id: &str,
        show_revision: u64,
        action: PlaybackTopologyAction,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.handle_as(request_id, show_revision, action, 2, 1, 3)
    }

    fn handle_as(
        &self,
        request_id: &str,
        show_revision: u64,
        action: PlaybackTopologyAction,
        user: u128,
        desk: u128,
        session: u128,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.handle_context(
            ActionContext::operator(
                Uuid::from_u128(desk),
                Uuid::from_u128(user),
                Uuid::from_u128(session),
                ActionSource::Http,
            )
            .with_request_id(request_id)
            .with_expected_revision(show_revision),
            action,
        )
    }

    fn handle_context(
        &self,
        context: ActionContext,
        action: PlaybackTopologyAction,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.service.handle(
            ActionEnvelope {
                context,
                command: PlaybackTopologyCommand {
                    show_id: self.show_id,
                    action,
                },
            },
            &self.ports,
        )
    }

    fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }

    fn clear_steps(&self) {
        self.ports.steps.lock().clear();
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
}

struct TestUnit {
    store: ShowStore,
    document: PortableShowDocument,
    steps: Arc<Mutex<Vec<&'static str>>>,
}

impl ActiveShowUnitOfWork for TestUnit {
    fn document(&self) -> &PortableShowDocument {
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
        unreachable!("Playback topology does not use object Undo")
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

impl PlaybackTopologyPorts for TestPorts {
    fn authorize_playback_topology(&self, _context: &ActionContext) -> Result<(), ActionError> {
        self.steps.lock().push("authorize");
        Ok(())
    }

    fn reconcile_playback_topology(&self, _changes: &[ActiveShowObjectChange]) {
        self.steps.lock().push("reconcile");
    }
}

fn playback(number: u16, name: &str) -> PlaybackDefinition {
    let target = PlaybackTarget::GrandMaster;
    PlaybackDefinition {
        number,
        name: name.into(),
        buttons: PlaybackDefinition::default_buttons(&target),
        fader: PlaybackDefinition::default_fader(&target),
        target,
        button_count: 3,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn cue_list_playback(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
    let mut value = playback(number, "Cuelist");
    value.target = PlaybackTarget::CueList { cue_list_id };
    value.buttons = PlaybackDefinition::default_buttons(&value.target);
    value.fader = PlaybackDefinition::default_fader(&value.target);
    value
}

fn map_existing_action(
    page: u8,
    slot: u8,
    playback_number: u16,
    page_revision: u64,
    page_object_id: &str,
) -> PlaybackTopologyAction {
    PlaybackTopologyAction::MapExistingPlayback {
        page,
        slot,
        playback_number,
        expected_page_revision: page_revision,
        expected_page_object_id: Some(page_object_id.into()),
        expected_playback_revision: 1,
        expected_playback_object_id: Some("legacy-twelve".into()),
    }
}

fn cue_list(id: CueListId, name: &str) -> CueList {
    CueList {
        id,
        name: name.into(),
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
        cues: vec![Cue::new(1.0)],
    }
}
