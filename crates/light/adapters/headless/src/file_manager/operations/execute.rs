use std::{fs, path::PathBuf};

use super::super::super::ApiError;
use super::super::super::file_manager_support::{self as support, ConflictChoice, TransferOutcome};
use super::super::paths::{confined, io_api_error, relative};
use super::{FileOperation, FileOperationKind, OperationContext, OperationOutput, operation_item};

pub(super) fn create_item(
    context: &OperationContext,
    input: &FileOperation,
) -> Result<OperationOutput, ApiError> {
    require_same_root(
        context,
        "create operations use the root in the request path",
    )?;
    let directory = input
        .destination
        .as_deref()
        .map(|value| confined(&context.source_root.path, value, false))
        .transpose()?
        .unwrap_or_else(|| context.canonical_source_root.clone());
    if !directory.is_dir() {
        return Err(ApiError::bad_request("destination is not a folder"));
    }
    let name = safe_name(input.name.as_deref())?;
    let path = confined(
        &context.source_root.path,
        &relative(&context.canonical_source_root, &directory.join(name)),
        true,
    )?;
    if path.exists() {
        return Err(ApiError::conflict("an item with that name already exists"));
    }
    if matches!(input.operation, FileOperationKind::CreateFolder) {
        fs::create_dir(&path).map_err(ApiError::io)?;
    } else {
        fs::write(&path, []).map_err(ApiError::io)?;
    }
    let created = relative(&context.canonical_source_root, &path);
    Ok(OperationOutput {
        paths: vec![created.clone()],
        items: vec![operation_item(
            &context.root_id,
            "",
            Some(&context.root_id),
            Some(created),
            "completed",
            None,
        )],
    })
}

pub(super) fn rename_item(
    context: &OperationContext,
    input: &FileOperation,
) -> Result<OperationOutput, ApiError> {
    require_same_root(context, "rename cannot change file roots; use move")?;
    let source = one_source(&context.source_root.path, &input.sources)?;
    reject_root_source(&context.canonical_source_root, &source)?;
    let name = safe_name(input.name.as_deref())?;
    let parent = source.parent().unwrap_or(&context.canonical_source_root);
    let target = confined(
        &context.source_root.path,
        &relative(&context.canonical_source_root, &parent.join(name)),
        true,
    )?;
    let outcome = support::copy_or_move(&source, &target, true, false, context.conflict)
        .map_err(io_api_error)?;
    let (target, status) = transfer_status(outcome);
    let target = relative(&context.canonical_source_root, &target);
    let paths = (status == "completed")
        .then(|| target.clone())
        .into_iter()
        .collect();
    Ok(OperationOutput {
        paths,
        items: vec![operation_item(
            &context.root_id,
            &input.sources[0],
            Some(&context.root_id),
            Some(target),
            status,
            None,
        )],
    })
}

fn transfer_status(outcome: TransferOutcome) -> (PathBuf, &'static str) {
    match outcome {
        TransferOutcome::Completed(path) => (path, "completed"),
        TransferOutcome::Skipped(path) => (path, "skipped"),
    }
}

pub(super) fn remove_items(
    context: &OperationContext,
    input: &FileOperation,
) -> Result<OperationOutput, ApiError> {
    require_same_root(
        context,
        "delete operations use the root in the request path",
    )?;
    require_sources(input)?;
    let resolved = resolve_sources(context, &input.sources)?;
    let items = resolved
        .into_iter()
        .map(|(source, path)| {
            let result = if matches!(input.operation, FileOperationKind::Trash) {
                support::trash_path(&path)
            } else {
                support::remove_permanent(&path)
            };
            match result {
                Ok(()) => operation_item(&context.root_id, &source, None, None, "completed", None),
                Err(error) => operation_item(
                    &context.root_id,
                    &source,
                    None,
                    None,
                    "failed",
                    Some(error.to_string()),
                ),
            }
        })
        .collect();
    Ok(OperationOutput {
        paths: Vec::new(),
        items,
    })
}

fn resolve_sources(
    context: &OperationContext,
    sources: &[String],
) -> Result<Vec<(String, PathBuf)>, ApiError> {
    sources
        .iter()
        .map(|source| {
            let path = confined(&context.source_root.path, source, false)?;
            reject_root_source(&context.canonical_source_root, &path)?;
            Ok((source.clone(), path))
        })
        .collect()
}

pub(super) fn transfer_items(
    context: &OperationContext,
    input: &FileOperation,
) -> Result<OperationOutput, ApiError> {
    require_sources(input)?;
    let destination = destination_directory(context, input)?;
    let resolved = resolve_transfers(context, &destination, &input.sources)?;
    reject_undecided_conflicts(context.conflict, &resolved)?;
    let move_source = matches!(input.operation, FileOperationKind::Move);
    let cross_root = context.root_id != context.destination_root_id;
    let mut output = OperationOutput::default();
    for (source_relative, source, requested_target) in resolved {
        append_transfer(
            &mut output,
            context,
            source_relative,
            source,
            requested_target,
            move_source,
            cross_root,
        );
    }
    Ok(output)
}

