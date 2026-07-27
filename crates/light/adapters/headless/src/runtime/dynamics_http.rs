use super::dynamics_adapter::{ServerDynamicsPorts, controller_for_runtime_instance};
use super::live_action_http::run_http_programming_action;
use super::playback_service::projection::dynamic_target_lane_coverage;
use super::{
    ApiError, AppState, DeskContext, ShowContext, persist_output_runtime, persist_programmer,
    session_for_desk,
};
use crate::tolerant_json::TolerantJson;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use light_application::{
    ActionContext, ActionSource, DynamicControllerUpdate, DynamicFixAtCommand, DynamicOffCommand,
    DynamicStartCommand,
};
use light_core::{AttributeKey, FixtureId};
use light_dynamics::{
    DynamicInstanceOverrides, DynamicSpeed, DynamicValueTiming, Rational, SpeedGroup,
};
use light_wire::v2::dynamics::{
    DynamicControllerActionOutcome, DynamicControllerValueActionRequest,
    DynamicDefinitionStatusProjection, DynamicFixAtActionRequest, DynamicInstanceActionOutcome,
    DynamicOffActionRequest, DynamicRuntimeControllerProjection, DynamicRuntimeInstanceProjection,
    DynamicRuntimeSnapshotProjection, DynamicStartActionRequest,
};
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/dynamics/runtime", get(runtime_snapshot))
        .route("/api/v2/dynamics/{dynamic_id}/toggle", post(toggle))
        .route("/api/v2/dynamics/{dynamic_id}/start", post(start))
        .route("/api/v2/dynamic-instances/{instance_id}/off", post(off))
        .route("/api/v2/dynamic-instances/{instance_id}/size", post(size))
        .route("/api/v2/dynamic-instances/{instance_id}/speed", post(speed))
        .route("/api/v2/dynamic-instances/{instance_id}/phase", post(phase))
        .route("/api/v2/programmer/values/fix-at", post(fix_at))
}

async fn runtime_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<DynamicRuntimeSnapshotProjection>, ApiError> {
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let now_millis =
        u64::try_from(state.output.application_time().timestamp_millis()).unwrap_or_default();
    let snapshot = state.output.dynamic_runtime_snapshot();
    let speed_groups = state.output.speed_group_snapshots(now_millis);
    let output_interval_millis = 1_000_u64.div_ceil(u64::from(state.output.frame_rate_hz().max(1)));
    let engine_snapshot = state.output.snapshot();
    let selected = state
        .programming
        .selection(session.id)
        .map_or_else(Vec::new, |selection| selection.selected);
    let definitions = engine_snapshot
        .dynamics
        .iter()
        .map(|definition| dynamic_definition_status(&engine_snapshot, definition, &selected))
        .collect();
    let instances = snapshot
        .instances
        .into_iter()
        .filter(|instance| !instance.completed)
        .map(|instance| {
            let winning_id = instance
                .controllers
                .iter()
                .max_by_key(|controller| {
                    (
                        controller.priority,
                        controller.activated_at_millis,
                        controller.id,
                    )
                })
                .map(|controller| controller.id);
            let winning_speed_multiplier = instance
                .controllers
                .iter()
                .find(|controller| Some(controller.id) == winning_id)
                .map_or(1.0, |controller| f64::from(controller.speed_multiplier));
            let combined_speed_multiplier = (winning_speed_multiplier
                * instance.definition.overall_speed_multiplier.factor())
            .max(f64::EPSILON);
            let (
                speed_source,
                effective_cycle_millis,
                effective_bpm,
                beat_phase,
                transport_advancing,
            ) = match instance.definition.speed {
                DynamicSpeed::Fixed { duration_millis } => (
                    "Fixed".to_owned(),
                    ((duration_millis as f64 / combined_speed_multiplier)
                        .round()
                        .max(1.0)) as u64,
                    None,
                    None,
                    true,
                ),
                DynamicSpeed::SpeedGroup {
                    group,
                    beats_per_cycle,
                } => {
                    let transport = speed_groups[speed_group_index(group)];
                    let cycle_millis = if transport.effective_bpm > f64::EPSILON {
                        (beats_per_cycle.factor() * 60_000.0
                            / transport.effective_bpm
                            / combined_speed_multiplier)
                            .round()
                            .max(1.0) as u64
                    } else {
                        0
                    };
                    (
                        format!("Speed Group {}", speed_group_label(group)),
                        cycle_millis,
                        Some(transport.effective_bpm),
                        Some(transport.beat_phase),
                        transport.phase_advancing,
                    )
                }
            };
            let transitions = instance
                .controller_transitions
                .iter()
                .map(|transition| (transition.controller_id, *transition))
                .collect::<std::collections::HashMap<_, _>>();
            let controllers = instance
                .controllers
                .into_iter()
                .map(|controller| {
                    let transition = transitions.get(&controller.id).copied().unwrap_or(
                        light_dynamics::DynamicControllerTransitionSnapshot {
                            controller_id: controller.id,
                            activation_started_at_millis: controller.activated_at_millis,
                            ..Default::default()
                        },
                    );
                    DynamicRuntimeControllerProjection {
                        controller_id: controller.id,
                        source: dynamic_source_label(&controller.source),
                        priority: controller.priority,
                        size: controller.size,
                        speed_multiplier: controller.speed_multiplier,
                        phase_offset_degrees: controller.phase_offset_degrees,
                        paused: controller.paused,
                        winning: winning_id == Some(controller.id),
                        releasing: transition.release_started_at_millis.is_some(),
                        activation_mix: runtime_transition_mix(transition, now_millis),
                    }
                })
                .collect();
            let aliasing_warning = light_dynamics::aliasing_warning(
                &instance.definition,
                effective_cycle_millis,
                output_interval_millis,
            )
            .map(|warning| {
                format!(
                    "Aliasing: shortest segment is {} ms ({} sample(s) at {} ms output interval; at least 4 required)",
                    warning.shortest_segment_millis,
                    warning.samples_per_segment,
                    warning.output_interval_millis,
                )
            });
            DynamicRuntimeInstanceProjection {
                instance_id: instance.id,
                dynamic_id: instance.definition.id,
                pool_number: instance.definition.pool_number,
                name: instance.definition.name,
                targets: instance
                    .targets
                    .into_iter()
                    .map(|target| target.0)
                    .collect(),
                pending: instance
                    .pending_until_millis
                    .is_some_and(|boundary| now_millis < boundary),
                pending_until_millis: instance.pending_until_millis,
                paused: instance.paused_at_millis.is_some(),
                speed_source,
                activation_boundary: match instance.definition.activation_boundary {
                    light_dynamics::ActivationBoundary::Beat => {
                        light_wire::v2::dynamics::DynamicActivationBoundaryProjection::Beat
                    }
                    light_dynamics::ActivationBoundary::Bar => {
                        light_wire::v2::dynamics::DynamicActivationBoundaryProjection::Bar
                    }
                },
                effective_cycle_millis,
                effective_bpm,
                beat_phase,
                phase_advancing: transport_advancing
                    && !snapshot.global_paused
                    && instance.paused_at_millis.is_none(),
                aliasing_warning,
                controllers,
            }
        })
        .collect();
    Ok(Json(DynamicRuntimeSnapshotProjection {
        global_paused: snapshot.global_paused,
        instances,
        definitions,
    }))
}

