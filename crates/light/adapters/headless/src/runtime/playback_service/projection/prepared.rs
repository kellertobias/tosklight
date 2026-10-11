//! Final GO projections from private candidates and validated, captured ancillary controls.
use super::*;
use light_dynamics::DynamicRuntimeSnapshot;
use light_playback::PlaybackEngine;

/// Build final feedback before publishing the prepared Playback and Dynamic candidates.
///
/// `before` must come from the same immutable show snapshot, inside the caller's ordered
/// Playback operation. Playback batches cannot mutate Group/Speed/Output masters or desk fade
/// settings, so those captured fields remain authoritative. All Playback-owned fields are
/// refreshed from `playback`; Dynamic feedback uses `runtime`, never the Live mutex.
pub(in crate::runtime) fn prepared_runtime_projections(
    snapshot: &EngineSnapshot,
    before: &[(PlaybackRuntimeIdentity, PlaybackRuntimeProjection)],
    playback: &PlaybackEngine,
    runtime: &DynamicRuntimeSnapshot,
    output_interval_millis: u64,
) -> Result<Vec<(PlaybackRuntimeIdentity, PlaybackRuntimeProjection)>, String> {
    let scope = before.first().map(|(_, projection)| projection.scope);
    before
        .iter()
        .map(|(identity, projection)| {
            if projection.identity != *identity
                || Some(projection.scope) != scope
                || projection.scope.show_revision != snapshot.revision
            {
                return Err("prepared Playback projection has mismatched authority".into());
            }
            project_after(
                snapshot,
                projection,
                playback,
                runtime,
                output_interval_millis,
            )
            .map(|projection| (identity.clone(), projection))
        })
        .collect()
}

fn addressed_playback(
    identity: &PlaybackRuntimeIdentity,
) -> Result<Option<light_playback::PlaybackAddress>, String> {
    Ok(match identity {
        PlaybackRuntimeIdentity::Playback(address) => Some(*address),
        _ => None,
    })
}

