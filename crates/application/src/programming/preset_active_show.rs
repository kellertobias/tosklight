use super::{
    ProgrammingPresetActiveShowPorts, ProgrammingPresetCommit, ProgrammingPresetCommitResult,
    ProgrammingPresetProjection, ProgrammingPresetRevisionExpectation,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft, lossless_json,
    prepare_show_candidate,
};
use light_programmer::{Preset, PresetAddress};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowRevision};
use std::sync::Arc;

impl ActiveShowService {
    /// Commits one captured normal Programmer Preset through the centralized active-show lifecycle.
    pub fn commit_programming_preset<P>(
        &self,
        context: &ActionContext,
        commit: &ProgrammingPresetCommit,
        ports: &P,
    ) -> Result<ProgrammingPresetCommitResult, ActionError>
    where
        P: ProgrammingPresetActiveShowPorts,
    {
        self.transact(
            context,
            commit.show_id,
            ports,
            "record-preset",
            |document| prepare_recording(document, commit),
            complete_recording,
        )
    }
}

fn prepare_recording(
    document: &PortableShowDocument,
    commit: &ProgrammingPresetCommit,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    validate_show(document, commit)?;
    let existing = find_preset(document, commit.address)?;
    validate_revision(existing.as_ref().map(|(object, _)| *object), commit)?;
    let preset = commit.merged_with(existing.as_ref().map(|(_, preset)| preset))?;
    let object_id = existing.as_ref().map_or_else(
        || commit.address.storage_key(),
        |(object, _)| object.key().id().to_owned(),
    );
    let raw_body = merged_body(existing.as_ref(), &preset)?;
    let current_revision = existing.as_ref().map_or(0, |(object, _)| object.revision());
    if existing
        .as_ref()
        .is_some_and(|(object, _)| object.body() == &raw_body)
    {
        return Ok(PreparedActiveShowTransaction::NoChange(PreparedRecording {
            result: completion(
                document.revision(),
                commit,
                object_id,
                current_revision,
                raw_body,
                false,
            ),
        }));
    }
    let mut transaction = document.transaction();
    transaction.put("preset", object_id.clone(), raw_body.clone());
    let prepared = prepare_show_candidate(document, transaction)?;
    let (show_revision, object_revision, raw_body) = {
        let candidate = document
            .candidate(prepared.transaction())
            .map_err(invalid)?;
        let object = candidate.object("preset", &object_id).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Internal,
                "prepared Preset is missing from its authoritative candidate",
            )
        })?;
        (
            candidate.revision(),
            object.revision(),
            object.body().clone(),
        )
    };
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedRecording {
            result: completion(
                show_revision,
                commit,
                object_id,
                object_revision,
                raw_body,
                true,
            ),
        },
    })
}

fn complete_recording<P: ProgrammingPresetActiveShowPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedRecording>,
) -> ProgrammingPresetCommitResult {
    let mut result = completed.state.result;
    let Some(commit) = completed.commit else {
        return result;
    };
    result.show_revision = commit.revision();
    ports.reconcile_programming_preset(&result.projection);
    let change = ActiveShowObjectChange {
        kind: ActiveShowObjectKind::Preset,
        object_id: result.projection.object_id.clone(),
        object_revision: result.projection.object_revision,
        body: Some(result.projection.raw_body.as_ref().clone()),
        deleted: false,
    };
    result.event_sequence = Some(
        events
            .publish(EventDraft::active_show_objects_changed(
                context,
                ActiveShowObjectsChange {
                    show_id: result.projection.show_id,
                    show_revision: result.show_revision,
                    changes: vec![change],
                },
            ))
            .sequence,
    );
    result
}

struct PreparedRecording {
    result: ProgrammingPresetCommitResult,
}

fn completion(
    show_revision: PortableShowRevision,
    commit: &ProgrammingPresetCommit,
    object_id: String,
    object_revision: u64,
    raw_body: serde_json::Value,
    changed: bool,
) -> ProgrammingPresetCommitResult {
    ProgrammingPresetCommitResult {
        changed,
        projection: ProgrammingPresetProjection {
            show_id: commit.show_id,
            object_id,
            address: commit.address,
            object_revision,
            raw_body: Arc::new(raw_body),
        },
        show_revision,
        event_sequence: None,
    }
}

fn validate_show(
    document: &PortableShowDocument,
    commit: &ProgrammingPresetCommit,
) -> Result<(), ActionError> {
    if document.id() != commit.show_id {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "requested show is not active",
        ));
    }
    if let Some(expected) = commit.expected_show_revision
        && expected != document.revision()
    {
        return Err(
            ActionError::new(ActionErrorKind::Conflict, "stale active-show revision")
                .at_related_revision(document.revision().value()),
        );
    }
    Ok(())
}

fn validate_revision(
    existing: Option<&PortableShowObject>,
    commit: &ProgrammingPresetCommit,
) -> Result<(), ActionError> {
    let current = existing.map_or(0, PortableShowObject::revision);
    match commit.expected_object_revision {
        ProgrammingPresetRevisionExpectation::Current => Ok(()),
        ProgrammingPresetRevisionExpectation::Exact(expected) if expected == current => Ok(()),
        ProgrammingPresetRevisionExpectation::Exact(_) => Err(ActionError::new(
            ActionErrorKind::Conflict,
            "stale Preset object revision",
        )
        .at_revision(current)),
    }
}

