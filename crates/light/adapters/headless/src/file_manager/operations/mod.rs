mod execute;

use std::{collections::HashSet, fs, path::PathBuf};

use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use light_wire::v2::files::{
    FileConflictChoice as RequestedConflict, FileOperationKind,
    FileOperationRequest as FileOperation,
};
use serde::Serialize;

use self::execute::{create_item, remove_items, rename_item, transfer_items};
use super::super::file_manager_support::ConflictChoice;
use super::super::{
    ApiError, AppState, authenticate, desk_management_v2::ReplayKey, emit,
    show_objects_v2::validate_request_id,
};
use super::FILE_MUTATION_LOCK;
use super::paths::{ConfiguredRoot, root};
use crate::tolerant_json::TolerantJson;

#[derive(Serialize)]
pub(super) struct FileOperationResult {
    paths: Vec<String>,
    complete: bool,
    items: Vec<FileOperationItem>,
}

#[derive(Serialize)]
pub(super) struct FileOperationItem {
    source_root_id: String,
    source: String,
    destination_root_id: Option<String>,
    destination: Option<String>,
    status: &'static str,
    error: Option<String>,
}

pub(super) struct OperationContext {
    root_id: String,
    source_root: ConfiguredRoot,
    canonical_source_root: PathBuf,
    destination_root_id: String,
    destination_root: ConfiguredRoot,
    canonical_destination_root: PathBuf,
    conflict: ConflictChoice,
}

#[derive(Default)]
pub(super) struct OperationOutput {
    paths: Vec<String>,
    items: Vec<FileOperationItem>,
}

pub(super) async fn operate(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<FileOperation>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&input.request_id)?;
    let key = ReplayKey::new(
        session.id,
        &format!("file-operation:{root_id}"),
        &input.request_id,
    );
    let fingerprint =
        serde_json::to_value(&input).map_err(|error| ApiError::internal(error.to_string()))?;
    if let Some(value) = state
        .replay
        .lookup_desk_management(&key, &fingerprint)
        .await?
    {
        return Ok(Json(value));
    }
    let _mutation_guard = FILE_MUTATION_LOCK.lock().await;
    validate_sources(&input.sources)?;
    let _apply_to_all = input.apply_to_all;
    let context = OperationContext::new(&state, root_id, &input)?;
    let output = execute_operation(&context, &input)?;
    emit_completion(&state, input.operation, &output.items);
    let value = super::intent_value(
        FileOperationResult {
            complete: output.items.iter().all(|item| item.status != "failed"),
            paths: output.paths,
            items: output.items,
        },
        &input.request_id,
        false,
    )?;
    state
        .replay
        .insert_desk_management(key, fingerprint, value.clone())
        .await;
    Ok(Json(value))
}

fn validate_sources(sources: &[String]) -> Result<(), ApiError> {
    if sources.len() > 1_000 {
        return Err(ApiError::bad_request(
            "a file operation is limited to 1000 sources",
        ));
    }
    if sources.iter().collect::<HashSet<_>>().len() != sources.len() {
        return Err(ApiError::bad_request(
            "a file operation may not contain duplicate sources",
        ));
    }
    Ok(())
}

impl OperationContext {
    fn new(state: &AppState, root_id: String, input: &FileOperation) -> Result<Self, ApiError> {
        let (source_root, _) = root(state, &root_id)?;
        let canonical_source_root = fs::canonicalize(&source_root.path)
            .map_err(|_| ApiError::not_found("file root is unavailable"))?;
        let destination_root_id = input
            .destination_root_id
            .clone()
            .unwrap_or_else(|| root_id.clone());
        let (destination_root, _) = root(state, &destination_root_id)?;
        let canonical_destination_root = fs::canonicalize(&destination_root.path)
            .map_err(|_| ApiError::not_found("destination file root is unavailable"))?;
        Ok(Self {
            root_id,
            source_root,
            canonical_source_root,
            destination_root_id,
            destination_root,
            canonical_destination_root,
            conflict: requested_conflict(input),
        })
    }
}

fn requested_conflict(input: &FileOperation) -> ConflictChoice {
    input
        .conflict
        .map(|value| match value {
            RequestedConflict::Replace => ConflictChoice::Replace,
            RequestedConflict::KeepBoth => ConflictChoice::KeepBoth,
            RequestedConflict::Skip => ConflictChoice::Skip,
        })
        .unwrap_or(if input.replace {
            ConflictChoice::Replace
        } else {
            ConflictChoice::Error
        })
}

fn execute_operation(
    context: &OperationContext,
    input: &FileOperation,
) -> Result<OperationOutput, ApiError> {
    match input.operation {
        FileOperationKind::CreateFile | FileOperationKind::CreateFolder => {
            create_item(context, input)
        }
        FileOperationKind::Rename => rename_item(context, input),
        FileOperationKind::Delete | FileOperationKind::Trash => remove_items(context, input),
        FileOperationKind::Copy | FileOperationKind::Move => transfer_items(context, input),
    }
}

fn emit_completion(state: &AppState, operation: FileOperationKind, items: &[FileOperationItem]) {
    emit(
        state,
        "file_operation_completed",
        serde_json::json!({
            "operation": operation,
            "items": items,
        }),
    );
}

pub(super) fn operation_item(
    source_root_id: &str,
    source: &str,
    destination_root_id: Option<&str>,
    destination: Option<String>,
    status: &'static str,
    error: Option<String>,
) -> FileOperationItem {
    FileOperationItem {
        source_root_id: source_root_id.into(),
        source: source.into(),
        destination_root_id: destination_root_id.map(str::to_owned),
        destination,
        status,
        error,
    }
}

#[cfg(test)]
pub(super) fn safe_name(value: Option<&str>) -> Result<&str, ApiError> {
    execute::safe_name(value)
}
