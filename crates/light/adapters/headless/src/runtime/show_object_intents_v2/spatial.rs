//! Spatial mapping preview and evaluation context for Dynamic show-object intents.

use super::*;

pub(super) async fn dynamic_spatial_preview(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<light_wire::v2::dynamics::DynamicSpatialPreviewRequest>,
) -> Result<Json<light_wire::v2::dynamics::DynamicSpatialPreviewResponse>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let _activation = state.active_show.acquire().await;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, object) = store
        .object_with_portable_revision("dynamic", &id.to_string())
        .map_err(ApiError::store)?;
    if show_revision.value() != request.expected_show_revision {
        return Err(ApiError::conflict(format!(
            "active Show revision conflict: expected {}, current {}",
            request.expected_show_revision,
            show_revision.value()
        )));
    }
    let object = object.ok_or_else(|| ApiError::not_found("Dynamic does not exist"))?;
    if object.revision != request.expected_dynamic_revision {
        return Err(ApiError::conflict(format!(
            "Dynamic revision conflict: expected {}, current {}",
            request.expected_dynamic_revision, object.revision
        )));
    }
    let definition = decode_dynamic(object.body)?;
    let draft = decode_spatial_mapping(request.spatial_mapping.clone())?;
    let snapshot = state.output.snapshot();
    let context = dynamic_spatial_context(&snapshot, &definition)?;
    let ranked = light_dynamics::evaluate_dynamic_spatial_mapping(
        context.inherited_mapping.as_ref(),
        &draft,
        &context.targets,
        None,
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let ranks = ranked
        .ordered_fixture_ids
        .iter()
        .map(
            |fixture_id| light_wire::v2::group_management::GroupSpatialRankProjection {
                fixture_id: fixture_id.0,
                rank: ranked.rank_by_fixture[fixture_id],
            },
        )
        .collect();
    // The plane the ranking actually happens on, so the Phase view plots what ranks rather than
    // a top-down picture that agrees with it only when the projection happens to look down.
    let projected_positions =
        effective_spatial_projection(&draft, context.inherited_mapping.as_ref())
            .map(|projection| {
                light_dynamics::project_spatial_positions(&projection, &context.targets)
                    .map(|positions| {
                        positions
                            .into_iter()
                            .map(|position| {
                                light_wire::v2::group_management::GroupProjectedPositionProjection {
                                    fixture_id: position.fixture_id.0,
                                    u: position.u,
                                    v: position.v,
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .map_err(|error| ApiError::bad_request(error.to_string()))
            })
            .transpose()?
            .unwrap_or_default();
    Ok(Json(
        light_wire::v2::dynamics::DynamicSpatialPreviewResponse {
            show_id: show_id.0,
            show_revision: show_revision.value(),
            dynamic_id: id,
            dynamic_revision: object.revision,
            target_binding: dynamic_target_binding_projection(&definition.target_binding),
            base: context.base,
            inherited_mapping: context
                .inherited_mapping
                .map(wire_group_spatial_mapping)
                .transpose()?,
            draft: request.spatial_mapping,
            source_order: context
                .source_order
                .iter()
                .map(|fixture_id| fixture_id.0)
                .collect(),
            ordered_fixture_ids: ranked
                .ordered_fixture_ids
                .iter()
                .map(|fixture_id| fixture_id.0)
                .collect(),
            projected_positions,
            ranks,
            rank_count: ranked.rank_count,
            warnings: ranked
                .warnings
                .into_iter()
                .map(wire_spatial_warning)
                .collect(),
        },
    ))
}

pub(super) struct DynamicSpatialContext {
    source_order: Vec<light_core::FixtureId>,
    pub(super) targets: Vec<light_dynamics::SpatialTarget>,
    pub(super) inherited_mapping: Option<light_dynamics::SpatialSelectionMapping>,
    base: light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection,
}

/// The projection the draft actually ranks by: its own override, or the Group's while inheriting.
/// Mirrors `evaluate_dynamic_spatial_mapping`, which resolves the same two stages.
fn effective_spatial_projection(
    draft: &light_dynamics::DynamicSpatialMappingOverride,
    inherited: Option<&light_dynamics::SpatialSelectionMapping>,
) -> Option<light_dynamics::SpatialProjection> {
    match &draft.projection {
        light_dynamics::OverrideStage::Inherit => {
            inherited.map(|mapping| mapping.projection.clone())
        }
        light_dynamics::OverrideStage::Replace(projection) => Some(projection.clone()),
    }
}

pub(super) fn dynamic_spatial_context(
    snapshot: &light_engine::EngineSnapshot,
    definition: &light_dynamics::DynamicDefinition,
) -> Result<DynamicSpatialContext, ApiError> {
    use light_dynamics::DynamicTargetBinding;
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(fixture_id, position)| {
            (
                *fixture_id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let target = |fixture_id: light_core::FixtureId| light_dynamics::SpatialTarget {
        fixture_id,
        position: positions.get(&fixture_id).copied(),
    };
    match &definition.target_binding {
        DynamicTargetBinding::LiveGroup { group_id } => {
            let groups = snapshot
                .groups
                .iter()
                .cloned()
                .map(|group| (group.id.clone(), group))
                .collect();
            let resolved = light_programmer::resolve_group_spatial(group_id, &groups, &positions)
                .map_err(ApiError::bad_request)?;
            let source_order = resolved.source_order;
            let targets = source_order.iter().copied().map(target).collect();
            Ok(DynamicSpatialContext {
                source_order,
                targets,
                inherited_mapping: resolved.effective_mapping,
                base: light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection::LiveGroup {
                    group_id: group_id.clone(),
                    mapping_provenance: wire_group_mapping_provenance(resolved.mapping_provenance),
                },
            })
        }
        DynamicTargetBinding::FrozenTargets { targets } => {
            let source_order = targets.clone();
            let targets = source_order.iter().copied().map(target).collect();
            Ok(DynamicSpatialContext {
                source_order,
                targets,
                inherited_mapping: None,
                base:
                    light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection::FrozenTargets {},
            })
        }
        DynamicTargetBinding::Targetless => Ok(DynamicSpatialContext {
            source_order: Vec::new(),
            targets: Vec::new(),
            inherited_mapping: None,
            base: light_wire::v2::dynamics::DynamicSpatialPreviewBaseProjection::Targetless {},
        }),
    }
}

pub(super) fn decode_spatial_mapping(
    value: light_wire::v2::dynamics::DynamicSpatialMappingOverrideProjection,
) -> Result<light_dynamics::DynamicSpatialMappingOverride, ApiError> {
    serde_json::to_value(value)
        .map_err(|error| ApiError::internal(error.to_string()))
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| {
                ApiError::bad_request(format!("invalid Dynamic spatial mapping: {error}"))
            })
        })
}

fn dynamic_target_binding_projection(
    value: &light_dynamics::DynamicTargetBinding,
) -> light_wire::v2::dynamics::DynamicTargetBindingProjection {
    match value {
        light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
            light_wire::v2::dynamics::DynamicTargetBindingProjection::LiveGroup {
                group_id: group_id.clone(),
            }
        }
        light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
            light_wire::v2::dynamics::DynamicTargetBindingProjection::FrozenTargets {
                targets: targets.iter().map(|fixture_id| fixture_id.0).collect(),
            }
        }
        light_dynamics::DynamicTargetBinding::Targetless => {
            light_wire::v2::dynamics::DynamicTargetBindingProjection::Targetless
        }
    }
}

fn wire_group_spatial_mapping(
    value: light_dynamics::SpatialSelectionMapping,
) -> Result<light_wire::v2::group_management::GroupSpatialSelectionMapping, ApiError> {
    serde_json::to_value(value)
        .map_err(|error| ApiError::internal(error.to_string()))
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| ApiError::internal(error.to_string()))
        })
}

fn wire_group_mapping_provenance(
    value: light_programmer::GroupMappingProvenance,
) -> light_wire::v2::group_management::GroupMappingProvenanceProjection {
    use light_wire::v2::group_management::GroupMappingProvenanceProjection as Wire;
    match value {
        light_programmer::GroupMappingProvenance::None => Wire::None {},
        light_programmer::GroupMappingProvenance::Local { group_id } => Wire::Local { group_id },
        light_programmer::GroupMappingProvenance::Inherited { source_group_ids } => {
            Wire::Inherited { source_group_ids }
        }
        light_programmer::GroupMappingProvenance::MixedSourceMappings => {
            Wire::MixedSourceMappings {}
        }
    }
}

fn wire_spatial_warning(
    value: light_dynamics::SpatialMappingWarning,
) -> light_wire::v2::group_management::GroupSpatialWarningProjection {
    match value {
        light_dynamics::SpatialMappingWarning::MissingPosition { fixture_id } => {
            light_wire::v2::group_management::GroupSpatialWarningProjection::MissingPosition {
                fixture_id: fixture_id.0,
            }
        }
    }
}
