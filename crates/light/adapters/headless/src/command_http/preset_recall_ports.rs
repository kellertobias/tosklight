use light_application::{
    ActionContext, ActionError, ActionErrorKind, ProgrammingPorts,
    ProgrammingPresetRecallEnvironment, ProgrammingPresetRecallPorts,
    ProgrammingPresetRecallRequest,
};
use light_programmer::{Preset, PresetAddress};
use light_show::{PortableShowDocument, PortableShowObject};
use std::{collections::HashMap, sync::Arc};

use super::programming_ports::ServerProgrammingPorts;
use crate::runtime::ActiveShowRepository;

impl ProgrammingPresetRecallPorts for ServerProgrammingPorts<'_> {
    fn authorize_preset_recall(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize_programming_change(self, context)
    }

    fn preset_recall_environment(
        &self,
        _context: &ActionContext,
        request: &ProgrammingPresetRecallRequest,
    ) -> Result<ProgrammingPresetRecallEnvironment, ActionError> {
        environment(self, request)
    }

    fn persist_preset_recall(
        &self,
        context: &ActionContext,
        operation: &'static str,
    ) -> Option<String> {
        <Self as ProgrammingPorts>::persist(self, context, operation)
    }
}

fn environment(
    ports: &ServerProgrammingPorts<'_>,
    request: &ProgrammingPresetRecallRequest,
) -> Result<ProgrammingPresetRecallEnvironment, ActionError> {
    let active = ports
        .state()
        .active_show
        .current()
        .clone()
        .ok_or_else(|| not_found("no active Show is loaded"))?;
    // Show recovery: the failed show's Presets are not recalled.
    ports.state().active_show.ensure_content_writable()?;
    if active.id != request.show_id {
        return Err(conflict("the requested Show is not active"));
    }
    let document = ActiveShowRepository::open(&active.path)
        .and_then(|store| store.portable_document())
        .map_err(|error| internal(format!("failed to load the active Show: {error}")))?;
    if document.id() != request.show_id {
        return Err(conflict(
            "the active Show authority changed during Preset recall",
        ));
    }
    let (object, preset) = resolve_preset(&document, request.address)?;
    // Preset and Group expansion deliberately share this one portable document. The activation
    // lock keeps it current through the mutation, while deriving Groups here prevents a stale or
    // independently replaced engine snapshot from changing recall membership.
    let groups = decode_groups(&document)?;
    // The activation lock protects this revision comparison. Reuse only the projection
    // of this captured document; recovery/stale generations compile the captured data instead.
    let snapshot = ports.state().output.snapshot();
    let spatial_inputs =
        spatial_inputs_for_revision(&snapshot, document.revision().value(), || {
            let transaction = document.transaction();
            let candidate = document.candidate(&transaction).map_err(|error| {
                conflict(format!("stored Show spatial inputs are invalid: {error}"))
            })?;
            light_application::show_compiler::compile_show_spatial_positions(candidate)
        })?;
    let stage_positions = spatial_inputs
        .iter()
        .map(|(&id, point)| {
            (
                id,
                light_dynamics::Position3d {
                    x: f64::from(point.x),
                    y: f64::from(point.y),
                    z: f64::from(point.z),
                },
            )
        })
        .collect();
    let (selectable_targets, target_expansions) = selectable_targets(ports);
    // Resolve/validate an explicit Aim before the service's empty-selection branch. Both
    // HTTP and WebSocket consume this same captured application environment.
    let resolved_aim = preset
        .aim_at_fixture_number
        .map(|_| {
            super::super::programmer_aim_command::resolve_aim_preset(ports.state(), &preset)
                .map(Arc::new)
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error))
        })
        .transpose()?;
    Ok(ProgrammingPresetRecallEnvironment {
        supported_programming_contract: ports.state().output.supported_programming_contract(),
        show_id: document.id(),
        show_revision: document.revision(),
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
        address: request.address,
        raw_body: Arc::new(object.body().clone()),
        preset: Arc::new(preset),
        resolved_aim,
        groups: Arc::new(groups),
        stage_positions: Arc::new(stage_positions),
        selectable_targets: Arc::new(selectable_targets),
        target_expansions: Arc::new(target_expansions),
        programmer_fade_millis: ports
            .state()
            .installation
            .configuration()
            .programmer_fade_millis,
    })
}

fn selectable_targets(
    ports: &ServerProgrammingPorts<'_>,
) -> (
    Vec<light_core::FixtureId>,
    HashMap<light_core::FixtureId, Vec<light_core::FixtureId>>,
) {
    let mut targets = Vec::new();
    let mut expansions = HashMap::new();
    for fixture in ports.state().output.snapshot().fixtures.iter() {
        let selectable = super::super::selectable_fixture_ids(fixture);
        targets.extend(selectable.iter().copied());
        expansions.insert(fixture.fixture_id, selectable);
        for head in &fixture.logical_heads {
            expansions.insert(head.fixture_id, vec![head.fixture_id]);
        }
    }
    let mut seen = std::collections::HashSet::new();
    targets.retain(|target| seen.insert(*target));
    (targets, expansions)
}