fn find_preset(
    document: &PortableShowDocument,
    address: PresetAddress,
) -> Result<Option<(&PortableShowObject, Preset)>, ActionError> {
    let canonical = address.storage_key();
    // The canonical pool key is authoritative. Legacy aliases are considered only when it is
    // absent, which preserves deterministic compatibility without letting an alias shadow it.
    if let Some(object) = document.object("preset", &canonical) {
        let (_, preset) = decode_preset(object)?;
        return Ok(Some((object, preset)));
    }
    let mut found = None;
    for object in document.objects_of_kind("preset") {
        let decoded = decode_preset(object);
        if decoded.as_ref().is_ok_and(|preset| preset.0 == address) {
            let (_, preset) = decoded?;
            if found.replace((object, preset)).is_some() {
                return Err(ActionError::new(
                    ActionErrorKind::Invalid,
                    "multiple legacy Presets resolve to the requested address",
                ));
            }
        }
    }
    Ok(found)
}

fn decode_preset(object: &PortableShowObject) -> Result<(PresetAddress, Preset), ActionError> {
    let mut preset = serde_json::from_value::<Preset>(object.body().clone()).map_err(invalid)?;
    let address = preset
        .reconcile_address(object.key().id())
        .map_err(invalid)?;
    Ok((address, preset))
}

fn merged_body(
    existing: Option<&(&PortableShowObject, Preset)>,
    preset: &Preset,
) -> Result<serde_json::Value, ActionError> {
    match existing {
        Some((object, before)) => {
            lossless_json::merge_typed(object.body(), before, preset).map_err(invalid)
        }
        None => serde_json::to_value(preset).map_err(invalid),
    }
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgrammingPresetRecordRequest;
    use light_core::{AttributeKey, AttributeValue};
    use light_programmer::{PresetFamily, PresetStoreMode};
    use light_show::ShowStore;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn malformed_unrelated_preset_does_not_block_legacy_lookup() {
        let document = TestDocument::new([
            ("2.99", json!({"family":"Color","values":"invalid"})),
            ("01", preset_body("Legacy", 0.2)),
        ]);
        let found = find_preset(&document.document, color_one())
            .unwrap()
            .unwrap();
        assert_eq!(found.0.key().id(), "01");
    }

    #[test]
    fn canonical_key_wins_over_a_legacy_alias() {
        let document = TestDocument::new([
            ("01", preset_body("Legacy", 0.2)),
            ("2.1", preset_body("Canonical", 0.4)),
        ]);
        let found = find_preset(&document.document, color_one())
            .unwrap()
            .unwrap();
        assert_eq!(found.0.key().id(), "2.1");
        assert_eq!(found.1.name, "Canonical");
    }

    #[test]
    fn prepared_recording_preserves_legacy_id_and_unknown_fields() {
        let mut body = preset_body("Legacy", 0.2);
        body["future"] = json!({"keep":true});
        let document = TestDocument::new([("01", body)]);
        let commit = recording(&document, 0.8);
        let PreparedActiveShowTransaction::PreparedCommit { state, .. } =
            prepare_recording(&document.document, &commit).unwrap()
        else {
            panic!("changed capture should prepare one commit")
        };
        assert_eq!(state.result.projection.object_id, "01");
        assert_eq!(state.result.projection.raw_body["future"]["keep"], true);
    }

    #[test]
    fn multiple_legacy_aliases_for_one_address_are_rejected() {
        let document = TestDocument::new([
            ("1", preset_body("First", 0.2)),
            ("01", preset_body("Second", 0.4)),
        ]);
        let error = find_preset(&document.document, color_one()).unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Invalid);
        assert!(error.message.contains("multiple legacy Presets"));
    }

    struct TestDocument {
        path: PathBuf,
        document: PortableShowDocument,
    }

    impl TestDocument {
        fn new<const N: usize>(objects: [(&str, Value); N]) -> Self {
            let path = std::env::temp_dir().join(format!(
                "light-programming-preset-{}.sqlite",
                Uuid::new_v4()
            ));
            let (store, _) = ShowStore::create(&path, "Preset test").unwrap();
            for (id, body) in objects {
                store.put_object("preset", id, &body, 0).unwrap();
            }
            let document = store.portable_document().unwrap();
            drop(store);
            Self { path, document }
        }
    }

    impl Drop for TestDocument {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{}", self.path.display(), suffix));
            }
        }
    }

    fn color_one() -> PresetAddress {
        PresetAddress::new(PresetFamily::Color, 1).unwrap()
    }

    fn preset_body(name: &str, value: f32) -> Value {
        serde_json::to_value(Preset {
            name: name.into(),
            family: PresetFamily::Color,
            number: 1,
            values: HashMap::from([(
                light_core::FixtureId::new(),
                HashMap::from([(
                    AttributeKey("color.wheel.1".into()),
                    AttributeValue::Normalized(value),
                )]),
            )]),
            group_values: HashMap::new(),
        })
        .unwrap()
    }

    fn recording(document: &TestDocument, value: f32) -> ProgrammingPresetCommit {
        let request = ProgrammingPresetRecordRequest {
            show_id: document.document.id(),
            address: color_one(),
            name: "Recorded".into(),
            mode: PresetStoreMode::Merge,
            expected_object_revision: ProgrammingPresetRevisionExpectation::Current,
            expected_show_revision: None,
        };
        let captured = Preset {
            name: request.name.clone(),
            family: PresetFamily::Color,
            number: 1,
            values: HashMap::from([(
                light_core::FixtureId::new(),
                HashMap::from([(
                    AttributeKey("color.wheel.1".into()),
                    AttributeValue::Normalized(value),
                )]),
            )]),
            group_values: HashMap::new(),
        };
        ProgrammingPresetCommit::new(&request, captured)
    }
}
