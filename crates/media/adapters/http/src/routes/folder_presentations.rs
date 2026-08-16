use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{
    FolderPresentationView, FolderPresentationsView, RemoveFolderPicture, UpdateFolderPresentation,
};

pub(super) const MAX_FOLDER_PICTURE_BYTES: usize = 16 * 1024 * 1024;

pub(super) async fn list(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let presentations = (state.diagnostics.library.folder_presentations)()
        .map_err(|detail| unavailable("folder-presentations-unavailable", detail))?;
    Ok(axum::Json(FolderPresentationsView {
        folders: presentations
            .iter()
            .map(FolderPresentationView::of)
            .collect(),
    })
    .into_response())
}

pub(super) async fn get(
    State(state): State<ApiState>,
    Path(folder): Path<u16>,
) -> Result<Response, ApiError> {
    validate_folder(folder)?;
    let presentation = (state.diagnostics.library.folder_presentations)()
        .map_err(|detail| unavailable("folder-presentation-unavailable", detail))?
        .into_iter()
        .find(|candidate| candidate.folder == folder)
        .ok_or_else(|| {
            ApiError::not_found(
                "folder-presentation-not-found",
                "the folder has no presentation",
            )
        })?;
    Ok(axum::Json(FolderPresentationView::of(&presentation)).into_response())
}

pub(super) async fn update(
    State(state): State<ApiState>,
    Path(folder): Path<u16>,
    TolerantJson(body): TolerantJson<UpdateFolderPresentation>,
) -> Result<Response, ApiError> {
    validate_folder(folder)?;
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }
    let (name, icon) = match (body.name, body.icon) {
        (Some(name), None) => (Some(normalize(name)), None),
        (None, Some(icon)) => (None, Some(normalize(icon))),
        _ => {
            return Err(ApiError::bad_request(
                "ambiguous-folder-presentation-edit",
                "set exactly one of name or icon; use an empty string to clear it",
            ));
        }
    };
    let presentation = (state.diagnostics.library.update_folder_presentation)(folder, name, icon)
        .map_err(|detail| unavailable("folder-presentation-not-updated", detail))?;
    remember(&state, &body.request_id, &presentation)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UploadQuery {
    request_id: String,
}

