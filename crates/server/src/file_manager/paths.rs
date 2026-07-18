use std::{
    collections::HashSet,
    fs,
    path::{Component, Path as FsPath, PathBuf},
    time::UNIX_EPOCH,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

use super::super::file_manager_support as support;
use super::super::{ApiError, AppState};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ConfiguredRoot {
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Default, Deserialize)]
pub(super) struct DirectoryQuery {
    #[serde(default)]
    pub(super) path: String,
    #[serde(default)]
    pub(super) hidden: bool,
}

pub(super) fn configured_roots(state: &AppState) -> Vec<(ConfiguredRoot, bool)> {
    configured_roots_from(
        state.configuration.read().file_manager_roots.clone(),
        state.data_dir.join("shows"),
        support::discover_removable_paths(),
    )
}

pub(super) fn configured_roots_from(
    configured: Vec<ConfiguredRoot>,
    default_shows_path: PathBuf,
    removable_paths: Vec<PathBuf>,
) -> Vec<(ConfiguredRoot, bool)> {
    let mut roots = configured_or_default(configured, default_shows_path);
    let configured_paths = canonical_root_paths(&roots);
    let mut ids = roots
        .iter()
        .map(|(root, _)| root.id.clone())
        .collect::<HashSet<_>>();
    roots.extend(
        removable_roots(removable_paths)
            .into_iter()
            .filter(|root| {
                let canonical = fs::canonicalize(&root.path).unwrap_or_else(|_| root.path.clone());
                !configured_paths.contains(&canonical) && ids.insert(root.id.clone())
            })
            .map(|root| (root, true)),
    );
    roots
}

fn configured_or_default(
    configured: Vec<ConfiguredRoot>,
    default_shows_path: PathBuf,
) -> Vec<(ConfiguredRoot, bool)> {
    if configured.is_empty() {
        vec![(
            ConfiguredRoot {
                id: "shows".into(),
                label: "Shows".into(),
                path: default_shows_path,
                icon: Some("shows".into()),
            },
            false,
        )]
    } else {
        configured.into_iter().map(|root| (root, false)).collect()
    }
}

fn canonical_root_paths(roots: &[(ConfiguredRoot, bool)]) -> HashSet<PathBuf> {
    roots
        .iter()
        .filter_map(|(root, _)| fs::canonicalize(&root.path).ok())
        .collect()
}

fn removable_roots(paths: Vec<PathBuf>) -> Vec<ConfiguredRoot> {
    paths
        .into_iter()
        .map(|path| {
            let label = path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .into_owned();
            ConfiguredRoot {
                id: format!(
                    "removable-{}",
                    URL_SAFE_NO_PAD.encode(path.to_string_lossy().as_bytes())
                ),
                label,
                path,
                icon: Some("drive".into()),
            }
        })
        .collect()
}

pub(super) fn root(state: &AppState, id: &str) -> Result<(ConfiguredRoot, bool), ApiError> {
    configured_roots(state)
        .into_iter()
        .find(|(root, _)| root.id == id)
        .ok_or_else(|| ApiError::not_found("file root"))
}

pub(super) fn confined(
    root: &FsPath,
    relative: &str,
    allow_missing_leaf: bool,
) -> Result<PathBuf, ApiError> {
    let relative_path = FsPath::new(relative);
    validate_relative_path(relative_path)?;
    let canonical_root =
        fs::canonicalize(root).map_err(|_| ApiError::not_found("file root is unavailable"))?;
    let joined = canonical_root.join(relative_path);
    let checked =
        resolve_confined_path(&canonical_root, &joined, relative_path, allow_missing_leaf)?;
    if checked != canonical_root && !checked.starts_with(&canonical_root) {
        return Err(ApiError::bad_request("path escapes the configured root"));
    }
    Ok(checked)
}

fn validate_relative_path(path: &FsPath) -> Result<(), ApiError> {
    let escapes = path.components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if path.is_absolute() || escapes {
        return Err(ApiError::bad_request(
            "path must be root-relative and may not traverse parents",
        ));
    }
    Ok(())
}

fn resolve_confined_path(
    canonical_root: &FsPath,
    joined: &FsPath,
    relative: &FsPath,
    allow_missing_leaf: bool,
) -> Result<PathBuf, ApiError> {
    if relative.as_os_str().is_empty() {
        return Ok(canonical_root.to_path_buf());
    }
    if allow_missing_leaf && !joined.exists() {
        let parent = joined
            .parent()
            .ok_or_else(|| ApiError::bad_request("invalid path"))?;
        return Ok(fs::canonicalize(parent)
            .map_err(|_| ApiError::not_found("parent directory"))?
            .join(
                joined
                    .file_name()
                    .ok_or_else(|| ApiError::bad_request("invalid name"))?,
            ));
    }
    fs::canonicalize(joined).map_err(|_| ApiError::not_found("file"))
}

pub(super) fn relative(root: &FsPath, path: &FsPath) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn io_api_error(error: std::io::Error) -> ApiError {
    match error.kind() {
        std::io::ErrorKind::AlreadyExists => ApiError::conflict(error.to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            ApiError::bad_request(error.to_string())
        }
        std::io::ErrorKind::NotFound => ApiError::not_found(error.to_string()),
        std::io::ErrorKind::PermissionDenied => ApiError::forbidden(error.to_string()),
        std::io::ErrorKind::Unsupported => ApiError::unavailable(error.to_string()),
        _ => ApiError::io(error),
    }
}

pub(super) fn millis(value: std::io::Result<std::time::SystemTime>) -> Option<u128> {
    value
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis())
}

pub(super) fn mime_for(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "txt" | "md" | "csv" | "log" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
