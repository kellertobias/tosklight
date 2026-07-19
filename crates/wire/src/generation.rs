//! Deterministic checked-in artifacts derived from the Rust wire DTOs.

use std::{fs, io, path::Path};

use schemars::{JsonSchema, generate::SchemaSettings};
use ts_rs::{Config, TS};

use crate::v2::command_line::{
    CommandAcceptedAction, CommandChoiceOption, CommandChoiceOptionId, CommandErrorResponse,
    CommandHttpSource, CommandKey, CommandKeyPhase, CommandKeyRequest, CommandLineChangedEvent,
    CommandLineResponse, CommandOperationOutcome, CommandOperationResponse, CommandTarget,
    CueMoveCopyChoice, CueMoveCopyChoiceType, CueTransferOperation, ExecuteCommandLineRequest,
    ReplaceCommandLineRequest,
};
use crate::v2::events::{
    CueReference, EventActionSource, EventCapability, EventClass, EventClientMessage,
    EventDeliveryPolicy, EventEnvelope, EventObject, EventPayload, EventRateLimit,
    EventServerMessage, EventSnapshotCursor, EventSource, EventSubscriptionFilter,
    OutputDeliveryMode, OutputProtocol, OutputRoute, OutputRouteChange, PlaybackCueTransition,
    PlaybackEventSnapshot, PlaybackStateSnapshot, PlaybackTransitionCause, SequenceGap,
    ShowObjectChange, ShowObjectKind, ShowObjectsChange,
};
use crate::v2::patch::{
    PatchDelta, PatchDirectControlEndpoint, PatchDirectControlProtocol, PatchErrorResponse,
    PatchFixtureInput, PatchFixtureLocation, PatchFixtureProjection, PatchFixtureRotation,
    PatchFixturesOutcome, PatchFixturesRequest, PatchHighlightOverrideInput,
    PatchHighlightOverrideProjection, PatchLogicalHeadProjection, PatchModeProjection,
    PatchModeSplitProjection, PatchMultiPatchInput, PatchMultiPatchProjection, PatchProfilePolicy,
    PatchProfileRevisionProjection, PatchSnapshot, PatchSplitAssignment,
};

const TYPESCRIPT_PATH: &str = "apps/control-ui/src/api/generated/light-wire.ts";
const SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-command-line";
const EVENT_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-events";
const PATCH_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-patch";

/// One generated artifact relative to the workspace root.
#[derive(Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
    pub path: String,
    pub contents: String,
}

/// Render every checked-in artifact without touching the filesystem.
pub fn generated_artifacts() -> Vec<GeneratedArtifact> {
    vec![
        GeneratedArtifact {
            path: TYPESCRIPT_PATH.into(),
            contents: typescript_bindings(),
        },
        request_schema_artifact::<ReplaceCommandLineRequest>("replace-command-line-request"),
        request_schema_artifact::<CommandKeyRequest>("command-key-request"),
        request_schema_artifact::<ExecuteCommandLineRequest>("execute-command-line-request"),
        response_schema_artifact::<CommandLineResponse>("command-line-response"),
        response_schema_artifact::<CommandOperationResponse>("command-operation-response"),
        response_schema_artifact::<CommandErrorResponse>("command-error-response"),
        response_schema_artifact::<CommandLineChangedEvent>("command-line-changed-event"),
        event_request_schema::<EventClientMessage>("event-client-message"),
        event_response_schema::<EventServerMessage>("event-server-message"),
        event_response_schema::<PlaybackEventSnapshot>("playback-event-snapshot"),
        patch_request_schema::<PatchFixturesRequest>("patch-fixtures-request"),
        patch_response_schema::<PatchFixturesOutcome>("patch-fixtures-outcome"),
        patch_response_schema::<PatchErrorResponse>("patch-error-response"),
        patch_response_schema::<PatchSnapshot>("patch-snapshot"),
        patch_response_schema::<PatchDelta>("patch-delta"),
        patch_response_schema::<PatchFixtureProjection>("patch-fixture-projection"),
        patch_response_schema::<PatchProfileRevisionProjection>(
            "patch-profile-revision-projection",
        ),
    ]
}

/// Rewrite all generated artifacts below `workspace_root`.
pub fn write_generated_artifacts(workspace_root: &Path) -> io::Result<()> {
    for artifact in generated_artifacts() {
        let destination = workspace_root.join(&artifact.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(destination, artifact.contents)?;
    }
    Ok(())
}

fn request_schema_artifact<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    schema_artifact::<T>(name, SchemaSettings::draft2020_12().for_deserialize())
}

fn response_schema_artifact<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    schema_artifact::<T>(name, SchemaSettings::draft2020_12().for_serialize())
}

