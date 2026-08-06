use light_application as application;
use light_core::Revision;
use light_wire::v2::group_management as wire;

pub(super) fn operation(
    value: wire::GroupManagementOperation,
) -> application::GroupManagementOperation {
    match value {
        wire::GroupManagementOperation::UpdateProperties { properties } => {
            application::GroupManagementOperation::UpdateProperties(
                application::GroupPropertiesUpdate {
                    name: properties.name,
                    color: properties.color,
                    icon: properties.icon,
                },
            )
        }
        wire::GroupManagementOperation::Undo {} => application::GroupManagementOperation::Undo,
        wire::GroupManagementOperation::RefreshFrozen { expected_source } => {
            application::GroupManagementOperation::RefreshFrozen {
                expected_source: expected_source.map(source_expectation),
            }
        }
        wire::GroupManagementOperation::DetachDerived { expected_source } => {
            application::GroupManagementOperation::DetachDerived {
                expected_source: expected_source.map(source_expectation),
            }
        }
        wire::GroupManagementOperation::SetSpatialMapping { mapping } => {
            application::GroupManagementOperation::SetSpatialMapping(spatial_mapping(mapping))
        }
        wire::GroupManagementOperation::RemoveSpatialMapping {} => {
            application::GroupManagementOperation::RemoveSpatialMapping
        }
    }
}

fn spatial_mapping(
    value: wire::GroupSpatialSelectionMapping,
) -> light_dynamics::SpatialSelectionMapping {
    light_dynamics::SpatialSelectionMapping {
        projection: light_dynamics::SpatialProjection {
            anchor: light_dynamics::Position3d {
                x: value.projection.anchor.x,
                y: value.projection.anchor.y,
                z: value.projection.anchor.z,
            },
            view_direction: light_dynamics::Vector3 {
                x: value.projection.view_direction.x,
                y: value.projection.view_direction.y,
                z: value.projection.view_direction.z,
            },
            rotation_degrees: value.projection.rotation_degrees,
            kind: value
                .projection
                .kind
                .map_or(light_dynamics::ProjectionKind::Planar, |kind| match kind {
                    wire::GroupMappingProjectionKind::Planar => {
                        light_dynamics::ProjectionKind::Planar
                    }
                    wire::GroupMappingProjectionKind::Cylindrical => {
                        light_dynamics::ProjectionKind::Cylindrical
                    }
                    wire::GroupMappingProjectionKind::Spherical => {
                        light_dynamics::ProjectionKind::Spherical
                    }
                }),
            axis_rotation: value.projection.axis_rotation.map_or_else(
                light_dynamics::Vector3::default,
                |rotation| light_dynamics::Vector3 {
                    x: rotation.x,
                    y: rotation.y,
                    z: rotation.z,
                },
            ),
            start_angle_degrees: value.projection.start_angle_degrees.unwrap_or(0.0),
            elevation_degrees: value.projection.elevation_degrees.unwrap_or(0.0),
            preset: value.projection.preset.map(|preset| match preset {
                wire::GroupMappingProjectionPreset::Top => light_dynamics::ProjectionPreset::Top,
                wire::GroupMappingProjectionPreset::Front => {
                    light_dynamics::ProjectionPreset::Front
                }
                wire::GroupMappingProjectionPreset::Back => light_dynamics::ProjectionPreset::Back,
                wire::GroupMappingProjectionPreset::Left => light_dynamics::ProjectionPreset::Left,
                wire::GroupMappingProjectionPreset::Right => {
                    light_dynamics::ProjectionPreset::Right
                }
            }),
        },
        shape: match value.shape {
            wire::GroupMappingShape::Grid {
                angle_degrees,
                direction,
            } => light_dynamics::SpatialSelectionShape::Grid {
                angle_degrees,
                direction: match direction {
                    wire::GroupMappingRankDirection::Ascending => {
                        light_dynamics::RankDirection::Ascending
                    }
                    wire::GroupMappingRankDirection::Descending => {
                        light_dynamics::RankDirection::Descending
                    }
                },
            },
            wire::GroupMappingShape::Radial {
                center_u,
                center_v,
                direction,
            } => light_dynamics::SpatialSelectionShape::Radial {
                center_u,
                center_v,
                direction: match direction {
                    wire::GroupMappingRadialDirection::Outward => {
                        light_dynamics::RadialDirection::Outward
                    }
                    wire::GroupMappingRadialDirection::Inward => {
                        light_dynamics::RadialDirection::Inward
                    }
                },
            },
            wire::GroupMappingShape::Radar {
                center_u,
                center_v,
                start_angle_degrees,
                sweep,
            } => light_dynamics::SpatialSelectionShape::Radar {
                center_u,
                center_v,
                start_angle_degrees,
                sweep: match sweep {
                    wire::GroupMappingRadarSweep::Clockwise => {
                        light_dynamics::RadarSweep::Clockwise
                    }
                    wire::GroupMappingRadarSweep::CounterClockwise => {
                        light_dynamics::RadarSweep::CounterClockwise
                    }
                },
            },
        },
    }
}