fn project_after(
    snapshot: &EngineSnapshot,
    before: &PlaybackRuntimeProjection,
    playback: &PlaybackEngine,
    runtime: &DynamicRuntimeSnapshot,
    output_interval_millis: u64,
) -> Result<PlaybackRuntimeProjection, String> {
    let mut after = before.clone();
    match &mut after.target {
        PlaybackTargetProjection::CueList { cue_list_id, .. } => {
            let status = match addressed_playback(&before.identity)? {
                Some(identity) => playback.runtime_status_for_playback(identity),
                None if matches!(before.identity, PlaybackRuntimeIdentity::DirectCueList(_)) => {
                    playback.runtime_status_for_direct_cue_list(*cue_list_id)
                }
                None => playback.runtime_status_for_cue_list(*cue_list_id),
            };
            return Ok(cue_list_projection(
                before.scope,
                before.identity.clone(),
                None,
                *cue_list_id,
                status.as_ref(),
            ));
        }
        PlaybackTargetProjection::Dynamic {
            runtime: projected, ..
        } => {
            let identity = addressed_playback(&before.identity)?
                .ok_or("prepared Dynamic projection has no Playback address")?;
            let assignment = playback
                .dynamic_assignment(identity)
                .ok_or("prepared Dynamic Playback assignment is missing")?;
            *projected = playback.active_dynamic_playback(identity).map(|active| {
                Box::new(dynamic_runtime_projection_from_snapshot(
                    snapshot,
                    assignment,
                    active,
                    runtime,
                    output_interval_millis,
                ))
            });
        }
        PlaybackTargetProjection::Group {
            fader_position,
            fader_pickup_required,
            fader_pickup_target,
            ..
        } => {
            // Match the ordinary projector: Group identities can resolve a physical fader;
            // virtual Group tiles have no assignment-local fader projection.
            let identity = addressed_playback(&before.identity)?;
            let control = identity
                .map(|identity| playback.control_state_at(identity))
                .unwrap_or_default();
            *fader_position = control.fader_position;
            *fader_pickup_required = control.fader_pickup_required;
            *fader_pickup_target = control.fader_pickup_target;
        }
        PlaybackTargetProjection::GrandMaster(control) => {
            control.dynamics_paused = playback.dynamics_paused();
        }
        PlaybackTargetProjection::Missing
        | PlaybackTargetProjection::Macro { .. }
        | PlaybackTargetProjection::Timecode { .. }
        | PlaybackTargetProjection::SpeedGroup { .. }
        | PlaybackTargetProjection::ProgrammerFade { .. }
        | PlaybackTargetProjection::CueFade { .. } => {}
    }
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::CueListId;
    use light_playback::{Cue, CueList, PlaybackAddress, PlaybackDefinition};

    fn address(position: u16) -> PlaybackAddress {
        PlaybackAddress::new(1, position).unwrap()
    }

    fn scope() -> PlaybackShowScope {
        PlaybackShowScope {
            show_id: uuid::Uuid::from_u128(1),
            show_revision: 0,
        }
    }

    fn before(
        identity: PlaybackRuntimeIdentity,
        target: PlaybackTargetProjection,
    ) -> (PlaybackRuntimeIdentity, PlaybackRuntimeProjection) {
        (
            identity.clone(),
            PlaybackRuntimeProjection {
                scope: scope(),
                identity,
                target,
            },
        )
    }

    fn cue_list(id: CueListId) -> CueList {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": "Prepared projection", "priority": 0, "mode": "sequence",
            "looped": false, "cues": [Cue::new("1".parse().unwrap()), Cue::new("2".parse().unwrap())]
        })).unwrap()
    }

    fn assignment(_number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "Prepared assignment",
            "target": {"type": "cue_list", "cue_list_id": cue_list_id}
        }))
        .unwrap()
    }

    #[test]
    fn cue_feedback_uses_prepared_page_qualified_alias_owners() {
        let first = CueListId::new();
        let second = CueListId::new();
        let one = PlaybackAddress::new(2, 1).unwrap();
        let two = PlaybackAddress::new(3, 1).unwrap();
        let mut live = PlaybackEngine::default();
        live.register(cue_list(first)).unwrap();
        live.register(cue_list(second)).unwrap();
        live.register_definition(address(1), assignment(1, first))
            .unwrap();
        live.register_definition(one, assignment(1001, first))
            .unwrap();
        live.register_definition(two, assignment(1301, second))
            .unwrap();
        let identities = [
            (PlaybackRuntimeIdentity::Playback(address(1)), first),
            (PlaybackRuntimeIdentity::Playback(one), first),
            (PlaybackRuntimeIdentity::Playback(two), second),
            (PlaybackRuntimeIdentity::CueList(first), first),
        ];
        let before = identities
            .into_iter()
            .map(|(identity, cue_list_id)| {
                before(
                    identity,
                    PlaybackTargetProjection::CueList {
                        cue_list_id,
                        runtime: None,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut prepared = live.clone();
        prepared.go_playback(address(1)).unwrap();
        prepared.go_playback(address(1)).unwrap();
        prepared.go_playback(two).unwrap();
        let after = prepared_runtime_projections(
            &EngineSnapshot::default(),
            &before,
            &prepared,
            &DynamicRuntimeSnapshot::default(),
            25,
        )
        .unwrap();
        for (index, (_, projection)) in after.iter().enumerate() {
            let expected = if index == 2 { "1" } else { "2" };
            assert_eq!(
                projection.current_cue().unwrap().number.to_string(),
                expected
            );
            assert_eq!(projection.identity, before[index].0);
        }
        assert!(live.runtime_status_for_playback(address(1)).is_none());
        assert!(
            before
                .iter()
                .all(|(_, projection)| projection.cue_list_runtime().is_none())
        );
    }

    #[test]
    fn ancillary_values_are_captured_while_playback_owned_feedback_is_refreshed() {
        let before = vec![
            before(
                PlaybackRuntimeIdentity::Playback(address(1)),
                PlaybackTargetProjection::Group {
                    group_id: "front".into(),
                    master: 0.4,
                    flash_level: 0.8,
                    fader_position: 0.9,
                    fader_pickup_required: true,
                    fader_pickup_target: Some(0.2),
                },
            ),
            before(
                PlaybackRuntimeIdentity::Playback(address(2)),
                PlaybackTargetProjection::GrandMaster(
                    light_application::GrandMasterRuntimeProjection {
                        level: 0.3,
                        effective_level: 1.0,
                        blackout: false,
                        flash_active: true,
                        dynamics_paused: false,
                    },
                ),
            ),
            before(
                PlaybackRuntimeIdentity::Playback(address(3)),
                PlaybackTargetProjection::ProgrammerFade { millis: 432 },
            ),
        ];
        let mut prepared = PlaybackEngine::default();
        prepared.set_dynamics_paused(true);
        let after = prepared_runtime_projections(
            &EngineSnapshot::default(),
            &before,
            &prepared,
            &DynamicRuntimeSnapshot::default(),
            25,
        )
        .unwrap();
        let PlaybackTargetProjection::Group {
            master,
            flash_level,
            fader_position,
            fader_pickup_required,
            fader_pickup_target,
            ..
        } = after[0].1.target
        else {
            panic!("Group projection")
        };
        assert_eq!((master, flash_level), (0.4, 0.8));
        let control = prepared.control_state_at(address(1));
        assert_eq!(
            (fader_position, fader_pickup_required, fader_pickup_target),
            (
                control.fader_position,
                control.fader_pickup_required,
                control.fader_pickup_target
            )
        );
        let PlaybackTargetProjection::GrandMaster(control) = &after[1].1.target else {
            panic!("Grand Master projection")
        };
        assert_eq!(
            (control.level, control.effective_level, control.flash_active),
            (0.3, 1.0, true)
        );
        assert!(control.dynamics_paused);
        assert_eq!(after[2], before[2]);
    }

    #[test]
    fn missing_prepared_dynamic_assignment_and_mismatched_authority_are_precommit_errors() {
        let before = vec![before(
            PlaybackRuntimeIdentity::Playback(address(1)),
            PlaybackTargetProjection::Dynamic {
                dynamic_id: Some(uuid::Uuid::new_v4()),
                last_known_pool_number: 1,
                embedded: false,
                runtime: None,
            },
        )];
        assert!(
            prepared_runtime_projections(
                &EngineSnapshot::default(),
                &before,
                &PlaybackEngine::default(),
                &DynamicRuntimeSnapshot::default(),
                25
            )
            .unwrap_err()
            .contains("assignment is missing")
        );
        let mut wrong_identity = before.clone();
        wrong_identity[0].1.identity = PlaybackRuntimeIdentity::Playback(address(2));
        assert!(
            prepared_runtime_projections(
                &EngineSnapshot::default(),
                &wrong_identity,
                &PlaybackEngine::default(),
                &DynamicRuntimeSnapshot::default(),
                25
            )
            .unwrap_err()
            .contains("authority")
        );
        let snapshot = EngineSnapshot {
            revision: 1,
            ..Default::default()
        };
        assert!(
            prepared_runtime_projections(
                &snapshot,
                &before,
                &PlaybackEngine::default(),
                &DynamicRuntimeSnapshot::default(),
                25
            )
            .unwrap_err()
            .contains("authority")
        );
    }
}
