use super::*;

#[derive(Default, Deserialize)]
pub(super) struct VisualizationQuery {
    #[serde(default)]
    pub(super) preload: bool,
    #[serde(default)]
    pub(super) dynamic_stack_only: bool,
    pub(super) fixture_ids: Option<String>,
}
pub(super) async fn media_servers(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let fixtures = state
        .output
        .snapshot()
        .fixtures
        .iter()
        .filter(|fixture| {
            !fixture.logical_heads.is_empty()
                && fixture
                    .definition
                    .direct_control_protocols
                    .contains(&light_fixture::DirectControlProtocol::Citp)
        })
        .map(|fixture| {
            let endpoint = fixture
                .direct_control
                .as_ref()
                .filter(|endpoint| endpoint.protocol == light_fixture::DirectControlProtocol::Citp);
            let status = state.media.status(fixture.fixture_id);
            serde_json::json!({
                "fixture_id": fixture.fixture_id,
                "name": format!("{} {}", fixture.definition.manufacturer, fixture.definition.model),
                "endpoint": endpoint,
                "layers": fixture.logical_heads,
                "status": status,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "fixtures": fixtures })))
}

pub(super) async fn inspect_media_server(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Json<light_media::MediaServerSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    let address = media_endpoint(&state, fixture_id)?;
    match async {
        let mut client = CitpClient::connect(address, Duration::from_secs(3)).await?;
        client.inspect().await
    }
    .await
    {
        Ok(mut snapshot) => {
            snapshot.capabilities.layers = media_layer_capabilities(&state, fixture_id)?;
            state.media.record_inspection(fixture_id, snapshot.clone());
            state.media.record_status(fixture_id, None);
            emit(
                &state,
                "media_inspected",
                serde_json::json!({"session_id":session.id,"fixture_id":fixture_id}),
            );
            Ok(Json(snapshot))
        }
        Err(error) => {
            state.media.clear_inspection(fixture_id);
            state
                .media
                .record_status(fixture_id, Some(error.to_string()));
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn apply_media_library_selection(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::MediaLibrarySelectionRequest>,
) -> Result<Json<light_wire::v2::output_control::MediaLibrarySelectionOutcome>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    show.verify(&state)?;
    if input.request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "media selection request_id is required",
        ));
    }
    let inspection = state.media.inspection(fixture_id).ok_or_else(|| {
        ApiError::conflict("Media library must be inspected before selecting a file")
    })?;
    if inspection.library_revision != input.expected_library_revision {
        return Err(ApiError::conflict(format!(
            "Media library revision is stale: expected {}, current {}",
            input.expected_library_revision, inspection.library_revision
        )));
    }
    if !inspection
        .files
        .iter()
        .any(|file| file.folder_id == input.folder && file.id == input.file)
    {
        return Err(ApiError::conflict(
            "The selected media file is absent from the inspected library revision",
        ));
    }
    let capabilities = inspection
        .capabilities
        .layers
        .iter()
        .find(|capability| {
            layer_head_index(
                &state,
                fixture_id,
                light_core::FixtureId(input.layer_fixture_id),
            ) == Some(capability.layer)
        })
        .ok_or_else(|| ApiError::bad_request("selected fixture is not a layer of this server"))?;
    let (folder_attribute, file_attribute, supported) = match input.kind {
        light_wire::v2::output_control::MediaLibraryKind::Content => {
            ("media.folder", "media.file", capabilities.content_library)
        }
        light_wire::v2::output_control::MediaLibraryKind::Mask => (
            "media.mask.folder",
            "media.mask.file",
            capabilities.mask_library,
        ),
    };
    if !supported {
        return Err(ApiError::bad_request(
            "The selected layer does not advertise this library capability",
        ));
    }

    let context = operator_action_context(&session, light_application::ActionSource::UserInterface);
    let ports = command_http::ServerProgrammingPorts::new(&state, &session, "media", true);
    let values = state
        .programming
        .values_snapshot(&context, &ports)
        .map_err(|error| ApiError::conflict(error.message))?;
    let capture = state
        .programming
        .capture_mode_snapshot(&context, &ports)
        .map_err(|error| ApiError::conflict(error.message))?;
    let context = context
        .with_request_id(input.request_id.clone())
        .with_expected_revision(values.projection.revision);
    let timing = light_application::ProgrammingValueTiming::default();
    let fixture_id = light_core::FixtureId(input.layer_fixture_id);
    let normalized = |value: u8| light_core::AttributeValue::Normalized(f32::from(value) / 255.0);
    let command = light_application::ProgrammingValuesRequest {
        expected_capture_mode_revision: capture.projection.revision,
        command: light_application::ProgrammingValuesCommand::Batch {
            mutations: vec![
                light_application::ProgrammingValueMutation::SetFixture {
                    fixture_id,
                    attribute: light_core::AttributeKey(folder_attribute.into()),
                    value: normalized(input.folder),
                    timing,
                },
                light_application::ProgrammingValueMutation::SetFixture {
                    fixture_id,
                    attribute: light_core::AttributeKey(file_attribute.into()),
                    value: normalized(input.file),
                    timing,
                },
            ],
        },
    };
    let _activation = state.active_show.acquire().await;
    let result = state
        .programming
        .handle_values(
            light_application::ActionEnvelope { context, command },
            &ports,
        )
        .map_err(|error| ApiError::conflict(error.message))?;
    Ok(Json(
        light_wire::v2::output_control::MediaLibrarySelectionOutcome {
            request_id: input.request_id,
            library_revision: inspection.library_revision,
            programmer_revision: result.outcome.revision(),
        },
    ))
}