fn event_request_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    event_schema::<T>(name, SchemaSettings::draft2020_12().for_deserialize())
}

fn event_response_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    event_schema::<T>(name, SchemaSettings::draft2020_12().for_serialize())
}

fn patch_request_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    patch_schema::<T>(name, SchemaSettings::draft2020_12().for_deserialize())
}

fn patch_response_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    patch_schema::<T>(name, SchemaSettings::draft2020_12().for_serialize())
}

fn event_schema<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    let mut artifact = schema_artifact::<T>(name, settings);
    artifact.path = format!("{EVENT_SCHEMA_DIRECTORY}/{name}.schema.json");
    artifact
}

fn patch_schema<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    let mut artifact = schema_artifact::<T>(name, settings);
    artifact.path = format!("{PATCH_SCHEMA_DIRECTORY}/{name}.schema.json");
    artifact
}

fn schema_artifact<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    let schema = settings.into_generator().into_root_schema_for::<T>();
    let mut contents =
        serde_json::to_string_pretty(&schema).expect("JSON Schema always serializes as JSON");
    contents.push('\n');
    GeneratedArtifact {
        path: format!("{SCHEMA_DIRECTORY}/{name}.schema.json"),
        contents,
    }
}

fn typescript_bindings() -> String {
    let config = Config::default();
    let declarations = [
        CommandTarget::decl(&config),
        CommandKey::decl(&config),
        CommandKeyPhase::decl(&config),
        CommandAcceptedAction::decl(&config),
        CommandChoiceOptionId::decl(&config),
        CueTransferOperation::decl(&config),
        CueMoveCopyChoiceType::decl(&config),
        CommandHttpSource::decl(&config),
        CommandChoiceOption::decl(&config),
        CueMoveCopyChoice::decl(&config),
        ReplaceCommandLineRequest::decl(&config),
        CommandKeyRequest::decl(&config),
        ExecuteCommandLineRequest::decl(&config),
        CommandLineResponse::decl(&config),
        CommandOperationOutcome::decl(&config),
        CommandOperationResponse::decl(&config),
        CommandErrorResponse::decl(&config),
        CommandLineChangedEvent::decl(&config),
        EventCapability::decl(&config),
        EventClass::decl(&config),
        EventDeliveryPolicy::decl(&config),
        EventActionSource::decl(&config),
        PlaybackTransitionCause::decl(&config),
        EventObject::decl(&config),
        EventSubscriptionFilter::decl(&config),
        EventRateLimit::decl(&config),
        EventSnapshotCursor::decl(&config),
        SequenceGap::decl(&config),
        EventSource::decl(&config),
        CueReference::decl(&config),
        PlaybackCueTransition::decl(&config),
        OutputProtocol::decl(&config),
        OutputDeliveryMode::decl(&config),
        OutputRoute::decl(&config),
        OutputRouteChange::decl(&config),
        ShowObjectKind::decl(&config),
        ShowObjectChange::decl(&config),
        ShowObjectsChange::decl(&config),
        EventPayload::decl(&config),
        EventEnvelope::decl(&config),
        EventClientMessage::decl(&config),
        EventServerMessage::decl(&config),
        PlaybackStateSnapshot::decl(&config),
        PlaybackEventSnapshot::decl(&config),
        PatchDirectControlProtocol::decl(&config),
        PatchProfilePolicy::decl(&config),
        PatchSplitAssignment::decl(&config),
        PatchDirectControlEndpoint::decl(&config),
        PatchFixtureLocation::decl(&config),
        PatchFixtureRotation::decl(&config),
        PatchMultiPatchInput::decl(&config),
        PatchHighlightOverrideInput::decl(&config),
        PatchFixtureInput::decl(&config),
        PatchFixturesRequest::decl(&config),
        PatchErrorResponse::decl(&config),
        PatchLogicalHeadProjection::decl(&config),
        PatchMultiPatchProjection::decl(&config),
        PatchHighlightOverrideProjection::decl(&config),
        PatchFixtureProjection::decl(&config),
        PatchModeSplitProjection::decl(&config),
        PatchModeProjection::decl(&config),
        PatchProfileRevisionProjection::decl(&config),
        PatchDelta::decl(&config),
        PatchFixturesOutcome::decl(&config),
        PatchSnapshot::decl(&config),
    ];
    let declarations = declarations
        .into_iter()
        .map(|declaration| {
            let declaration = declaration
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            format!("export {declaration}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "// This file is generated by `cargo run -p light-wire --example generate-contracts`.\n\
         // Do not edit it by hand.\n\n{declarations}\n"
    )
}
