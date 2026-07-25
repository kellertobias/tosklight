use std::fs;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::super::file_manager_support as support;
use super::super::{ApiError, AppState, authenticate};
use super::paths::{confined, io_api_error, millis, root};

#[derive(Default, Deserialize)]
pub(super) struct ThumbnailQuery {
    path: String,
    #[serde(default = "default_thumbnail_size")]
    max_size: u32,
}

fn default_thumbnail_size() -> u32 {
    256
}

pub(super) async fn thumbnail(
    State(state): State<AppState>,
    Path(root_id): Path<String>,
    Query(query): Query<ThumbnailQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let (root, _) = root(&state, &root_id)?;
    let path = confined(&root.path, &query.path, false)?;
    let metadata = fs::metadata(&path).map_err(ApiError::io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let size = query.max_size.clamp(32, 1_024);
    let key = thumbnail_cache_key(&root_id, &query.path, &metadata, size);
    let cache = state.data_dir.join("cache/file-thumbnails");
    fs::create_dir_all(&cache).map_err(ApiError::io)?;
    let cached = cache.join(format!("{key}.png"));
    let bytes = if cached.is_file() {
        fs::read(&cached).map_err(ApiError::io)?
    } else {
        generate_cached_thumbnail(&path, &cache, &cached, &key, size).await?
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::ETAG, format!("\"{key}\""))
        .header(header::CONTENT_LENGTH, bytes.len().to_string())
        .body(Body::from(bytes))
        .map_err(|_| ApiError::internal("could not return thumbnail"))
}

async fn generate_cached_thumbnail(
    path: &std::path::Path,
    cache: &std::path::Path,
    cached: &std::path::Path,
    key: &str,
    size: u32,
) -> Result<Vec<u8>, ApiError> {
    let source = path.to_path_buf();
    let bytes = tokio::task::spawn_blocking(move || support::thumbnail_png(&source, size))
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .map_err(io_api_error)?;
    let temporary = cache.join(format!(".{key}.{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, &bytes).map_err(ApiError::io)?;
    match fs::rename(&temporary, cached) {
        Ok(()) => {}
        Err(_) if cached.exists() => {
            let _ = fs::remove_file(&temporary);
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(ApiError::io(error));
        }
    }
    Ok(bytes)
}

fn thumbnail_cache_key(root_id: &str, path: &str, metadata: &fs::Metadata, size: u32) -> String {
    let mut digest = Sha256::new();
    digest.update(root_id.as_bytes());
    digest.update([0]);
    digest.update(path.as_bytes());
    digest.update(metadata.len().to_le_bytes());
    digest.update(
        millis(metadata.modified())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    digest.update(size.to_le_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