pub(super) fn resolved_spatial(
    snapshot: &light_engine::EngineSnapshot,
    group_id: &str,
) -> Result<wire::GroupResolvedSpatialProjection, application::ActionError> {
    let groups = snapshot
        .groups
        .iter()
        .cloned()
        .map(|group| (group.id.clone(), group))
        .collect();
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
        .collect();
    let resolved = light_programmer::resolve_group_spatial(group_id, &groups, &positions).map_err(
        |error| application::ActionError::new(application::ActionErrorKind::Invalid, error),
    )?;
    let projected_positions = resolved.effective_mapping.as_ref().map_or_else(
        || {
            Ok(resolved
                .source_order
                .iter()
                .map(|fixture_id| wire::GroupProjectedPositionProjection {
                    fixture_id: fixture_id.0,
                    u: None,
                    v: None,
                })
                .collect())
        },
        |mapping| {
            let targets = resolved
                .source_order
                .iter()
                .map(|fixture_id| light_dynamics::SpatialTarget {
                    fixture_id: *fixture_id,
                    position: positions.get(fixture_id).copied(),
                })
                .collect::<Vec<_>>();
            light_dynamics::project_spatial_positions(&mapping.projection, &targets)
                .map(|positions| {
                    positions
                        .into_iter()
                        .map(|position| wire::GroupProjectedPositionProjection {
                            fixture_id: position.fixture_id.0,
                            u: position.u,
                            v: position.v,
                        })
                        .collect()
                })
                .map_err(|error| {
                    application::ActionError::new(
                        application::ActionErrorKind::Invalid,
                        error.to_string(),
                    )
                })
        },
    )?;
    let ranks = resolved
        .ranked_selection
        .ordered_fixture_ids
        .iter()
        .map(|fixture_id| wire::GroupSpatialRankProjection {
            fixture_id: fixture_id.0,
            rank: resolved.ranked_selection.rank_by_fixture[fixture_id],
        })
        .collect();
    Ok(wire::GroupResolvedSpatialProjection {
        source_order: resolved
            .source_order
            .iter()
            .map(|fixture_id| fixture_id.0)
            .collect(),
        effective_mapping: resolved.effective_mapping.map(wire_spatial_mapping),
        mapping_provenance: match resolved.mapping_provenance {
            light_programmer::GroupMappingProvenance::None => {
                wire::GroupMappingProvenanceProjection::None {}
            }
            light_programmer::GroupMappingProvenance::Local { group_id } => {
                wire::GroupMappingProvenanceProjection::Local { group_id }
            }
            light_programmer::GroupMappingProvenance::Inherited { source_group_ids } => {
                wire::GroupMappingProvenanceProjection::Inherited { source_group_ids }
            }
            light_programmer::GroupMappingProvenance::MixedSourceMappings => {
                wire::GroupMappingProvenanceProjection::MixedSourceMappings {}
            }
        },
        ordered_fixture_ids: resolved
            .ranked_selection
            .ordered_fixture_ids
            .iter()
            .map(|fixture_id| fixture_id.0)
            .collect(),
        projected_positions,
        ranks,
        rank_count: resolved.ranked_selection.rank_count,
        warnings: resolved
            .ranked_selection
            .warnings
            .into_iter()
            .map(|warning| match warning {
                light_dynamics::SpatialMappingWarning::MissingPosition { fixture_id } => {
                    wire::GroupSpatialWarningProjection::MissingPosition {
                        fixture_id: fixture_id.0,
                    }
                }
            })
            .collect(),
    })
}

