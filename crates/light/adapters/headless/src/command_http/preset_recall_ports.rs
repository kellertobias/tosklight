use light_application::{
    ActionContext, ActionError, ActionErrorKind, ProgrammingPorts,
    ProgrammingPresetRecallEnvironment, ProgrammingPresetRecallPorts,
    ProgrammingPresetRecallRequest,
};
use light_programmer::{Preset, PresetAddress};
use light_show::{PortableShowDocument, PortableShowObject, ShowStore};
use std::{collections::HashMap, sync::Arc};

use super::programming_ports::ServerProgrammingPorts;

impl ProgrammingPresetRecallPorts for ServerProgrammingPorts<'_> {
    fn authorize_preset_recall(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
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
        .read()
        .clone()
        .ok_or_else(|| not_found("no active Show is loaded"))?;
    if active.id != request.show_id {
        return Err(conflict("the requested Show is not active"));
    }
    let document = ShowStore::open(&active.path)
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
    Ok(ProgrammingPresetRecallEnvironment {
        show_id: document.id(),
        show_revision: document.revision(),
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
        address: request.address,
        raw_body: Arc::new(object.body().clone()),
        preset: Arc::new(preset),
        groups: Arc::new(groups),
        programmer_fade_millis: ports.state().configuration.read().programmer_fade_millis,
    })
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