fn dynamic_definition_status(
    snapshot: &light_engine::EngineSnapshot,
    definition: &light_dynamics::DynamicDefinition,
    selected: &[FixtureId],
) -> DynamicDefinitionStatusProjection {
    let (targets, binding_warning) = match &definition.target_binding {
        light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => snapshot
            .groups
            .iter()
            .find(|group| group.id == *group_id)
            .map_or_else(
                || {
                    (
                        Vec::new(),
                        Some(format!("Live Group {group_id} is missing")),
                    )
                },
                |group| (group.fixtures.clone(), None),
            ),
        light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => (targets.clone(), None),
        light_dynamics::DynamicTargetBinding::Targetless if !selected.is_empty() => {
            (selected.to_vec(), None)
        }
        light_dynamics::DynamicTargetBinding::Targetless => (
            snapshot
                .fixtures
                .iter()
                .filter(|fixture| {
                    definition.lanes.iter().any(|lane| {
                        fixture
                            .definition
                            .heads
                            .iter()
                            .flat_map(|head| &head.parameters)
                            .any(|parameter| parameter.attribute == lane.attribute)
                    })
                })
                .map(|fixture| fixture.fixture_id)
                .collect(),
            None,
        ),
    };
    let coverage = dynamic_target_lane_coverage(snapshot, definition, &targets);
    let coverage_warning =
        (coverage.missing_target_count > 0 || coverage.skipped_address_count > 0).then(|| {
            format!(
                "{} of {} target/lane addresses run; {} skipped ({} missing targets, {} unpatched targets)",
                coverage.supported_address_count,
                targets.len().saturating_mul(definition.lanes.len()),
                coverage.skipped_address_count,
                coverage.missing_target_count,
                coverage.unpatched_target_count,
            )
        });
    DynamicDefinitionStatusProjection {
        dynamic_id: definition.id,
        target_count: targets.len(),
        compatible_target_count: coverage.compatible_target_count,
        missing_target_count: coverage.missing_target_count,
        unpatched_target_count: coverage.unpatched_target_count,
        lane_count: definition.lanes.len(),
        supported_address_count: coverage.supported_address_count,
        skipped_address_count: coverage.skipped_address_count,
        warning: binding_warning.or(coverage_warning),
    }
}

