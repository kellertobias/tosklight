//! TL-572: `SelectiveShowImportService::undo` with requested semantic intent.
//!
//! `apply::selective_import_undo_restores_replaced_objects_and_deletes_created_objects_once`
//! covers the generic Macro inverse. This module adds only the semantic gaps: replaced semantic
//! Presets are restored to their exact requested intent, added semantic Presets, Cue list, Group
//! and fixtures are removed, unrelated destination edits survive, and every rejected or failed
//! undo leaves the show (objects and revision) untouched. The existing policy is reused as is:
//! an undo is rejected as a Conflict when any object it would touch has a different revision.
use super::semantic_intent::{
    CUE_LIST_ID, beam_preset, color_preset, cue_list, key_of, position_preset,
};
use super::support::*;
use crate::programming::semantic_intent_cases::*;
use crate::selective_import::*;
use crate::{
    ActionEnvelope, ActionErrorKind, ActiveShowObjectBody, ActiveShowObjectKind,
    ActiveShowObjectMutation, ActiveShowObjectMutationKind, MutateActiveShowObjectsCommand,
};
use light_core::programming::ProgrammingOwner;
use light_programmer::{GroupDefinition, Preset};
use light_show::PortableShowObjectKey;
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    sync::atomic::Ordering,
};

/// The Group id the shared TL-567 Preset and Cue builders reference.
const GROUP: &str = "semantic-front";
const UNRELATED: &str = "4.9";

