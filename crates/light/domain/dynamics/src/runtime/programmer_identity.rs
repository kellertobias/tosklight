use super::{DynamicControllerSource, DynamicRuntimeError, DynamicRuntimeSnapshot};
use light_core::ProgrammerId;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

/// Normalize a legacy checkpoint only when the caller has verified its authored links against
/// the Programmer rows captured with that checkpoint. A missing link is not itself evidence
/// that the old controller ID was an authored link. Unmatched controllers remain unknown.
///
/// The caller supplies `(Programmer owner, authored link)` pairs, not session IDs. Existing
/// explicit source links are never relabelled. All collisions are checked before mutating any
/// row, so an error leaves the complete snapshot unchanged. Runtime instance IDs, phase, Random
/// streams, transitions, and retained expression graphs retain their original identity/state.
/// This is a cold restoration operation; it must run before installing a snapshot in a runtime.
pub fn normalize_legacy_programmer_controller_ids(
    snapshot: &mut DynamicRuntimeSnapshot,
    verified_links: &[(ProgrammerId, Uuid)],
) -> Result<usize, DynamicRuntimeError> {
    let verified = verified_links.iter().copied().collect::<HashSet<_>>();
    let explicit_sources = snapshot
        .instances
        .iter()
        .flat_map(|instance| &instance.controllers)
        .filter_map(|controller| match controller.source {
            DynamicControllerSource::Programmer {
                programmer_id,
                instance_link: Some(link),
            } => Some((ProgrammerId(programmer_id), link)),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut remaps = Vec::with_capacity(snapshot.instances.len());
    let mut final_ids = HashMap::<Uuid, bool>::new();
    let mut count = 0;
    for instance in &snapshot.instances {
        let mut remap = HashMap::new();
        let mut original_ids = HashMap::<Uuid, bool>::new();
        for controller in &instance.controllers {
            let source = match controller.source {
                DynamicControllerSource::Programmer {
                    programmer_id,
                    instance_link: None,
                } => Some((ProgrammerId(programmer_id), controller.id)),
                _ => None,
            };
            let mapped = source.filter(|source| verified.contains(source));
            if mapped.is_some_and(|source| explicit_sources.contains(&source)) {
                return Err(invalid(
                    "legacy and explicit controllers claim the same Programmer link",
                ));
            }
            let final_id = mapped.map_or(controller.id, |(owner, link)| {
                crate::programmer_dynamic_controller_id(owner, link)
            });
            if original_ids
                .insert(controller.id, mapped.is_some())
                .is_some_and(|previous_mapped| previous_mapped || mapped.is_some())
            {
                return Err(invalid(
                    "legacy controller rows have an ambiguous original identity",
                ));
            }
            if final_ids
                .insert(final_id, mapped.is_some())
                .is_some_and(|previous_mapped| previous_mapped || mapped.is_some())
            {
                return Err(invalid(
                    "normalized Programmer controller identity collides",
                ));
            }
            if mapped.is_some() {
                remap.insert(controller.id, final_id);
                count += 1;
            }
        }
        remaps.push(remap);
    }
    // Validation above is complete. The mutation phase is infallible and never touches tape
    // node/occurrence IDs: they are expression identity, not controller-keyed storage.
    for (instance, remap) in snapshot.instances.iter_mut().zip(remaps) {
        for controller in &mut instance.controllers {
            if let Some(&new_id) = remap.get(&controller.id) {
                let authored_link = controller.id;
                controller.id = new_id;
                if let DynamicControllerSource::Programmer { instance_link, .. } =
                    &mut controller.source
                {
                    *instance_link = Some(authored_link);
                }
            }
        }
        let rekey = |id: &mut Uuid| {
            if let Some(&new_id) = remap.get(id) {
                *id = new_id;
            }
        };
        for selection in &mut instance.lane_selections {
            rekey(&mut selection.controller_id);
        }
        for transition in &mut instance.controller_transitions {
            rekey(&mut transition.controller_id);
        }
        for sample in instance
            .last_sample_values
            .iter_mut()
            .chain(&mut instance.synchronized_hold_values)
        {
            rekey(&mut sample.controller_id);
        }
        // Each shared emission witness is one table entry: rewrite it once so every root
        // in both held maps stays bound to the same object (never regenerate per target).
        if let Some(tape) = &mut instance.expression_tape
            && tape
                .operation_emissions()
                .iter()
                .any(|emission| remap.contains_key(&emission.controller().id))
        {
            Arc::make_mut(tape).rekey_emission_controllers(&remap);
        }
    }
    Ok(count)
}

fn invalid(message: &str) -> DynamicRuntimeError {
    DynamicRuntimeError::InvalidSnapshot(message.into())
}

#[cfg(test)]
#[path = "programmer_identity_tests.rs"]
mod tests;
