use std::sync::Arc;

use light_application::{
    ActionContext, ActionError, ActionErrorKind, AutomaticPlaybackProjection,
    DynamicPlaybackControllerStatus, DynamicPlaybackRuntimeProjection, DynamicPlaybackRuntimeState,
    DynamicPlaybackSpeedSource, PlaybackDeskProjection, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackShowScope, PlaybackTargetProjection,
};
#[cfg(test)]
use light_core::CueListId;
use light_engine::EngineSnapshot;
use light_playback::{PlaybackAddress, PlaybackTarget};

use super::{ServerPlaybackPorts, invalid, resolve_group_playback};

#[path = "projection/prepared.rs"]
mod prepared;
#[path = "projection/settled_dynamic.rs"]
mod settled_dynamic;
#[path = "projection/targets.rs"]
mod targets;
pub(in crate::runtime) use prepared::prepared_runtime_projections;
pub(in crate::runtime) use settled_dynamic::settled_dynamic_playback_events;
use targets::{cue_list_projection, grand_master_projection, group_projection, speed_projection};

pub(in crate::runtime) fn automatic_changes(
    engine: &light_engine::Engine,
    scope: PlaybackShowScope,
    transitions: Vec<light_playback::AutomaticPlaybackTransition>,
) -> Vec<AutomaticPlaybackProjection> {
    let snapshot = engine.snapshot();
    transitions.into_iter().flat_map(|transition| {
      let mut addresses=snapshot.playback_pages.iter().flat_map(|page|page.assignments.iter().filter_map(move |(position,definition)| {
        matches!(definition.target,PlaybackTarget::CueList{cue_list_id} if cue_list_id==transition.cue_list_id)
          .then(||PlaybackAddress::new(page.number,*position).ok()).flatten()
      })).collect::<Vec<_>>();
      addresses.sort();addresses.dedup();
      let mut changes=addresses.into_iter().map(|address|AutomaticPlaybackProjection {
        projection:cue_list_projection(scope,PlaybackRuntimeIdentity::Playback(address),Some(address),transition.cue_list_id,engine.playback_runtime_status_at(address).as_ref()),transition:transition.clone()
      }).collect::<Vec<_>>();
      let status=engine.playback_runtime_status_for_cue_list(transition.cue_list_id);
      changes.push(AutomaticPlaybackProjection{projection:cue_list_projection(scope,PlaybackRuntimeIdentity::CueList(transition.cue_list_id),None,transition.cue_list_id,status.as_ref()),transition:transition.clone()});
      if let Some(exact)=automatic_source_projection(scope,&transition,status.as_ref()) {if !changes.iter().any(|change|change.projection.identity==exact.projection.identity) {changes.push(exact);}}
      changes
    }).collect()
}
pub(super) fn automatic_source_projection(
    scope: PlaybackShowScope,
    transition: &light_playback::AutomaticPlaybackTransition,
    status: Option<&light_playback::PlaybackRuntimeStatus>,
) -> Option<AutomaticPlaybackProjection> {
    let identity = match transition.playback_address {
        Some(address) => PlaybackRuntimeIdentity::Playback(address),
        None => PlaybackRuntimeIdentity::DirectCueList(transition.cue_list_id),
    };
    // An already accepted transition belongs to its captured owner, even if a newer
    // operation has taken over the shared Cuelist before feedback is projected.
    let status =
        status.filter(|status| status.source.playback_address == transition.playback_address);
    Some(AutomaticPlaybackProjection {
        projection: cue_list_projection(scope, identity, None, transition.cue_list_id, status),
        transition: transition.clone(),
    })
}

pub(super) fn projection(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    identity: PlaybackRuntimeIdentity,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    let snapshot = ports.state.output.snapshot();
    let scope = show_scope(ports, &snapshot);
    match identity {
        PlaybackRuntimeIdentity::Playback(number) => {
            project_playback(ports, &snapshot, scope, identity, number)
        }
        PlaybackRuntimeIdentity::CueList(cue_list_id) => Ok(cue_list_projection(
            scope,
            identity,
            None,
            cue_list_id,
            ports
                .state
                .output
                .playback_runtime_status_for_cue_list(cue_list_id)
                .as_ref(),
        )),
        PlaybackRuntimeIdentity::DirectCueList(cue_list_id) => Ok(cue_list_projection(
            scope,
            identity,
            None,
            cue_list_id,
            ports
                .state
                .output
                .playback_runtime_status_for_direct_cue_list(cue_list_id)
                .as_ref(),
        )),
        PlaybackRuntimeIdentity::Group(ref group_id) => {
            let group_id = group_id.clone();
            project_group(ports, &snapshot, scope, identity, group_id)
        }
    }
}