fn wire_spatial_mapping(
    value: light_dynamics::SpatialSelectionMapping,
) -> wire::GroupSpatialSelectionMapping {
    let is_planar = value.projection.kind == light_dynamics::ProjectionKind::Planar;
    wire::GroupSpatialSelectionMapping {
        projection: wire::GroupMappingProjection {
            anchor: wire::GroupMappingPosition3d {
                x: value.projection.anchor.x,
                y: value.projection.anchor.y,
                z: value.projection.anchor.z,
            },
            view_direction: wire::GroupMappingVector3 {
                x: value.projection.view_direction.x,
                y: value.projection.view_direction.y,
                z: value.projection.view_direction.z,
            },
            rotation_degrees: value.projection.rotation_degrees,
            // A planar projection omits all four, so its payload stays exactly what it was
            // before the other kinds existed.
            kind: match value.projection.kind {
                light_dynamics::ProjectionKind::Planar => None,
                light_dynamics::ProjectionKind::Cylindrical => {
                    Some(wire::GroupMappingProjectionKind::Cylindrical)
                }
                light_dynamics::ProjectionKind::Spherical => {
                    Some(wire::GroupMappingProjectionKind::Spherical)
                }
            },
            axis_rotation: (!is_planar).then(|| wire::GroupMappingVector3 {
                x: value.projection.axis_rotation.x,
                y: value.projection.axis_rotation.y,
                z: value.projection.axis_rotation.z,
            }),
            start_angle_degrees: (!is_planar).then_some(value.projection.start_angle_degrees),
            elevation_degrees: (!is_planar).then_some(value.projection.elevation_degrees),
            preset: value.projection.preset.map(|preset| match preset {
                light_dynamics::ProjectionPreset::Top => wire::GroupMappingProjectionPreset::Top,
                light_dynamics::ProjectionPreset::Front => {
                    wire::GroupMappingProjectionPreset::Front
                }
                light_dynamics::ProjectionPreset::Back => wire::GroupMappingProjectionPreset::Back,
                light_dynamics::ProjectionPreset::Left => wire::GroupMappingProjectionPreset::Left,
                light_dynamics::ProjectionPreset::Right => {
                    wire::GroupMappingProjectionPreset::Right
                }
            }),
        },
        shape: match value.shape {
            light_dynamics::SpatialSelectionShape::Grid {
                angle_degrees,
                direction,
            } => wire::GroupMappingShape::Grid {
                angle_degrees,
                direction: match direction {
                    light_dynamics::RankDirection::Ascending => {
                        wire::GroupMappingRankDirection::Ascending
                    }
                    light_dynamics::RankDirection::Descending => {
                        wire::GroupMappingRankDirection::Descending
                    }
                },
            },
            light_dynamics::SpatialSelectionShape::Radial {
                center_u,
                center_v,
                direction,
            } => wire::GroupMappingShape::Radial {
                center_u,
                center_v,
                direction: match direction {
                    light_dynamics::RadialDirection::Outward => {
                        wire::GroupMappingRadialDirection::Outward
                    }
                    light_dynamics::RadialDirection::Inward => {
                        wire::GroupMappingRadialDirection::Inward
                    }
                },
            },
            light_dynamics::SpatialSelectionShape::Radar {
                center_u,
                center_v,
                start_angle_degrees,
                sweep,
            } => wire::GroupMappingShape::Radar {
                center_u,
                center_v,
                start_angle_degrees,
                sweep: match sweep {
                    light_dynamics::RadarSweep::Clockwise => {
                        wire::GroupMappingRadarSweep::Clockwise
                    }
                    light_dynamics::RadarSweep::CounterClockwise => {
                        wire::GroupMappingRadarSweep::CounterClockwise
                    }
                },
            },
        },
    }
}

fn source_expectation(value: wire::GroupSourceExpectation) -> application::GroupSourceExpectation {
    application::GroupSourceExpectation {
        source_group_id: value.source_group_id,
        expected_source_revision: value.expected_source_revision.map(Revision::from),
    }
}

pub(super) fn outcome(result: application::GroupManagementResult) -> wire::GroupManagementOutcome {
    let application::GroupManagementResult {
        context,
        request_id,
        replayed,
        outcome,
        persistence_warning,
    } = result;
    match outcome {
        application::GroupManagementOutcome::Changed {
            projection,
            show_revision,
            event_sequence,
        } => wire::GroupManagementOutcome::Changed {
            request_id,
            correlation_id: context.correlation_id,
            replayed,
            show_id: projection.show_id.0,
            show_revision: show_revision.value(),
            group: object_projection(&projection),
            show_event_sequence: event_sequence,
            persistence_warning,
        },
        application::GroupManagementOutcome::NoChange {
            projection,
            show_revision,
        } => wire::GroupManagementOutcome::NoChange {
            request_id,
            correlation_id: context.correlation_id,
            replayed,
            show_id: projection.show_id.0,
            show_revision: show_revision.value(),
            group: object_projection(&projection),
            persistence_warning,
        },
    }
}

fn object_projection(
    projection: &application::GroupManagementProjection,
) -> wire::GroupManagementObjectProjection {
    wire::GroupManagementObjectProjection {
        object_id: projection.object_id.clone(),
        object_revision: projection.object_revision,
        body: projection.raw_body.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_exposes_only_supported_management_operations() {
        assert!(matches!(
            operation(wire::GroupManagementOperation::Undo {}),
            application::GroupManagementOperation::Undo
        ));
        assert!(matches!(
            operation(wire::GroupManagementOperation::RefreshFrozen {
                expected_source: None
            }),
            application::GroupManagementOperation::RefreshFrozen {
                expected_source: None
            }
        ));
        assert!(matches!(
            operation(wire::GroupManagementOperation::DetachDerived {
                expected_source: None
            }),
            application::GroupManagementOperation::DetachDerived {
                expected_source: None
            }
        ));
    }

    #[test]
    fn a_declared_source_expectation_is_carried_exactly() {
        let application::GroupManagementOperation::RefreshFrozen {
            expected_source: Some(expectation),
        } = operation(wire::GroupManagementOperation::RefreshFrozen {
            expected_source: Some(wire::GroupSourceExpectation {
                source_group_id: "source".into(),
                expected_source_revision: Some(3),
            }),
        })
        else {
            panic!("a declared source expectation must survive translation")
        };
        assert_eq!(expectation.source_group_id, "source");
        assert_eq!(expectation.expected_source_revision, Some(3));
    }
}
