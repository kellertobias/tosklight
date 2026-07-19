//! Deterministic checked-in artifacts derived from the Rust wire DTOs.

mod declarations;

use std::{fs, io, path::Path};

use schemars::{JsonSchema, generate::SchemaSettings};
use ts_rs::Config;

use crate::v2::command_line::{
    CommandErrorResponse, CommandKeyRequest, CommandLineChangedEvent, CommandLineResponse,
    CommandOperationResponse, ExecuteCommandLineRequest, ProgrammingInteractionSnapshot,
    ProgrammingSelectionActionOutcome, ProgrammingSelectionActionRequest,
    ReplaceCommandLineRequest,
};
use crate::v2::events::{EventClientMessage, EventServerMessage, OutputRuntimeSnapshot};
use crate::v2::patch::{
    PatchDelta, PatchErrorResponse, PatchFixtureProjection, PatchFixturesOutcome,
    PatchFixturesRequest, PatchProfileRevisionProjection, PatchSnapshot,
};
use crate::v2::playback::{
    PlaybackActionOutcome, PlaybackActionRequest, PlaybackErrorResponse, PlaybackRuntimeSnapshot,
    PlaybackRuntimeSnapshotRequest,
};
use crate::v2::programming::{
    ProgrammingCaptureModeSnapshot, ProgrammingValuesActionOutcome, ProgrammingValuesActionRequest,
    ProgrammingValuesErrorResponse, ProgrammingValuesSnapshot,
};
use crate::v2::selective_import::{
    SelectiveImportApplyRequest, SelectiveImportCatalog, SelectiveImportErrorResponse,
    SelectiveImportOutcome, SelectiveImportPreview, SelectiveImportSelection,
};

const TYPESCRIPT_PATH: &str = "apps/control-ui/src/api/generated/light-wire.ts";
const SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-command-line";
const EVENT_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-events";
const PATCH_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-patch";
const PLAYBACK_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-playback";
const PROGRAMMING_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-programming";
const SELECTIVE_IMPORT_SCHEMA_DIRECTORY: &str = "crates/wire/schemas/v2-selective-import";

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
        request_schema_artifact::<ProgrammingSelectionActionRequest>(
            "programming-selection-action-request",
        ),
        response_schema_artifact::<CommandLineResponse>("command-line-response"),
        response_schema_artifact::<CommandOperationResponse>("command-operation-response"),
        response_schema_artifact::<CommandErrorResponse>("command-error-response"),
        response_schema_artifact::<CommandLineChangedEvent>("command-line-changed-event"),
        response_schema_artifact::<ProgrammingInteractionSnapshot>(
            "programming-interaction-snapshot",
        ),
        response_schema_artifact::<ProgrammingSelectionActionOutcome>(
            "programming-selection-action-outcome",
        ),
        event_request_schema::<EventClientMessage>("event-client-message"),
        event_response_schema::<EventServerMessage>("event-server-message"),
        event_response_schema::<OutputRuntimeSnapshot>("output-runtime-snapshot"),
        programming_request_schema::<ProgrammingValuesActionRequest>(
            "programming-values-action-request",
        ),
        programming_response_schema::<ProgrammingValuesActionOutcome>(
            "programming-values-action-outcome",
        ),
        programming_response_schema::<ProgrammingValuesErrorResponse>(
            "programming-values-error-response",
        ),
        programming_response_schema::<ProgrammingValuesSnapshot>("programming-values-snapshot"),
        programming_response_schema::<ProgrammingCaptureModeSnapshot>(
            "programming-capture-mode-snapshot",
        ),
        playback_request_schema::<PlaybackActionRequest>("playback-action-request"),
        playback_response_schema::<PlaybackActionOutcome>("playback-action-outcome"),
        playback_response_schema::<PlaybackErrorResponse>("playback-error-response"),
        playback_request_schema::<PlaybackRuntimeSnapshotRequest>(
            "playback-runtime-snapshot-request",
        ),
        playback_response_schema::<PlaybackRuntimeSnapshot>("playback-runtime-snapshot"),
        patch_request_schema::<PatchFixturesRequest>("patch-fixtures-request"),
        patch_response_schema::<PatchFixturesOutcome>("patch-fixtures-outcome"),
        patch_response_schema::<PatchErrorResponse>("patch-error-response"),
        patch_response_schema::<PatchSnapshot>("patch-snapshot"),
        patch_response_schema::<PatchDelta>("patch-delta"),
        patch_response_schema::<PatchFixtureProjection>("patch-fixture-projection"),
        patch_response_schema::<PatchProfileRevisionProjection>(
            "patch-profile-revision-projection",
        ),
        selective_import_request_schema::<SelectiveImportSelection>("preview-request"),
        selective_import_request_schema::<SelectiveImportApplyRequest>("apply-request"),
        selective_import_response_schema::<SelectiveImportCatalog>("catalog"),
        selective_import_response_schema::<SelectiveImportPreview>("preview"),
        selective_import_response_schema::<SelectiveImportOutcome>("outcome"),
        selective_import_response_schema::<SelectiveImportErrorResponse>("error-response"),
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

fn playback_request_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    playback_schema::<T>(name, SchemaSettings::draft2020_12().for_deserialize())
}

fn playback_response_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    playback_schema::<T>(name, SchemaSettings::draft2020_12().for_serialize())
}

fn programming_request_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    programming_schema::<T>(name, SchemaSettings::draft2020_12().for_deserialize())
}

fn programming_response_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    programming_schema::<T>(name, SchemaSettings::draft2020_12().for_serialize())
}

fn selective_import_request_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    selective_import_schema::<T>(name, SchemaSettings::draft2020_12().for_deserialize())
}

fn selective_import_response_schema<T: JsonSchema>(name: &str) -> GeneratedArtifact {
    selective_import_schema::<T>(name, SchemaSettings::draft2020_12().for_serialize())
}

fn event_schema<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    namespaced_schema::<T>(EVENT_SCHEMA_DIRECTORY, name, settings)
}

fn patch_schema<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    namespaced_schema::<T>(PATCH_SCHEMA_DIRECTORY, name, settings)
}

fn playback_schema<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    namespaced_schema::<T>(PLAYBACK_SCHEMA_DIRECTORY, name, settings)
}

fn programming_schema<T: JsonSchema>(name: &str, settings: SchemaSettings) -> GeneratedArtifact {
    namespaced_schema::<T>(PROGRAMMING_SCHEMA_DIRECTORY, name, settings)
}

fn selective_import_schema<T: JsonSchema>(
    name: &str,
    settings: SchemaSettings,
) -> GeneratedArtifact {
    namespaced_schema::<T>(SELECTIVE_IMPORT_SCHEMA_DIRECTORY, name, settings)
}

fn namespaced_schema<T: JsonSchema>(
    directory: &str,
    name: &str,
    settings: SchemaSettings,
) -> GeneratedArtifact {
    let mut artifact = schema_artifact::<T>(name, settings);
    artifact.path = format!("{directory}/{name}.schema.json");
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
    let declarations = declarations::all(&Config::default())
        .into_iter()
        .map(export_declaration)
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "// This file is generated by `cargo run -p light-wire --example generate-contracts`.\n\
         // Do not edit it by hand.\n\n{declarations}\n"
    )
}

fn export_declaration(declaration: String) -> String {
    let declaration = declaration
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    format!("export {declaration}")
}