pub(super) fn projections(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    identities: &[PlaybackRuntimeIdentity],
) -> Result<Vec<PlaybackRuntimeProjection>, ActionError> {
    let snapshot = ports.state.output.snapshot();
    let scope = show_scope(ports, &snapshot);
    let mut result = Vec::with_capacity(identities.len());
    for identity in identities {
        project_identity(ports, &snapshot, scope, identity.clone(), &mut result)?;
    }
    Ok(result)
}

pub(super) fn desk_projection(
    ports: &ServerPlaybackPorts<'_>,
    context: &ActionContext,
) -> Result<Option<PlaybackDeskProjection>, ActionError> {
    if context.desk_id.is_nil() {
        return Ok(None);
    }
    let snapshot = ports.state.output.snapshot();
    let Some(show) = ports.state.active_show.current().clone() else {
        // Test-bench and startup compatibility paths may operate a prepared in-memory runtime
        // before its show index entry is installed. Nil makes that transient scope explicit.
        return Ok(Some(PlaybackDeskProjection {
            scope: PlaybackShowScope {
                show_id: uuid::Uuid::nil(),
                show_revision: snapshot.revision,
            },
            desk_id: context.desk_id,
            command_page: 1,
            selected_surface_id: "main".into(),
            resolved_pages: std::collections::BTreeMap::from([("main".into(), 1)]),
            selected_playback: None,
            selected_cue_list: None,
        }));
    };
    let active_page = ports
        .state
        .installation
        .command_page(context.desk_id)
        .map_err(|error| invalid(error.to_string()))?;
    let selected_playback = ports
        .state
        .installation
        .selected_playback(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?
        .map(|selection| {
            light_playback::PlaybackAddress::new(selection.page, selection.position)
                .map_err(invalid)
        })
        .transpose()?;
    let surfaces =
        crate::runtime::playback_surfaces_http::surface_snapshot(ports.state, context.desk_id)
            .map_err(super::api_action_error)?;
    Ok(Some(PlaybackDeskProjection {
        scope: PlaybackShowScope {
            show_id: show.id.0,
            show_revision: snapshot.revision,
        },
        desk_id: context.desk_id,
        command_page: active_page,
        selected_surface_id: surfaces.selected_surface_id,
        resolved_pages: surfaces.resolved_pages,
        selected_playback,
        selected_cue_list: ports
            .state
            .installation
            .selected_cue_list(context.desk_id, show.id)
            .map_err(|error| invalid(error.to_string()))?,
    }))
}

fn project_identity(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &Arc<EngineSnapshot>,
    scope: PlaybackShowScope,
    identity: PlaybackRuntimeIdentity,
    result: &mut Vec<PlaybackRuntimeProjection>,
) -> Result<(), ActionError> {
    match identity {
        PlaybackRuntimeIdentity::Playback(number) => {
            result.push(project_playback(ports, snapshot, scope, identity, number)?);
        }
        PlaybackRuntimeIdentity::CueList(cue_list_id) => {
            let status = ports
                .state
                .output
                .playback_runtime_status_for_cue_list(cue_list_id);
            result.push(cue_list_projection(
                scope,
                identity,
                None,
                cue_list_id,
                status.as_ref(),
            ));
        }
        PlaybackRuntimeIdentity::DirectCueList(cue_list_id) => {
            let status = ports
                .state
                .output
                .playback_runtime_status_for_direct_cue_list(cue_list_id);
            result.push(cue_list_projection(
                scope,
                identity,
                None,
                cue_list_id,
                status.as_ref(),
            ));
        }
        PlaybackRuntimeIdentity::Group(ref group_id) => {
            let group_id = group_id.clone();
            result.push(project_group(ports, snapshot, scope, identity, group_id)?);
        }
    }
    Ok(())
}

fn project_playback(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &EngineSnapshot,
    scope: PlaybackShowScope,
    requested: PlaybackRuntimeIdentity,
    address: light_playback::PlaybackAddress,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    let definition = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())
        .and_then(|page| page.assignments.get(&address.position()));
    let Some(definition) = definition else {
        return Ok(PlaybackRuntimeProjection {
            scope,
            identity: requested,

            target: PlaybackTargetProjection::Missing,
        });
    };
    let target = match &definition.target {
        PlaybackTarget::CueList { cue_list_id } => {
            let status = ports.state.output.playback_runtime_status_at(address);
            return Ok(cue_list_projection(
                scope,
                requested,
                Some(address),
                *cue_list_id,
                status.as_ref(),
            ));
        }
        PlaybackTarget::Dynamic { assignment } => PlaybackTargetProjection::Dynamic {
            dynamic_id: assignment.dynamic.dynamic_id,
            last_known_pool_number: assignment.dynamic.last_known_pool_number,
            embedded: assignment.dynamic.dynamic_id.is_none(),
            runtime: ports
                .state
                .output
                .active_dynamic_playback(address)
                .map(|active| {
                    Box::new(dynamic_runtime_projection(
                        ports, snapshot, assignment, active,
                    ))
                }),
        },
        PlaybackTarget::Macro { macro_id } => PlaybackTargetProjection::Macro {
            macro_id: *macro_id,
        },
        PlaybackTarget::Timecode { timecode_id } => PlaybackTargetProjection::Timecode {
            timecode_id: *timecode_id,
        },
        PlaybackTarget::Group { group_id, .. } => {
            group_projection(ports, snapshot, group_id, Some(address))?
        }
        PlaybackTarget::SpeedGroup { group } => speed_projection(ports, group)?,
        PlaybackTarget::GrandMaster => grand_master_projection(ports),
        PlaybackTarget::ProgrammerFade => PlaybackTargetProjection::ProgrammerFade {
            millis: ports
                .state
                .installation
                .configuration()
                .programmer_fade_millis,
        },
        PlaybackTarget::CueFade => PlaybackTargetProjection::CueFade {
            millis: ports
                .state
                .installation
                .configuration()
                .sequence_master_fade_millis,
        },
    };
    Ok(PlaybackRuntimeProjection {
        scope,
        identity: requested,

        target,
    })
}