const fn speed_group_index(group: SpeedGroup) -> usize {
    match group {
        SpeedGroup::A => 0,
        SpeedGroup::B => 1,
        SpeedGroup::C => 2,
        SpeedGroup::D => 3,
        SpeedGroup::E => 4,
    }
}

const fn speed_group_label(group: SpeedGroup) -> &'static str {
    match group {
        SpeedGroup::A => "A",
        SpeedGroup::B => "B",
        SpeedGroup::C => "C",
        SpeedGroup::D => "D",
        SpeedGroup::E => "E",
    }
}

pub(super) fn dynamic_source_label(source: &light_dynamics::DynamicControllerSource) -> String {
    match source {
        light_dynamics::DynamicControllerSource::Programmer { .. } => "Programmer".into(),
        light_dynamics::DynamicControllerSource::Cue { .. } => "Cue".into(),
        light_dynamics::DynamicControllerSource::Playback { playback_number } => {
            format!("Playback {playback_number}")
        }
    }
}

pub(super) fn runtime_transition_mix(
    transition: light_dynamics::DynamicControllerTransitionSnapshot,
    now_millis: u64,
) -> f32 {
    if let Some(started) = transition.release_started_at_millis {
        if now_millis < started.saturating_add(transition.release_delay_millis) {
            return 1.0;
        }
        if transition.release_duration_millis == 0 {
            return 0.0;
        }
        return (1.0
            - now_millis
                .saturating_sub(started)
                .saturating_sub(transition.release_delay_millis) as f32
                / transition.release_duration_millis as f32)
            .clamp(0.0, 1.0);
    }
    if now_millis
        < transition
            .activation_started_at_millis
            .saturating_add(transition.activation_delay_millis)
    {
        return 0.0;
    }
    if transition.activation_duration_millis == 0 {
        return 1.0;
    }
    (now_millis
        .saturating_sub(transition.activation_started_at_millis)
        .saturating_sub(transition.activation_delay_millis) as f32
        / transition.activation_duration_millis as f32)
        .clamp(0.0, 1.0)
}

async fn start(
    State(state): State<AppState>,
    Path(dynamic_id): Path<Uuid>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicStartActionRequest>,
) -> Result<Json<DynamicInstanceActionOutcome>, ApiError> {
    start_or_toggle(state, dynamic_id, show, desk, headers, request, false).await
}

async fn toggle(
    State(state): State<AppState>,
    Path(dynamic_id): Path<Uuid>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicStartActionRequest>,
) -> Result<Json<DynamicInstanceActionOutcome>, ApiError> {
    start_or_toggle(state, dynamic_id, show, desk, headers, request, true).await
}

async fn start_or_toggle(
    state: AppState,
    dynamic_id: Uuid,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: DynamicStartActionRequest,
    toggle: bool,
) -> Result<Json<DynamicInstanceActionOutcome>, ApiError> {
    validate_request_id(&request.request_id)?;
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = request.request_id.clone();
    run_http_programming_action(state, session, request_id, move |state, session, _| {
        let ports = ServerDynamicsPorts { state, session };
        let command = DynamicStartCommand {
            dynamic_id,
            targets: request.targets.into_iter().map(FixtureId).collect(),
            overrides: DynamicInstanceOverrides {
                size: request.overrides.size,
                speed_multiplier: Rational {
                    numerator: request.overrides.speed_multiplier.numerator,
                    denominator: request.overrides.speed_multiplier.denominator,
                },
                phase_offset_degrees: request.overrides.phase_offset_degrees,
            },
            timing: timing(request.timing),
        };
        let result = if toggle {
            state
                .dynamics
                .toggle(&context(session, &request.request_id), command, &ports)
        } else {
            state
                .dynamics
                .start(&context(session, &request.request_id), command, &ports)
        }
        .map_err(|error| error.message)?;
        persist_programmer(state, session).map_err(|error| error.message)?;
        persist_output_runtime(state).map_err(|error| error.message)?;
        Ok(DynamicInstanceActionOutcome {
            request_id: request.request_id,
            runtime_instance_id: result.runtime_instance_id,
            controller_id: result.controller_id,
            targets: result.targets.into_iter().map(|target| target.0).collect(),
            started: result.started,
        })
    })
    .await
    .map(Json)
}