fn destination_directory(
    context: &OperationContext,
    input: &FileOperation,
) -> Result<PathBuf, ApiError> {
    let destination = input
        .destination
        .as_deref()
        .map(|value| confined(&context.destination_root.path, value, false))
        .transpose()?
        .unwrap_or_else(|| context.canonical_destination_root.clone());
    if !destination.is_dir() {
        return Err(ApiError::bad_request("destination is not a folder"));
    }
    Ok(destination)
}

type ResolvedTransfer = (String, PathBuf, PathBuf);

fn resolve_transfers(
    context: &OperationContext,
    destination: &std::path::Path,
    sources: &[String],
) -> Result<Vec<ResolvedTransfer>, ApiError> {
    sources
        .iter()
        .map(|source_relative| {
            let source = confined(&context.source_root.path, source_relative, false)?;
            reject_root_source(&context.canonical_source_root, &source)?;
            let name = source
                .file_name()
                .ok_or_else(|| ApiError::bad_request("invalid source"))?;
            let requested_target = confined(
                &context.destination_root.path,
                &relative(&context.canonical_destination_root, &destination.join(name)),
                true,
            )?;
            Ok((source_relative.clone(), source, requested_target))
        })
        .collect()
}

fn reject_undecided_conflicts(
    conflict: ConflictChoice,
    transfers: &[ResolvedTransfer],
) -> Result<(), ApiError> {
    if conflict == ConflictChoice::Error && transfers.iter().any(|(_, _, target)| target.exists()) {
        return Err(ApiError::conflict(
            "one or more destination names already exist",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_transfer(
    output: &mut OperationOutput,
    context: &OperationContext,
    source_relative: String,
    source: PathBuf,
    requested_target: PathBuf,
    move_source: bool,
    cross_root: bool,
) {
    match support::copy_or_move(
        &source,
        &requested_target,
        move_source,
        cross_root,
        context.conflict,
    ) {
        Ok(TransferOutcome::Completed(target)) => {
            let target = relative(&context.canonical_destination_root, &target);
            output.paths.push(target.clone());
            output.items.push(operation_item(
                &context.root_id,
                &source_relative,
                Some(&context.destination_root_id),
                Some(target),
                "completed",
                None,
            ));
        }
        Ok(TransferOutcome::Skipped(target)) => {
            output.items.push(operation_item(
                &context.root_id,
                &source_relative,
                Some(&context.destination_root_id),
                Some(relative(&context.canonical_destination_root, &target)),
                "skipped",
                None,
            ));
        }
        Err(error) => output.items.push(operation_item(
            &context.root_id,
            &source_relative,
            Some(&context.destination_root_id),
            Some(relative(
                &context.canonical_destination_root,
                &requested_target,
            )),
            "failed",
            Some(error.to_string()),
        )),
    }
}

fn require_same_root(context: &OperationContext, message: &str) -> Result<(), ApiError> {
    if context.destination_root_id != context.root_id {
        return Err(ApiError::bad_request(message));
    }
    Ok(())
}

fn require_sources(input: &FileOperation) -> Result<(), ApiError> {
    if input.sources.is_empty() {
        return Err(ApiError::bad_request("at least one source is required"));
    }
    Ok(())
}

pub(super) fn safe_name(value: Option<&str>) -> Result<&str, ApiError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    if invalid_name(value) {
        return Err(ApiError::bad_request(
            "name may not be a dot path or contain path separators or control characters",
        ));
    }
    if value.len() > 255 || value.ends_with(['.', ' ']) || reserved_name(value) {
        return Err(ApiError::bad_request(
            "name is not portable across supported filesystems",
        ));
    }
    Ok(value)
}

fn invalid_name(value: &str) -> bool {
    value == "."
        || value == ".."
        || value.contains(['/', '\\', '\0'])
        || value.chars().any(char::is_control)
}

fn reserved_name(value: &str) -> bool {
    let upper = value
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn one_source(root: &std::path::Path, sources: &[String]) -> Result<PathBuf, ApiError> {
    if sources.len() != 1 {
        return Err(ApiError::bad_request("exactly one source is required"));
    }
    confined(root, &sources[0], false)
}

fn reject_root_source(root: &std::path::Path, source: &std::path::Path) -> Result<(), ApiError> {
    if root == source {
        return Err(ApiError::forbidden(
            "the configured root itself cannot be changed",
        ));
    }
    Ok(())
}