fn project_group(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &EngineSnapshot,
    scope: PlaybackShowScope,
    requested: PlaybackRuntimeIdentity,
    group_id: light_application::PlaybackGroupId,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    // A stale or forged Group->Playback assignment must not fail the whole runtime snapshot: the
    // Group stays visible and simply projects with no assigned Playback until the operator
    // reassigns it. Operator actions still reject the stale assignment through the action port.
    let playback_address = match resolve_group_playback(snapshot, group_id.as_str()) {
        Ok(number) => number,
        Err(error) if error.kind == ActionErrorKind::Conflict => None,
        // A Group absent from the active show (deleted, or named by a previous show's layout)
        // is reported per item so the rest of the snapshot still hydrates.
        Err(error) if error.kind == ActionErrorKind::NotFound => {
            return Ok(PlaybackRuntimeProjection {
                scope,
                identity: requested,
                target: PlaybackTargetProjection::Missing,
            });
        }
        Err(error) => return Err(error),
    };
    Ok(PlaybackRuntimeProjection {
        scope,
        identity: requested,
        target: group_projection(ports, snapshot, group_id.as_str(), playback_address)?,
    })
}

fn dynamic_runtime_projection(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &EngineSnapshot,
    assignment: &light_playback::DynamicPlaybackAssignment,
    active: light_playback::ActiveDynamicPlayback,
) -> DynamicPlaybackRuntimeProjection {
    let runtime = ports.state.output.dynamic_runtime_snapshot();
    let output_interval_millis =
        1_000_u64.div_ceil(u64::from(ports.state.output.frame_rate_hz().max(1)));
    dynamic_runtime_projection_from_snapshot(
        snapshot,
        assignment,
        active,
        &runtime,
        output_interval_millis,
    )
}

