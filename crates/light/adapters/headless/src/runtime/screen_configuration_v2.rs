//! Typed replay-safe desk-store screen configuration routes.

use super::*;
use crate::tolerant_json::TolerantJson;
use light_wire::v2::screen_configuration as wire;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/screens", get(snapshot))
        .route("/api/v2/screens/create", post(create))
        .route("/api/v2/screens/{screen_id}/update", post(update))
        .route("/api/v2/screens/{screen_id}/delete", post(delete))
        .route("/api/v2/screens/actions", post(apply_action))
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::ScreenConfigurationSnapshot>, ApiError> {
    let session = authenticate(&state, &headers)?;
    screen_snapshot(&state, &session).map(Json)
}

async fn apply_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScreenConfigurationActionRequest>,
) -> Result<Json<wire::ScreenConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    apply_authenticated_action(&state, &session, request.request_id, request.action).await
}

async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScreenConfigurationCreateRequest>,
) -> Result<Json<wire::ScreenConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    apply_authenticated_action(
        &state,
        &session,
        request.request_id,
        wire::ScreenConfigurationAction::Create {
            configuration: request.configuration,
        },
    )
    .await
}

async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(screen_id): Path<Uuid>,
    TolerantJson(request): TolerantJson<wire::ScreenConfigurationUpdateRequest>,
) -> Result<Json<wire::ScreenConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    apply_authenticated_action(
        &state,
        &session,
        request.request_id,
        wire::ScreenConfigurationAction::Update {
            screen_id,
            patch: request.patch,
        },
    )
    .await
}

async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(screen_id): Path<Uuid>,
    TolerantJson(request): TolerantJson<wire::ScreenConfigurationDeleteRequest>,
) -> Result<Json<wire::ScreenConfigurationActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    apply_authenticated_action(
        &state,
        &session,
        request.request_id,
        wire::ScreenConfigurationAction::Delete { screen_id },
    )
    .await
}