fn layer_head_index(
    state: &AppState,
    master_id: light_core::FixtureId,
    layer_id: light_core::FixtureId,
) -> Option<u16> {
    state
        .output
        .snapshot()
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == master_id)?
        .logical_heads
        .iter()
        .find(|head| head.fixture_id == layer_id)
        .map(|head| head.head_index)
}

fn media_layer_capabilities(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<Vec<light_media::MediaLayerCapabilities>, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    Ok(fixture
        .logical_heads
        .iter()
        .filter_map(|patched| {
            let head = fixture
                .definition
                .heads
                .iter()
                .find(|head| head.index == patched.head_index)?;
            let attributes = head
                .parameters
                .iter()
                .map(|parameter| parameter.attribute.0.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            let content_library =
                attributes.contains("media.folder") && attributes.contains("media.file");
            let mask_library =
                attributes.contains("media.mask.folder") && attributes.contains("media.mask.file");
            let secondary_controls = head
                .parameters
                .iter()
                .map(|parameter| parameter.attribute.0.as_str())
                .filter(|attribute| attribute.starts_with("media."))
                .filter(|attribute| {
                    !matches!(
                        *attribute,
                        "media.folder" | "media.file" | "media.mask.folder" | "media.mask.file"
                    )
                })
                .map(|attribute| light_media::MediaControlCapability {
                    attribute: attribute.to_owned(),
                })
                .collect();
            Some(light_media::MediaLayerCapabilities {
                layer: patched.head_index,
                content_library,
                mask_library,
                secondary_controls,
            })
        })
        .collect())
}

pub(super) async fn refresh_media_thumbnails(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::MediaThumbnailRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
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
            state
                .media
                .put_thumbnails(images.into_iter().map(|(element, image)| {
                    (
                        ThumbnailKey {
                            fixture: fixture_id.0.to_string(),
                            library_type: input.library_type,
                            library,
                            element,
                        },
                        image,
                    )
                }))
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            state.media.record_status(fixture_id, None);
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
            state
                .media
                .record_status(fixture_id, Some(error.to_string()));
            emit(
                &state,
                "media_server_offline",
                serde_json::json!({"fixture_id":fixture_id,"error":error.to_string()}),
            );
            Err(ApiError::unavailable(error.to_string()))
        }
    }
}

pub(super) async fn refresh_media_preview(
    State(state): State<AppState>,
    Path(fixture_id): Path<light_core::FixtureId>,
    show: ShowContext,
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<light_wire::v2::output_control::MediaPreviewRefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    show.verify(&state)?;
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
                .media
                .put_preview(
                    PreviewKey {
                        fixture: fixture_id.0.to_string(),
                        source: input.source,
                    },
                    image,
                )
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            state.media.record_status(fixture_id, None);
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
            state
                .media
                .record_status(fixture_id, Some(error.to_string()));
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
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    cached_image_response(
        state.media.preview(&PreviewKey {
            fixture: fixture_id.0.to_string(),
            source,
        }),
        "preview",
    )
}

pub(super) async fn media_thumbnail(
    State(state): State<AppState>,
    Path((fixture_id, folder, element)): Path<(light_core::FixtureId, u8, u8)>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    show.verify(&state)?;
    cached_image_response(
        state.media.thumbnail(&ThumbnailKey {
            fixture: fixture_id.0.to_string(),
            library_type: 1,
            library: LibraryId {
                level: 1,
                ids: [folder, 0, 0],
            },
            element,
        }),
        "thumbnail",
    )
}

pub(super) fn media_endpoint(
    state: &AppState,
    fixture_id: light_core::FixtureId,
) -> Result<SocketAddr, ApiError> {
    let snapshot = state.output.snapshot();
    let fixture = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == fixture_id)
        .ok_or_else(|| ApiError::not_found("fixture"))?;
    let endpoint = fixture
        .direct_control
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("fixture has no direct-control endpoint"))?;
    if endpoint.protocol != light_fixture::DirectControlProtocol::Citp {
        return Err(ApiError::bad_request(
            "fixture direct-control endpoint is not CITP",
        ));
    }
    Ok(SocketAddr::new(endpoint.ip_address, endpoint.port))
}
pub(super) fn library_id(
    input: &light_wire::v2::output_control::MediaThumbnailRefreshRequest,
) -> LibraryId {
    LibraryId {
        level: input.library_level,
        ids: [input.library_1, input.library_2, input.library_3],
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
    if let Ok(received_at) = image.received_at.duration_since(std::time::UNIX_EPOCH) {
        response.headers_mut().insert(
            header::HeaderName::from_static("x-light-received-at-millis"),
            header::HeaderValue::from_str(&received_at.as_millis().to_string())
                .expect("valid received-at header"),
        );
    }
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