struct Imported {
    rig: TestRig,
    /// Destination bodies replaced by the import, exactly as they were before it.
    replaced: Vec<(&'static str, Preset, Value)>,
    added: BTreeSet<PortableShowObjectKey>,
    undo: SelectiveShowImportUndoTarget,
}

fn preset_body(preset: &Preset) -> Value {
    serde_json::to_value(preset).unwrap()
}

fn decoded_preset(body: &Value) -> Preset {
    let decoded = ActiveShowObjectBody::decode(ActiveShowObjectKind::Preset, body.clone()).unwrap();
    decoded.preset().expect("typed Preset").typed().clone()
}

/// A destination Preset that carries only universal semantic intent, so it needs no fixture.
fn universal_only(
    base: Preset,
    owner: ProgrammingOwner,
    value: light_core::AttributeValue,
) -> Preset {
    Preset {
        values: HashMap::new(),
        group_values: HashMap::new(),
        universal_values: HashMap::from([(key_of(owner), value)]),
        ..base
    }
}

fn unrelated_beam(zoom_only: bool) -> Preset {
    let mut preset = universal_only(
        beam_preset(portable_fixture_record(574_000, 9).fixture_id),
        ProgrammingOwner::Zoom,
        zoom(),
    );
    preset.number = 9;
    preset.name = "Unrelated beam".into();
    if !zoom_only {
        preset
            .universal_values
            .insert(key_of(ProgrammingOwner::Focus), focus());
    }
    preset
}

/// Imports the TL-567 semantic bundle over two occupied semantic Presets with ReplaceDestination.
fn import_over_semantic_destination() -> Imported {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(575_000, 1);
    let point = portable_fixture_record(576_000, 2);
    for record in [&fixture, &point] {
        rig.source_profile(&record.profile);
        rig.source_object(
            "patched_fixture",
            &record.fixture_id.0.to_string(),
            record.body.clone(),
        );
    }
    rig.source_object(
        "group",
        GROUP,
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: "Front".into(),
            fixtures: vec![fixture.fixture_id],
            ..Default::default()
        })
        .unwrap(),
    );
    let id = fixture.fixture_id;
    for (key, preset) in [
        ("2.1", color_preset(id)),
        ("3.1", position_preset(id, point.fixture_id.0)),
        ("4.1", beam_preset(id)),
    ] {
        rig.source_object("preset", key, preset_body(&preset));
    }
    rig.source_object("cue_list", CUE_LIST_ID, cue_list(id, point.fixture_id.0));

    let replaced = vec![
        (
            "2.1",
            universal_only(
                color_preset(id),
                ProgrammingOwner::Color,
                color(warm_white_zero_output()),
            ),
        ),
        (
            "3.1",
            universal_only(
                position_preset(id, point.fixture_id.0),
                ProgrammingOwner::Position,
                angles(),
            ),
        ),
    ]
    .into_iter()
    .map(|(key, preset)| {
        let body = preset_body(&preset);
        rig.target_object("preset", key, body.clone());
        (key, preset, body)
    })
    .collect::<Vec<_>>();
    rig.target_object("preset", UNRELATED, preset_body(&unrelated_beam(true)));

    let mut request = SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        [
            key("cue_list", CUE_LIST_ID),
            key("preset", "2.1"),
            key("preset", "3.1"),
            key("preset", "4.1"),
        ],
    );
    for (preset, ..) in &replaced {
        request = request.resolve(
            key("preset", preset),
            ImportConflictResolution::ReplaceDestination,
        );
    }
    let preview = rig.preview(request);
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let applied = rig.apply(&preview).unwrap();
    let undo = applied.undo.expect("a changed import has an undo target");

    let added = [
        key("preset", "4.1"),
        key("cue_list", CUE_LIST_ID),
        key("group", GROUP),
        key("patched_fixture", &fixture.fixture_id.0.to_string()),
        key("patched_fixture", &point.fixture_id.0.to_string()),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut expected_keys = added.clone();
    expected_keys.extend(replaced.iter().map(|(preset, ..)| key("preset", preset)));
    let undo_keys = undo
        .objects
        .iter()
        .map(|object| object.key.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(undo_keys, expected_keys);
    for object in &undo.objects {
        assert_eq!(
            object.previous_body.is_none(),
            added.contains(&object.key),
            "{:?}",
            object.key
        );
    }
    // The import really wrote the source intent over the occupied destination Presets.
    let document = rig.target_document();
    let imported = decoded_preset(document.object("preset", "2.1").unwrap().body());
    assert_eq!(imported.universal_values, color_preset(id).universal_values);
    Imported {
        rig,
        replaced,
        added,
        undo,
    }
}

fn put_preset(rig: &TestRig, id: &str, preset: &Preset) -> u64 {
    let revision = rig
        .target_document()
        .object("preset", id)
        .expect("existing Preset")
        .revision();
    mutate(
        rig,
        ActiveShowObjectKind::Preset,
        id,
        revision,
        Some(preset_body(preset)),
    )
}

fn mutate(
    rig: &TestRig,
    kind: ActiveShowObjectKind,
    id: &str,
    expected_object_revision: u64,
    body: Option<Value>,
) -> u64 {
    let mutation = match body {
        Some(body) => ActiveShowObjectMutationKind::Put {
            body: Box::new(ActiveShowObjectBody::decode(kind, body).unwrap()),
        },
        None => ActiveShowObjectMutationKind::Delete,
    };
    rig.active_show
        .mutate_objects(
            ActionEnvelope {
                context: context(),
                command: MutateActiveShowObjectsCommand {
                    show_id: rig.target_id,
                    mutations: vec![ActiveShowObjectMutation {
                        kind,
                        object_id: id.into(),
                        expected_object_revision,
                        mutation,
                    }],
                },
            },
            &rig.ports,
        )
        .unwrap()
        .show_revision
        .value()
}

#[test]
fn semantic_import_undo_restores_replaced_intent_removes_added_objects_and_keeps_unrelated_edits() {
    let Imported {
        rig,
        replaced,
        added,
        undo,
    } = import_over_semantic_destination();
    // Unrelated destination edits after the import: one existing Preset changes its intent and a
    // new semantic Preset is recorded. Neither is part of the import undo target.
    put_preset(&rig, UNRELATED, &unrelated_beam(false));
    let recorded = Preset {
        number: 9,
        name: "Recorded after import".into(),
        ..universal_only(
            color_preset(portable_fixture_record(577_000, 3).fixture_id),
            ProgrammingOwner::Color,
            color(uv_only_black()),
        )
    };
    let edited_revision = mutate(
        &rig,
        ActiveShowObjectKind::Preset,
        "2.9",
        0,
        Some(preset_body(&recorded)),
    );
    let unrelated_body = rig
        .target_document()
        .object("preset", UNRELATED)
        .unwrap()
        .body()
        .clone();

    let undone = rig.service.undo(&context(), &undo, &rig.ports).unwrap();

    assert_eq!(undone, edited_revision + 1);
    let document = rig.target_document();
    assert_eq!(document.revision().value(), undone);
    for (key, preset, body) in &replaced {
        let restored = document.object("preset", key).unwrap().body();
        assert_persisted_eq(restored, body);
        let restored = decoded_preset(restored);
        assert_eq!(restored.universal_values, preset.universal_values, "{key}");
        assert!(restored.values.is_empty() && restored.group_values.is_empty());
        assert_semantic_attribute_map(
            &document.object("preset", key).unwrap().body()["universal_values"],
            &["color", "position"],
        );
    }
    for key in &added {
        assert!(document.object(key.kind(), key.id()).is_none(), "{key:?}");
    }
    assert_persisted_eq(
        document.object("preset", UNRELATED).unwrap().body(),
        &unrelated_body,
    );
    let unrelated = decoded_preset(document.object("preset", UNRELATED).unwrap().body());
    assert_eq!(
        unrelated.universal_values,
        unrelated_beam(false).universal_values
    );
    let kept = decoded_preset(document.object("preset", "2.9").unwrap().body());
    assert_eq!(kept.universal_values, recorded.universal_values);
    let installed = rig.ports.installed.lock();
    let snapshot = installed.as_ref().expect("undo installs its candidate");
    assert!(
        snapshot
            .cue_lists
            .iter()
            .all(|list| list.name != "Semantic")
    );
    drop(installed);

    // The inverse is consumed: repeating it is rejected without touching the restored show.
    rig.clear_steps();
    let error = rig.service.undo(&context(), &undo, &rig.ports).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.target_document(), document);
    assert!(!rig.steps().contains(&"commit"));
}