async fn apply_authenticated_action(
    state: &AppState,
    session: &Session,
    request_id: String,
    action: wire::ScreenConfigurationAction,
) -> Result<Json<wire::ScreenConfigurationActionOutcome>, ApiError> {
    show_objects_v2::validate_request_id(&request_id)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request_id.clone(),
    };
    if let Some(outcome) = state
        .replay
        .lookup_screen_configuration(&key, &action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let mut outcome = execute_action(state, session.desk.id, action.clone())?;
    outcome.request_id = request_id;
    state
        .replay
        .insert_screen_configuration(key, action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn execute_action(
    state: &AppState,
    desk_id: Uuid,
    action: wire::ScreenConfigurationAction,
) -> Result<wire::ScreenConfigurationActionOutcome, ApiError> {
    match action {
        wire::ScreenConfigurationAction::Create { configuration } => {
            if state
                .installation
                .screen(configuration.id)
                .map_err(ApiError::store)?
                .is_some()
            {
                return Err(ApiError::conflict("screen already exists"));
            }
            let screen = state
                .installation
                .put_screen(desk_id, domain_screen(configuration)?)
                .map_err(ApiError::store)?;
            emit_screen_changed(state, &screen);
            outcome(Some(screen), None)
        }
        wire::ScreenConfigurationAction::Update { screen_id, patch } => {
            let existing = state
                .installation
                .screen(screen_id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("screen"))?;
            let screen = state
                .installation
                .put_screen(desk_id, apply_patch(existing, patch)?)
                .map_err(ApiError::store)?;
            emit_screen_changed(state, &screen);
            outcome(Some(screen), None)
        }
        wire::ScreenConfigurationAction::Delete { screen_id } => {
            if state
                .installation
                .screen(screen_id)
                .map_err(ApiError::store)?
                .is_none()
            {
                return Err(ApiError::not_found("screen"));
            }
            state
                .installation
                .delete_screen(screen_id)
                .map_err(ApiError::store)?;
            emit(
                state,
                "screen_configuration_changed",
                serde_json::json!({"screen_id":screen_id,"deleted":true}),
            );
            let programmer_control_surface = state
                .installation
                .programmer_control_surface()
                .map_err(ApiError::store)?;
            outcome(None, Some(programmer_control_surface))
        }
        wire::ScreenConfigurationAction::UpdateProgrammerControlSurface { patch } => {
            let mut configuration = state
                .installation
                .programmer_control_surface()
                .map_err(ApiError::store)?;
            if patch.assign_to_main {
                configuration.owner_screen_id = None;
            } else if let Some(owner_screen_id) = patch.owner_screen_id {
                configuration.owner_screen_id = Some(owner_screen_id);
            }
            if let Some(visible_encoders) = patch.visible_encoders {
                configuration.visible_encoders = visible_encoders;
            }
            let configuration = state
                .installation
                .put_programmer_control_surface(configuration)
                .map_err(ApiError::store)?;
            emit(
                state,
                "screen_configuration_changed",
                serde_json::json!({"programmer_control_surface":configuration}),
            );
            outcome(None, Some(configuration))
        }
    }
}

fn screen_snapshot(
    state: &AppState,
    session: &Session,
) -> Result<wire::ScreenConfigurationSnapshot, ApiError> {
    let screens = state.installation.screens().map_err(ApiError::store)?;
    let surfaces = super::playback_surfaces_http::surface_snapshot(state, session.desk.id)?;
    Ok(wire::ScreenConfigurationSnapshot {
        screens: screens
            .into_iter()
            .map(wire_screen)
            .collect::<Result<_, _>>()?,
        surfaces,
        programmer_control_surface: wire_programmer_control_surface(
            state
                .installation
                .programmer_control_surface()
                .map_err(ApiError::store)?,
        ),
    })
}

fn wire_programmer_control_surface(
    configuration: light_show::ProgrammerControlSurfaceConfiguration,
) -> wire::ProgrammerControlSurfaceConfiguration {
    wire::ProgrammerControlSurfaceConfiguration {
        owner_screen_id: configuration.owner_screen_id,
        visible_encoders: configuration.visible_encoders,
    }
}

fn apply_patch(
    mut screen: ScreenConfiguration,
    patch: wire::ScreenConfigurationPatch,
) -> Result<ScreenConfiguration, ApiError> {
    macro_rules! patch {
        ($field:ident) => {
            if let Some(value) = patch.$field {
                screen.$field = value;
            }
        };
    }
    patch!(name);
    patch!(layout);
    patch!(show_dock);
    patch!(show_playbacks);
    patch!(surface_id);
    patch!(show_page_controls);
    patch!(show_programmer);
    patch!(not_editable);
    patch!(desired_open);
    if patch.clear_display_id {
        screen.display_id = None;
    } else if let Some(display_id) = patch.display_id {
        screen.display_id = Some(display_id);
    }
    if patch.clear_bounds {
        screen.bounds = None;
    } else if let Some(bounds) = patch.bounds {
        screen.bounds = Some(bounds);
    }
    patch!(fullscreen);
    if let Some(content) = patch.content {
        screen.content = domain_content(content);
    }
    Ok(screen)
}

fn domain_screen(screen: wire::ScreenConfiguration) -> Result<ScreenConfiguration, ApiError> {
    Ok(ScreenConfiguration {
        id: screen.id,
        name: screen.name,
        layout: screen.layout,
        show_dock: screen.show_dock,
        show_playbacks: screen.show_playbacks,
        surface_id: screen.surface_id,
        show_page_controls: screen.show_page_controls,
        show_programmer: screen.show_programmer,
        not_editable: screen.not_editable,
        desired_open: screen.desired_open,
        display_id: screen.display_id,
        bounds: screen.bounds,
        fullscreen: screen.fullscreen,
        content: domain_content(screen.content),
    })
}

fn wire_screen(screen: ScreenConfiguration) -> Result<wire::ScreenConfiguration, ApiError> {
    Ok(wire::ScreenConfiguration {
        id: screen.id,
        name: screen.name,
        layout: screen.layout,
        show_dock: screen.show_dock,
        show_playbacks: screen.show_playbacks,
        surface_id: screen.surface_id,
        show_page_controls: screen.show_page_controls,
        show_programmer: screen.show_programmer,
        not_editable: screen.not_editable,
        desired_open: screen.desired_open,
        display_id: screen.display_id,
        bounds: screen.bounds,
        fullscreen: screen.fullscreen,
        content: wire_content(screen.content),
    })
}

fn domain_content(content: wire::ScreenContent) -> light_show::ScreenContent {
    match content {
        wire::ScreenContent::Desktop => light_show::ScreenContent::Desktop,
        wire::ScreenContent::ControlSurface => light_show::ScreenContent::ControlSurface,
        wire::ScreenContent::FixedPane { pane } => light_show::ScreenContent::FixedPane {
            pane: domain_fixed_pane(pane),
        },
        wire::ScreenContent::FixedSidePane {
            pane,
            side,
            width_percent,
        } => light_show::ScreenContent::FixedSidePane {
            pane: domain_fixed_pane(pane),
            side: match side {
                wire::FixedScreenSide::Left => light_show::FixedScreenSide::Left,
                wire::FixedScreenSide::Right => light_show::FixedScreenSide::Right,
            },
            width_percent,
        },
    }
}

fn domain_fixed_pane(pane: wire::FixedScreenPane) -> light_show::FixedScreenPane {
    match pane {
        wire::FixedScreenPane::FixtureSheet {
            included_heads,
            order,
            active_only,
            compact_mode,
            cue_list_id,
            columns,
            show_type,
            show_group_shortcuts,
        } => light_show::FixedScreenPane::FixtureSheet {
            included_heads: match included_heads {
                wire::FixedScreenFixtureIncludedHeads::All => {
                    light_show::FixedScreenFixtureIncludedHeads::All
                }
                wire::FixedScreenFixtureIncludedHeads::NoSubHeads => {
                    light_show::FixedScreenFixtureIncludedHeads::NoSubHeads
                }
                wire::FixedScreenFixtureIncludedHeads::NoMasterHeads => {
                    light_show::FixedScreenFixtureIncludedHeads::NoMasterHeads
                }
            },
            order: match order {
                wire::FixedScreenFixtureOrder::FixtureId => {
                    light_show::FixedScreenFixtureOrder::FixtureId
                }
                wire::FixedScreenFixtureOrder::Active => {
                    light_show::FixedScreenFixtureOrder::Active
                }
            },
            active_only,
            compact_mode: match compact_mode {
                wire::FixedScreenFixtureCompactMode::Off => {
                    light_show::FixedScreenFixtureCompactMode::Off
                }
                wire::FixedScreenFixtureCompactMode::IconOnly => {
                    light_show::FixedScreenFixtureCompactMode::IconOnly
                }
                wire::FixedScreenFixtureCompactMode::TextOnly => {
                    light_show::FixedScreenFixtureCompactMode::TextOnly
                }
            },
            cue_list_id,
            columns: columns.into_iter().map(domain_fixture_column).collect(),
            show_type,
            show_group_shortcuts,
        },
        wire::FixedScreenPane::Stage2d {
            follow_preload,
            show_floor_grid,
        } => light_show::FixedScreenPane::Stage2d {
            follow_preload,
            show_floor_grid,
        },
        wire::FixedScreenPane::Stage3d {
            follow_preload,
            show_floor_grid,
            show_beam_guides,
            render_quality,
            environment_brightness,
        } => light_show::FixedScreenPane::Stage3d {
            follow_preload,
            show_floor_grid,
            show_beam_guides,
            render_quality: match render_quality {
                wire::FixedScreenStageRenderQuality::LinesOnly => {
                    light_show::FixedScreenStageRenderQuality::LinesOnly
                }
                wire::FixedScreenStageRenderQuality::LinesAndBeams => {
                    light_show::FixedScreenStageRenderQuality::LinesAndBeams
                }
                wire::FixedScreenStageRenderQuality::Full => {
                    light_show::FixedScreenStageRenderQuality::Full
                }
            },
            environment_brightness,
        },
        wire::FixedScreenPane::Cues { cue_list_id } => {
            light_show::FixedScreenPane::Cues { cue_list_id }
        }
        wire::FixedScreenPane::Text { root, path, mode } => light_show::FixedScreenPane::Text {
            root,
            path,
            mode: match mode {
                wire::FixedScreenTextMode::Plain => light_show::FixedScreenTextMode::Plain,
                wire::FixedScreenTextMode::Markdown => light_show::FixedScreenTextMode::Markdown,
            },
        },
    }
}

fn domain_fixture_column(
    column: wire::FixedScreenFixtureColumn,
) -> light_show::FixedScreenFixtureColumn {
    match column {
        wire::FixedScreenFixtureColumn::Id => light_show::FixedScreenFixtureColumn::Id,
        wire::FixedScreenFixtureColumn::Icon => light_show::FixedScreenFixtureColumn::Icon,
        wire::FixedScreenFixtureColumn::Name => light_show::FixedScreenFixtureColumn::Name,
        wire::FixedScreenFixtureColumn::Patch => light_show::FixedScreenFixtureColumn::Patch,
        wire::FixedScreenFixtureColumn::Intensity => {
            light_show::FixedScreenFixtureColumn::Intensity
        }
        wire::FixedScreenFixtureColumn::Color => light_show::FixedScreenFixtureColumn::Color,
        wire::FixedScreenFixtureColumn::Position => light_show::FixedScreenFixtureColumn::Position,
        wire::FixedScreenFixtureColumn::Beam => light_show::FixedScreenFixtureColumn::Beam,
        wire::FixedScreenFixtureColumn::Shapers => light_show::FixedScreenFixtureColumn::Shapers,
        wire::FixedScreenFixtureColumn::Focus => light_show::FixedScreenFixtureColumn::Focus,
        wire::FixedScreenFixtureColumn::Control => light_show::FixedScreenFixtureColumn::Control,
        wire::FixedScreenFixtureColumn::Media => light_show::FixedScreenFixtureColumn::Media,
    }
}

fn wire_content(content: light_show::ScreenContent) -> wire::ScreenContent {
    match content {
        light_show::ScreenContent::Desktop => wire::ScreenContent::Desktop,
        light_show::ScreenContent::ControlSurface => wire::ScreenContent::ControlSurface,
        light_show::ScreenContent::FixedPane { pane } => wire::ScreenContent::FixedPane {
            pane: wire_fixed_pane(pane),
        },
        light_show::ScreenContent::FixedSidePane {
            pane,
            side,
            width_percent,
        } => wire::ScreenContent::FixedSidePane {
            pane: wire_fixed_pane(pane),
            side: match side {
                light_show::FixedScreenSide::Left => wire::FixedScreenSide::Left,
                light_show::FixedScreenSide::Right => wire::FixedScreenSide::Right,
            },
            width_percent,
        },
    }
}

fn wire_fixed_pane(pane: light_show::FixedScreenPane) -> wire::FixedScreenPane {
    match pane {
        light_show::FixedScreenPane::FixtureSheet {
            included_heads,
            order,
            active_only,
            compact_mode,
            cue_list_id,
            columns,
            show_type,
            show_group_shortcuts,
        } => wire::FixedScreenPane::FixtureSheet {
            included_heads: match included_heads {
                light_show::FixedScreenFixtureIncludedHeads::All => {
                    wire::FixedScreenFixtureIncludedHeads::All
                }
                light_show::FixedScreenFixtureIncludedHeads::NoSubHeads => {
                    wire::FixedScreenFixtureIncludedHeads::NoSubHeads
                }
                light_show::FixedScreenFixtureIncludedHeads::NoMasterHeads => {
                    wire::FixedScreenFixtureIncludedHeads::NoMasterHeads
                }
            },
            order: match order {
                light_show::FixedScreenFixtureOrder::FixtureId => {
                    wire::FixedScreenFixtureOrder::FixtureId
                }
                light_show::FixedScreenFixtureOrder::Active => {
                    wire::FixedScreenFixtureOrder::Active
                }
            },
            active_only,
            compact_mode: match compact_mode {
                light_show::FixedScreenFixtureCompactMode::Off => {
                    wire::FixedScreenFixtureCompactMode::Off
                }
                light_show::FixedScreenFixtureCompactMode::IconOnly => {
                    wire::FixedScreenFixtureCompactMode::IconOnly
                }
                light_show::FixedScreenFixtureCompactMode::TextOnly => {
                    wire::FixedScreenFixtureCompactMode::TextOnly
                }
            },
            cue_list_id,
            columns: columns.into_iter().map(wire_fixture_column).collect(),
            show_type,
            show_group_shortcuts,
        },
        light_show::FixedScreenPane::Stage2d {
            follow_preload,
            show_floor_grid,
        } => wire::FixedScreenPane::Stage2d {
            follow_preload,
            show_floor_grid,
        },
        light_show::FixedScreenPane::Stage3d {
            follow_preload,
            show_floor_grid,
            show_beam_guides,
            render_quality,
            environment_brightness,
        } => wire::FixedScreenPane::Stage3d {
            follow_preload,
            show_floor_grid,
            show_beam_guides,
            render_quality: match render_quality {
                light_show::FixedScreenStageRenderQuality::LinesOnly => {
                    wire::FixedScreenStageRenderQuality::LinesOnly
                }
                light_show::FixedScreenStageRenderQuality::LinesAndBeams => {
                    wire::FixedScreenStageRenderQuality::LinesAndBeams
                }
                light_show::FixedScreenStageRenderQuality::Full => {
                    wire::FixedScreenStageRenderQuality::Full
                }
            },
            environment_brightness,
        },
        light_show::FixedScreenPane::Cues { cue_list_id } => {
            wire::FixedScreenPane::Cues { cue_list_id }
        }
        light_show::FixedScreenPane::Text { root, path, mode } => wire::FixedScreenPane::Text {
            root,
            path,
            mode: match mode {
                light_show::FixedScreenTextMode::Plain => wire::FixedScreenTextMode::Plain,
                light_show::FixedScreenTextMode::Markdown => wire::FixedScreenTextMode::Markdown,
            },
        },
    }
}

fn wire_fixture_column(
    column: light_show::FixedScreenFixtureColumn,
) -> wire::FixedScreenFixtureColumn {
    match column {
        light_show::FixedScreenFixtureColumn::Id => wire::FixedScreenFixtureColumn::Id,
        light_show::FixedScreenFixtureColumn::Icon => wire::FixedScreenFixtureColumn::Icon,
        light_show::FixedScreenFixtureColumn::Name => wire::FixedScreenFixtureColumn::Name,
        light_show::FixedScreenFixtureColumn::Patch => wire::FixedScreenFixtureColumn::Patch,
        light_show::FixedScreenFixtureColumn::Intensity => {
            wire::FixedScreenFixtureColumn::Intensity
        }
        light_show::FixedScreenFixtureColumn::Color => wire::FixedScreenFixtureColumn::Color,
        light_show::FixedScreenFixtureColumn::Position => wire::FixedScreenFixtureColumn::Position,
        light_show::FixedScreenFixtureColumn::Beam => wire::FixedScreenFixtureColumn::Beam,
        light_show::FixedScreenFixtureColumn::Shapers => wire::FixedScreenFixtureColumn::Shapers,
        light_show::FixedScreenFixtureColumn::Focus => wire::FixedScreenFixtureColumn::Focus,
        light_show::FixedScreenFixtureColumn::Control => wire::FixedScreenFixtureColumn::Control,
        light_show::FixedScreenFixtureColumn::Media => wire::FixedScreenFixtureColumn::Media,
    }
}

fn emit_screen_changed(state: &AppState, screen: &ScreenConfiguration) {
    emit(
        state,
        "screen_configuration_changed",
        serde_json::json!({"screen":screen}),
    );
}

fn outcome(
    screen: Option<ScreenConfiguration>,
    programmer_control_surface: Option<light_show::ProgrammerControlSurfaceConfiguration>,
) -> Result<wire::ScreenConfigurationActionOutcome, ApiError> {
    Ok(wire::ScreenConfigurationActionOutcome {
        request_id: String::new(),
        replayed: false,
        screen: screen.map(wire_screen).transpose()?,
        programmer_control_surface: programmer_control_surface.map(wire_programmer_control_surface),
    })
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

#[derive(Clone)]
struct ReplayEntry {
    key: ReplayKey,
    action: wire::ScreenConfigurationAction,
    outcome: wire::ScreenConfigurationActionOutcome,
}

#[derive(Default)]
pub(super) struct ScreenConfigurationReplayCache {
    entries: VecDeque<ReplayEntry>,
}

impl ScreenConfigurationReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        action: &wire::ScreenConfigurationAction,
    ) -> Result<Option<wire::ScreenConfigurationActionOutcome>, ApiError> {
        let Some(entry) = self.entries.iter().find(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different screen action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        action: wire::ScreenConfigurationAction,
        outcome: wire::ScreenConfigurationActionOutcome,
    ) {
        self.entries.push_back(ReplayEntry {
            key,
            action,
            outcome,
        });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            self.entries.pop_front();
        }
    }
}