fn dynamic_playback_state(
    active: &light_playback::ActiveDynamicPlayback,
    stored_instance: Option<&light_dynamics::DynamicInstanceSnapshot>,
    instance: Option<&light_dynamics::DynamicInstanceSnapshot>,
    controller_status: DynamicPlaybackControllerStatus,
) -> DynamicPlaybackRuntimeState {
    if !active.enabled || stored_instance.is_some_and(|instance| instance.completed) {
        DynamicPlaybackRuntimeState::Off
    } else if matches!(
        active.start_state.outcome,
        light_playback::DynamicPlaybackStartOutcome::Rejected(_)
    ) {
        DynamicPlaybackRuntimeState::Failed
    } else if matches!(
        active.start_state.outcome,
        light_playback::DynamicPlaybackStartOutcome::Passive(_)
    ) {
        DynamicPlaybackRuntimeState::Pending
    } else if instance.is_none() {
        match &active.start_state.outcome {
            light_playback::DynamicPlaybackStartOutcome::Pending
            | light_playback::DynamicPlaybackStartOutcome::Passive(_) => {
                DynamicPlaybackRuntimeState::Pending
            }
            light_playback::DynamicPlaybackStartOutcome::Accepted
            | light_playback::DynamicPlaybackStartOutcome::Rejected(_) => {
                DynamicPlaybackRuntimeState::Failed
            }
        }
    } else if active.fader_value <= f32::EPSILON || active.master <= f32::EPSILON {
        DynamicPlaybackRuntimeState::Zero
    } else if instance.is_some_and(|instance| instance.pending_until_millis.is_some()) {
        DynamicPlaybackRuntimeState::Pending
    } else if active.paused
        || instance.is_some_and(|instance| {
            instance.paused_at_millis.is_some() || instance.speed_paused_at_millis.is_some()
        })
    {
        DynamicPlaybackRuntimeState::Paused
    } else if controller_status == DynamicPlaybackControllerStatus::Losing {
        DynamicPlaybackRuntimeState::Hidden
    } else {
        DynamicPlaybackRuntimeState::Active
    }
}