/// Names of the Cue lists in the installed runtime, as the runtime identity of one install.
fn installed_cue_lists(rig: &TestRig) -> Option<Vec<String>> {
    rig.ports.installed.lock().as_ref().map(|snapshot| {
        snapshot
            .cue_lists
            .iter()
            .map(|list| format!("{}:{:?}", list.name, list.cues[0].changes))
            .collect()
    })
}

enum Rejection {
    EditedReplacedPreset,
    EditedAddedPreset,
    DeletedAddedCueList,
    RuntimeFailure,
    CommitFailure,
}

/// Every rejected or failed semantic undo is atomic: no object, requested intent or show
/// revision changes, the runtime is not reinstalled, and the conflicting edit keeps its intent.
#[test]
fn rejected_semantic_import_undo_is_atomic() {
    for rejection in [
        Rejection::EditedReplacedPreset,
        Rejection::EditedAddedPreset,
        Rejection::DeletedAddedCueList,
        Rejection::RuntimeFailure,
        Rejection::CommitFailure,
    ] {
        let Imported { rig, undo, .. } = import_over_semantic_destination();
        let edited = universal_only(
            color_preset(portable_fixture_record(578_000, 4).fixture_id),
            ProgrammingOwner::Color,
            color(magenta_with_uv()),
        );
        let label = match rejection {
            Rejection::EditedReplacedPreset => {
                put_preset(&rig, "2.1", &edited);
                "edited replaced"
            }
            Rejection::EditedAddedPreset => {
                let mut beam = beam_preset(portable_fixture_record(578_000, 4).fixture_id);
                beam.values.clear();
                beam.group_values.clear();
                put_preset(&rig, "4.1", &beam);
                "edited added"
            }
            Rejection::DeletedAddedCueList => {
                let revision = rig
                    .target_document()
                    .object("cue_list", CUE_LIST_ID)
                    .unwrap()
                    .revision();
                mutate(
                    &rig,
                    ActiveShowObjectKind::CueList,
                    CUE_LIST_ID,
                    revision,
                    None,
                );
                "deleted added"
            }
            Rejection::RuntimeFailure => {
                rig.ports.fail_prepare.store(true, Ordering::SeqCst);
                "runtime"
            }
            Rejection::CommitFailure => {
                rig.ports.fail_commit.store(true, Ordering::SeqCst);
                "commit"
            }
        };
        let before = rig.target_document();
        let installed_before = installed_cue_lists(&rig);
        rig.clear_steps();

        let error = rig.service.undo(&context(), &undo, &rig.ports).unwrap_err();

        let after = rig.target_document();
        assert_eq!(after, before, "{label}");
        assert_eq!(after.revision(), before.revision(), "{label}");
        assert_eq!(installed_cue_lists(&rig), installed_before, "{label}");
        assert!(!rig.steps().contains(&"install"), "{label}");
        match rejection {
            Rejection::RuntimeFailure | Rejection::CommitFailure => {
                assert_ne!(error.kind, ActionErrorKind::Conflict, "{label}");
                rig.ports.fail_prepare.store(false, Ordering::SeqCst);
                rig.ports.fail_commit.store(false, Ordering::SeqCst);
                // The failure consumed nothing: the same inverse still applies afterwards.
                rig.service.undo(&context(), &undo, &rig.ports).unwrap();
                assert!(rig.target_document().object("preset", "4.1").is_none());
            }
            _ => {
                assert_eq!(error.kind, ActionErrorKind::Conflict, "{label}");
                assert!(!rig.steps().contains(&"commit"), "{label}");
                assert!(after.object("group", GROUP).is_some(), "{label}");
                if matches!(rejection, Rejection::EditedReplacedPreset) {
                    let stored = decoded_preset(after.object("preset", "2.1").unwrap().body());
                    assert_eq!(stored.universal_values, edited.universal_values);
                }
            }
        }
    }
}