pub(super) async fn upload_picture(
    State(state): State<ApiState>,
    Path(folder): Path<u16>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    validate_folder(folder)?;
    if let Proceed::Replay(response) = edit::begin(&state, &query.request_id)? {
        return Ok(response);
    }
    let mut upload = None;
    while let Some(mut field) = multipart.next_field().await.map_err(|_| {
        ApiError::bad_request(
            "invalid-folder-picture",
            "the multipart upload could not be read",
        )
    })? {
        if field.name() != Some("file") {
            continue;
        }
        if upload.is_some() {
            return Err(ApiError::bad_request(
                "multiple-folder-pictures",
                "upload exactly one folder picture",
            ));
        }
        let content_type = field.content_type().map(str::to_owned).ok_or_else(|| {
            ApiError::bad_request(
                "missing-folder-picture-type",
                "the folder picture needs an image MIME type",
            )
        })?;
        if !content_type.starts_with("image/") {
            return Err(ApiError::bad_request(
                "invalid-folder-picture-type",
                "folder pictures must use an image MIME type",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|_| {
            ApiError::bad_request(
                "invalid-folder-picture",
                "the uploaded picture could not be read",
            )
        })? {
            if bytes.len().saturating_add(chunk.len()) > MAX_FOLDER_PICTURE_BYTES {
                return Err(ApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "folder-picture-too-large",
                    "folder pictures may be at most 16 MiB",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(ApiError::bad_request(
                "empty-folder-picture",
                "the uploaded picture is empty",
            ));
        }
        upload = Some((content_type, bytes));
    }
    let (content_type, bytes) = upload.ok_or_else(|| {
        ApiError::bad_request(
            "missing-folder-picture",
            "the multipart body has no file field",
        )
    })?;
    let presentation =
        (state.diagnostics.library.set_folder_picture)(folder, &content_type, &bytes)
            .map_err(|detail| unavailable("folder-picture-not-stored", detail))?;
    remember(&state, &query.request_id, &presentation)
}

pub(super) async fn remove_picture(
    State(state): State<ApiState>,
    Path(folder): Path<u16>,
    TolerantJson(body): TolerantJson<RemoveFolderPicture>,
) -> Result<Response, ApiError> {
    validate_folder(folder)?;
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }
    let presentation = (state.diagnostics.library.remove_folder_picture)(folder)
        .map_err(|detail| unavailable("folder-picture-not-removed", detail))?;
    remember(&state, &body.request_id, &presentation)
}

pub(super) async fn picture(
    State(state): State<ApiState>,
    Path(folder): Path<u16>,
) -> Result<Response, ApiError> {
    validate_folder(folder)?;
    let (content_type, bytes) =
        (state.diagnostics.library.folder_picture)(folder).map_err(|_| {
            ApiError::not_found("folder-picture-not-found", "this folder has no picture")
        })?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

fn validate_folder(folder: u16) -> Result<(), ApiError> {
    if (1..=255).contains(&folder) {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "invalid-folder",
            "folder presentation addresses run from 1 to 255",
        ))
    }
}

fn normalize(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn unavailable(code: &'static str, detail: String) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, code, detail)
}

fn remember(
    state: &ApiState,
    request_id: &str,
    presentation: &crate::diagnostics::FolderPresentation,
) -> Result<Response, ApiError> {
    let serialized =
        serde_json::to_string(&FolderPresentationView::of(presentation)).unwrap_or_default();
    state.replays.remember(request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use crate::diagnostics::{Diagnostics, FolderPresentation, LibraryAccess};
    use crate::routes::bench::{bench_with, get, post, send};

    #[tokio::test]
    async fn shared_presentations_cover_generated_folders_and_empty_strings_clear_fields() {
        let edits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&edits);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                folder_presentations: Arc::new(|| {
                    Ok(vec![FolderPresentation {
                        folder: 250,
                        name: Some("Bars".to_owned()),
                        icon: Some("waveform".to_owned()),
                        picture_content_type: Some("image/png".to_owned()),
                    }])
                }),
                update_folder_presentation: Arc::new(move |folder, name, icon| {
                    recorded.lock().unwrap().push((folder, name.clone(), icon));
                    Ok(FolderPresentation {
                        folder,
                        name: name.flatten(),
                        icon: Some("waveform".to_owned()),
                        picture_content_type: None,
                    })
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);

        let (status, list) = send(
            &bench.router,
            get("/api/v2/folder-presentations".to_owned()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["folders"][0]["folder"], 250);
        assert_eq!(
            list["folders"][0]["pictureUrl"],
            "/api/v2/folder-presentations/250/picture"
        );

        let body = r#"{"requestId":"clear-name","name":""}"#;
        let (status, updated) = send(
            &bench.router,
            post("/api/v2/folder-presentations/250/update".to_owned(), body),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["name"], serde_json::Value::Null);
        assert_eq!(edits.lock().unwrap().as_slice(), &[(250, Some(None), None)]);

        let _ = send(
            &bench.router,
            post("/api/v2/folder-presentations/250/update".to_owned(), body),
        )
        .await;
        assert_eq!(
            edits.lock().unwrap().len(),
            1,
            "request replay must not edit twice"
        );
    }

    #[tokio::test]
    async fn picture_upload_is_bounded_to_images_and_the_exact_bytes_reach_storage() {
        let stored = Arc::new(Mutex::new(None));
        let recorded = Arc::clone(&stored);
        let diagnostics = Diagnostics {
            library: LibraryAccess {
                set_folder_picture: Arc::new(move |folder, content_type, bytes| {
                    *recorded.lock().unwrap() =
                        Some((folder, content_type.to_owned(), bytes.to_vec()));
                    Ok(FolderPresentation {
                        folder,
                        name: None,
                        icon: None,
                        picture_content_type: Some(content_type.to_owned()),
                    })
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let boundary = "tosklight-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"folder.png\"\r\nContent-Type: image/png\r\n\r\nPNG-BYTES\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .method("POST")
            .uri("/api/v2/folder-presentations/200/picture/upload?requestId=picture-1")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .unwrap();
        let response = bench.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            stored.lock().unwrap().as_ref(),
            Some(&(200, "image/png".to_owned(), b"PNG-BYTES".to_vec()))
        );
    }

    #[tokio::test]
    async fn invalid_folders_and_non_image_uploads_are_rejected_before_storage() {
        let bench = bench_with(Diagnostics::default());
        let (status, body) = send(
            &bench.router,
            get("/api/v2/folder-presentations/0".to_owned()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "invalid-folder");

        let boundary = "tosklight-boundary";
        let payload = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bad.txt\"\r\nContent-Type: text/plain\r\n\r\nNO\r\n--{boundary}--\r\n"
        );
        let request = Request::builder()
            .method("POST")
            .uri("/api/v2/folder-presentations/200/picture/upload?requestId=picture-bad")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(payload))
            .unwrap();
        let response = bench.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "invalid-folder-picture-type");
    }
}