async fn off(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicOffActionRequest>,
) -> Result<Json<DynamicInstanceActionOutcome>, ApiError> {
    validate_request_id(&request.request_id)?;
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = request.request_id.clone();
    run_http_programming_action(state, session, request_id, move |state, session, _| {
        let ports = ServerDynamicsPorts { state, session };
        let controller_id = controller_for_runtime_instance(state, session, instance_id)?;
        let result = state
            .dynamics
            .off(
                &context(session, &request.request_id),
                DynamicOffCommand {
                    controller_id,
                    timing: timing(request.timing),
                },
                &ports,
            )
            .map_err(|error| error.message)?;
        persist_programmer(state, session).map_err(|error| error.message)?;
        persist_output_runtime(state).map_err(|error| error.message)?;
        Ok(DynamicInstanceActionOutcome {
            request_id: request.request_id,
            runtime_instance_id: result.runtime_instance_id,
            controller_id: result.controller_id,
            targets: result.targets.into_iter().map(|target| target.0).collect(),
            started: false,
        })
    })
    .await
    .map(Json)
}

async fn size(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicControllerValueActionRequest>,
) -> Result<Json<DynamicControllerActionOutcome>, ApiError> {
    update(
        state,
        instance_id,
        show,
        desk,
        headers,
        request,
        ControllerField::Size,
    )
    .await
}

async fn speed(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicControllerValueActionRequest>,
) -> Result<Json<DynamicControllerActionOutcome>, ApiError> {
    update(
        state,
        instance_id,
        show,
        desk,
        headers,
        request,
        ControllerField::Speed,
    )
    .await
}

async fn phase(
    State(state): State<AppState>,
    Path(instance_id): Path<Uuid>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicControllerValueActionRequest>,
) -> Result<Json<DynamicControllerActionOutcome>, ApiError> {
    update(
        state,
        instance_id,
        show,
        desk,
        headers,
        request,
        ControllerField::Phase,
    )
    .await
}

enum ControllerField {
    Size,
    Speed,
    Phase,
}

async fn update(
    state: AppState,
    runtime_instance_id: Uuid,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: DynamicControllerValueActionRequest,
    field: ControllerField,
) -> Result<Json<DynamicControllerActionOutcome>, ApiError> {
    validate_request_id(&request.request_id)?;
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = request.request_id.clone();
    run_http_programming_action(state, session, request_id, move |state, session, _| {
        let ports = ServerDynamicsPorts { state, session };
        let controller_id = controller_for_runtime_instance(state, session, runtime_instance_id)?;
        let (size, speed_multiplier, phase_offset_degrees) = match field {
            ControllerField::Size => (Some(request.value), None, None),
            ControllerField::Speed => (None, Some(request.value), None),
            ControllerField::Phase => (None, None, Some(request.value)),
        };
        state
            .dynamics
            .update_controller(
                &context(session, &request.request_id),
                DynamicControllerUpdate {
                    controller_id,
                    size,
                    speed_multiplier,
                    phase_offset_degrees,
                    undo_group: request.undo_group,
                },
                &ports,
            )
            .map_err(|error| error.message)?;
        persist_programmer(state, session).map_err(|error| error.message)?;
        persist_output_runtime(state).map_err(|error| error.message)?;
        Ok(DynamicControllerActionOutcome {
            request_id: request.request_id,
            controller_id,
            changed: true,
        })
    })
    .await
    .map(Json)
}

async fn fix_at(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<DynamicFixAtActionRequest>,
) -> Result<Json<DynamicControllerActionOutcome>, ApiError> {
    validate_request_id(&request.request_id)?;
    let session = session_for_desk(&state, &headers, &desk)?;
    show.verify(&state)?;
    let request_id = request.request_id.clone();
    run_http_programming_action(state, session, request_id, move |state, session, _| {
        let ports = ServerDynamicsPorts { state, session };
        state
            .dynamics
            .fix_at(
                &context(session, &request.request_id),
                DynamicFixAtCommand {
                    targets: request.targets.into_iter().map(FixtureId).collect(),
                    attribute: AttributeKey(request.attribute),
                    value: request.value,
                    timing: timing(request.timing),
                },
                &ports,
            )
            .map_err(|error| error.message)?;
        persist_programmer(state, session).map_err(|error| error.message)?;
        persist_output_runtime(state).map_err(|error| error.message)?;
        Ok(DynamicControllerActionOutcome {
            request_id: request.request_id,
            controller_id: Uuid::nil(),
            changed: true,
        })
    })
    .await
    .map(Json)
}

fn context(session: &super::Session, request_id: &str) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
    .with_request_id(request_id)
}

fn timing(value: light_wire::v2::dynamics::DynamicValueTimingProjection) -> DynamicValueTiming {
    DynamicValueTiming {
        fade_millis: value.fade_millis,
        delay_millis: value.delay_millis,
    }
}

fn validate_request_id(value: &str) -> Result<(), ApiError> {
    if value.is_empty() || value.len() > 128 {
        return Err(ApiError::bad_request(
            "request_id must contain between 1 and 128 bytes",
        ));
    }
    Ok(())
}