fn dynamic_runtime_projection_from_snapshot(
    snapshot: &EngineSnapshot,
    assignment: &light_playback::DynamicPlaybackAssignment,
    active: light_playback::ActiveDynamicPlayback,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    output_interval_millis: u64,
) -> DynamicPlaybackRuntimeProjection {
    let definition = assignment
        .dynamic
        .dynamic_id
        .and_then(|id| snapshot.dynamics.iter().find(|dynamic| dynamic.id == id))
        .unwrap_or(&assignment.dynamic.embedded_fallback.definition);
    let controller_id = light_playback::dynamic_playback_controller_id(assignment.target_id());
    let stored_instance = runtime.instances.iter().find(|instance| {
        instance
            .controllers
            .iter()
            .any(|controller| controller.id == controller_id)
    });
    let instance = stored_instance.filter(|instance| !instance.completed);
    let winning_controller_id = instance.and_then(|instance| {
        instance
            .controllers
            .iter()
            .max_by_key(|controller| {
                (
                    controller.priority,
                    controller.activated_at_millis,
                    controller.id,
                )
            })
            .map(|controller| controller.id)
    });
    let controller_status = match winning_controller_id {
        Some(winner) if winner == controller_id => DynamicPlaybackControllerStatus::Winning,
        Some(_) => DynamicPlaybackControllerStatus::Losing,
        None => DynamicPlaybackControllerStatus::Missing,
    };
    let state = dynamic_playback_state(&active, stored_instance, instance, controller_status);
    let effective_speed_multiplier = instance
        .and_then(|instance| {
            instance
                .controllers
                .iter()
                .find(|controller| controller.id == controller_id)
        })
        .map_or_else(
            || {
                (active.local_speed_multiplier.factor()
                    * definition.overall_speed_multiplier.factor()) as f32
            },
            |controller| {
                (f64::from(controller.speed_multiplier)
                    * definition.overall_speed_multiplier.factor()) as f32
            },
        );
    let (speed_source, effective_duration_millis) = match definition.speed {
        light_dynamics::DynamicSpeed::Fixed { duration_millis } => (
            DynamicPlaybackSpeedSource::Fixed,
            Some(
                (duration_millis as f64 / f64::from(effective_speed_multiplier).max(f64::EPSILON))
                    .round()
                    .max(1.0) as u64,
            ),
        ),
        light_dynamics::DynamicSpeed::SpeedGroup { .. } => {
            (DynamicPlaybackSpeedSource::SpeedGroup, None)
        }
    };
    let targets = instance.map_or(&[][..], |instance| instance.targets.as_slice());
    let coverage = dynamic_target_lane_coverage(snapshot, definition, targets);
    let mut warnings = Vec::new();
    if state == DynamicPlaybackRuntimeState::Failed {
        warnings.push(match &active.start_state.outcome {
            light_playback::DynamicPlaybackStartOutcome::Rejected(message) => message.clone(),
            _ => "Dynamic Playback controller disappeared after an accepted start".to_owned(),
        });
    } else if active.enabled
        && let light_playback::DynamicPlaybackStartOutcome::Passive(message) =
            &active.start_state.outcome
    {
        warnings.push(message.clone());
    }
    if coverage.missing_target_count > 0 || coverage.skipped_address_count > 0 {
        warnings.push(format!(
            "{} of {} target/lane addresses run; {} skipped ({} missing targets, {} unpatched targets)",
            coverage.supported_address_count,
            coverage.total_address_count,
            coverage.skipped_address_count,
            coverage.missing_target_count,
            coverage.unpatched_target_count,
        ));
    }
    if let Some(aliasing) = effective_duration_millis.and_then(|cycle| {
        light_dynamics::aliasing_warning(definition, cycle, output_interval_millis).map(|warning| {
            format!(
                "Aliasing: shortest segment is {} ms ({} output samples; at least 4 required)",
                warning.shortest_segment_millis, warning.samples_per_segment,
            )
        })
    }) {
        warnings.push(aliasing);
    }
    let warning = (!warnings.is_empty()).then(|| warnings.join(" · "));
    DynamicPlaybackRuntimeProjection {
        active,
        state,
        instance_id: instance.map(|instance| instance.id),
        controller_id,
        winning_controller_id,
        controller_status,
        target_count: targets.len(),
        compatible_target_count: coverage.compatible_target_count,
        missing_target_count: coverage.missing_target_count,
        unpatched_target_count: coverage.unpatched_target_count,
        lane_count: definition.lanes.len(),
        supported_address_count: coverage.supported_address_count,
        skipped_address_count: coverage.skipped_address_count,
        speed_source,
        effective_speed_multiplier,
        effective_duration_millis,
        warning,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct DynamicTargetLaneCoverage {
    pub(in crate::runtime) compatible_target_count: usize,
    pub(in crate::runtime) missing_target_count: usize,
    pub(in crate::runtime) unpatched_target_count: usize,
    pub(in crate::runtime) supported_address_count: usize,
    pub(in crate::runtime) skipped_address_count: usize,
    pub(in crate::runtime) total_address_count: usize,
}

pub(in crate::runtime) fn dynamic_target_lane_coverage(
    snapshot: &EngineSnapshot,
    definition: &light_dynamics::DynamicDefinition,
    targets: &[light_core::FixtureId],
) -> DynamicTargetLaneCoverage {
    let mut coverage = DynamicTargetLaneCoverage {
        total_address_count: targets.len().saturating_mul(definition.lanes.len()),
        ..DynamicTargetLaneCoverage::default()
    };
    for target in targets {
        let Some((fixture, head_index)) = snapshot.fixtures.iter().find_map(|fixture| {
            if fixture.fixture_id == *target {
                return Some((fixture, None));
            }
            fixture
                .logical_heads
                .iter()
                .find(|head| head.fixture_id == *target)
                .map(|head| (fixture, Some(head.head_index)))
        }) else {
            coverage.missing_target_count += 1;
            coverage.skipped_address_count += definition.lanes.len();
            continue;
        };
        let has_physical_patch = (fixture.universe.is_some() && fixture.address.is_some())
            || fixture
                .split_patches
                .iter()
                .any(|patch| patch.universe.is_some() && patch.address.is_some());
        if !has_physical_patch {
            coverage.unpatched_target_count += 1;
        }
        let supported = definition
            .lanes
            .iter()
            .filter(|lane| {
                fixture
                    .definition
                    .heads
                    .iter()
                    .filter(|head| head_index.is_none_or(|index| head.index == index))
                    .flat_map(|head| &head.parameters)
                    .any(|parameter| lane.drives_attribute(&parameter.attribute))
            })
            .count();
        coverage.supported_address_count += supported;
        coverage.skipped_address_count += definition.lanes.len().saturating_sub(supported);
        if supported > 0 {
            coverage.compatible_target_count += 1;
        }
    }
    coverage
}

fn show_scope(ports: &ServerPlaybackPorts<'_>, snapshot: &EngineSnapshot) -> PlaybackShowScope {
    let show_id = ports
        .state
        .active_show
        .current()
        .as_ref()
        .map(|show| show.id.0)
        .unwrap_or_else(uuid::Uuid::nil);
    PlaybackShowScope {
        show_id,
        show_revision: snapshot.revision,
    }
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