fn decode_groups(
    document: &PortableShowDocument,
) -> Result<HashMap<String, light_programmer::GroupDefinition>, ActionError> {
    document
        .objects_of_kind("group")
        .map(|object| {
            let mut group =
                serde_json::from_value::<light_programmer::GroupDefinition>(object.body().clone())
                    .map_err(|error| {
                        conflict(format!(
                            "stored Group {} is invalid: {error}",
                            object.key().id()
                        ))
                    })?;
            group.id = object.key().id().to_owned();
            Ok((group.id.clone(), group))
        })
        .collect()
}

fn resolve_preset(
    document: &PortableShowDocument,
    address: PresetAddress,
) -> Result<(&PortableShowObject, Preset), ActionError> {
    let storage_key = address.storage_key();
    if let Some(object) = document.object("preset", &storage_key) {
        return decode_exact(object, address);
    }
    let mut matches = document
        .objects_of_kind("preset")
        .filter_map(|object| decode(object).ok().map(|decoded| (object, decoded)))
        .filter(|(_, (stored, _))| *stored == address);
    let Some((object, (_, preset))) = matches.next() else {
        return Err(not_found("Preset does not exist"));
    };
    if matches.next().is_some() {
        return Err(conflict(
            "multiple legacy Presets resolve to the requested address",
        ));
    }
    Ok((object, preset))
}

fn decode_exact(
    object: &PortableShowObject,
    requested: PresetAddress,
) -> Result<(&PortableShowObject, Preset), ActionError> {
    let (stored, preset) = decode(object)?;
    if stored != requested {
        return Err(conflict(
            "stored Preset address does not match the requested pool entry",
        ));
    }
    Ok((object, preset))
}

fn decode(object: &PortableShowObject) -> Result<(PresetAddress, Preset), ActionError> {
    let mut preset = serde_json::from_value::<Preset>(object.body().clone())
        .map_err(|error| conflict(format!("stored Preset is invalid: {error}")))?;
    let address = preset
        .reconcile_address(object.key().id())
        .map_err(conflict)?;
    Ok((address, preset))
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}

fn internal(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Internal, message)
}

/// A matching document revision makes profile/geometry recompilation unnecessary on recall.
fn spatial_inputs_for_revision(
    snapshot: &light_engine::EngineSnapshot,
    document_revision: u64,
    fallback: impl FnOnce() -> Result<
        HashMap<light_core::FixtureId, light_dynamics::SpatialPosition>,
        ActionError,
    >,
) -> Result<Arc<HashMap<light_core::FixtureId, light_dynamics::SpatialPosition>>, ActionError> {
    if snapshot.revision == document_revision {
        Ok(Arc::clone(&snapshot.dynamic_stage_positions))
    } else {
        fallback().map(Arc::new)
    }
}

#[cfg(test)]
mod spatial_revision_tests {
    use super::*;
    use light_core::FixtureId;
    use light_dynamics::SpatialPosition;

    #[test]
    fn matching_captured_revision_reuses_spatial_projection_without_compiling_profiles() {
        let legacy = FixtureId(uuid::Uuid::from_u128(804));
        let snapshot = light_engine::EngineSnapshot {
            revision: 42,
            dynamic_stage_positions: Arc::new(HashMap::from([(
                legacy,
                SpatialPosition {
                    x: 1.,
                    y: 2.,
                    z: 3.,
                },
            )])),
            ..Default::default()
        };
        let result = spatial_inputs_for_revision(&snapshot, 42, || {
            panic!("matching recall must not compile Patch profiles")
        })
        .unwrap();
        assert!(Arc::ptr_eq(&result, &snapshot.dynamic_stage_positions));
        assert_eq!(
            result[&legacy],
            SpatialPosition {
                x: 1.,
                y: 2.,
                z: 3.
            }
        );
    }

    #[test]
    fn stale_snapshot_uses_only_captured_document_spatial_inputs_and_retains_errors() {
        let id = FixtureId(uuid::Uuid::from_u128(804));
        let snapshot = light_engine::EngineSnapshot {
            revision: 41,
            dynamic_stage_positions: Arc::new(HashMap::from([(
                id,
                SpatialPosition {
                    x: 99.,
                    y: 0.,
                    z: 0.,
                },
            )])),
            ..Default::default()
        };
        let result = spatial_inputs_for_revision(&snapshot, 42, || {
            Ok(HashMap::from([(
                id,
                SpatialPosition {
                    x: -1.,
                    y: 0.,
                    z: 3.,
                },
            )]))
        })
        .unwrap();
        assert_eq!(
            result[&id],
            SpatialPosition {
                x: -1.,
                y: 0.,
                z: 3.
            }
        );
        let error = spatial_inputs_for_revision(&snapshot, 42, || {
            Err(conflict("captured document invalid"))
        })
        .unwrap_err();
        assert_eq!(error.message, "captured document invalid");
    }
}
