use super::*;

#[derive(Default, Deserialize)]
pub(super) struct VisualizationQuery {
    #[serde(default)]
    pub(super) preload: bool,
}
pub(super) async fn media_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let statuses = state.media_status.read();
    let fixtures = state
        .engine
        .snapshot()
        .fixtures
        .iter()
        .filter_map(|fixture| {
            fixture.direct_control.as_ref().map(|endpoint| {
                let status = statuses.get(&fixture.fixture_id).cloned().unwrap_or_default();
                serde_json::json!({
                    "fixture_id": fixture.fixture_id,
                    "name": format!("{} {}", fixture.definition.manufacturer, fixture.definition.model),
                    "endpoint": endpoint,
                    "layers": fixture.logical_heads,
                    "status": status,
                })
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "fixtures": fixtures })))
}

pub(super) async fn refresh_media_thumbnails(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    headers: HeaderMap,
    Json(input): Json<ThumbnailRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if !(1..=2).contains(&input.library_type) || input.library_level > 3 {
        return Err(ApiError::bad_request("invalid CITP library type or level"));
    }
    let library = library_id(&input);
    let address = media_endpoint(&state, fixture_id)?;
    let result = async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client
            .request_thumbnail(
                input.library_type,
                library,
                &input.elements,
                input.width,
                input.height,
            )
            .await
    }
    .await;
    match result {
        Ok(images) => {
            let count = images.len();
            let mut cache = state.media_cache.lock();
            for (element, image) in images {
                cache
                    .put_thumbnail(
                        ThumbnailKey {
                            fixture: fixture_id.0.to_string(),
                            library_type: input.library_type,
                            library,
                            element,
                        },
                        image,
                    )
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            update_media_status(&state, fixture_id, None);
            emit(
                &state,
                "media_thumbnails_refreshed",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id,"count":count}),
            );
            Ok(Json(
                serde_json::json!({"fixture_id":fixture_id,"count":count}),
            ))
        }
        Err(error) => {
            update_media_status(&state, fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn media_thumbnail(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    Query(query): Query<ThumbnailQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let key = ThumbnailKey {
        fixture: fixture_id.0.to_string(),
        library_type: query.library_type,
        library: LibraryId {
            level: query.library_level,
            ids: [query.library_1, query.library_2, query.library_3],
        },
        element: query.element,
    };
    cached_image_response(state.media_cache.lock().thumbnail(&key), "thumbnail")
}

pub(super) async fn refresh_media_preview(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    headers: HeaderMap,
    Json(input): Json<PreviewRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let address = media_endpoint(&state, fixture_id)?;
    let result = async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client
            .request_preview(input.source, input.width, input.height)
            .await
    }
    .await;
    match result {
        Ok(image) => {
            let format = image.format;
            let width = image.width;
            let height = image.height;
            state
                .media_cache
                .lock()
                .put_preview(
                    PreviewKey {
                        fixture: fixture_id.0.to_string(),
                        source: input.source,
                    },
                    image,
                )
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            update_media_status(&state, fixture_id, None);
            emit(
                &state,
                "media_preview_refreshed",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id,"source":input.source}),
            );
            Ok(Json(
                serde_json::json!({"fixture_id":fixture_id,"source":input.source,"format":format,"width":width,"height":height}),
            ))
        }
        Err(error) => {
            update_media_status(&state, fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn media_preview(
    State(state): State<AppState>,
    Path((fixture_id, source)): Path<(light_core::FixtureId, u16)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    cached_image_response(
        state.media_cache.lock().preview(&PreviewKey {
            fixture: fixture_id.0.to_string(),
            source,
        }),
        "preview",
    )
}

pub(super) fn media_endpoint(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<SocketAddr, ApiError> {
    let snapshot = state.engine.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    let endpoint = fixture
        .direct_control
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("fixture has no direct-control endpoint"))?;
    Ok(SocketAddr::new(endpoint.ip_address, endpoint.port))
}
pub(super) fn library_id(input: &ThumbnailRequest) -> LibraryId {
    LibraryId {
        level: input.library_level,
        ids: [input.library_1, input.library_2, input.library_3],
    }
}
pub(super) fn update_media_status(
    state: &AppState,
    fixture_id: light_core::FixtureId,
    error: Option<String>,
) {
    let mut statuses = state.media_status.write();
    let status = statuses.entry(fixture_id).or_default();
    status.online = error.is_none();
    if let Some(error) = error {
        status.last_error = Some(error);
    } else {
        status.last_success = Some(chrono::Utc::now().to_rfc3339());
        status.last_error = None;
    }
}
pub(super) fn cached_image_response(
    image: Option<light_media::CachedImage>,
    kind: &str,
) -> Result<Response, ApiError> {
    let image = image.ok_or_else(|| ApiError::not_found(format!("cached media {kind}")))?;
    let mut response = image.image.bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(image.image.format.mime()),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, max-age=5"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-light-image-width"),
        header::HeaderValue::from_str(&image.image.width.to_string()).expect("valid width header"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-light-image-height"),
        header::HeaderValue::from_str(&image.image.height.to_string())
            .expect("valid height header"),
    );
    Ok(response)
}
