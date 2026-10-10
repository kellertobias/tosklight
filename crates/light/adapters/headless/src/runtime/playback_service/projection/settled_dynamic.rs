//! Publish assignment feedback after the authoritative Dynamic output transaction settles.
use super::*;
use light_application::{
    ApplicationEvent, EventDraft, EventSource, OutputEvent, PlaybackRuntimeChange,
};
use std::collections::HashSet;

/// Called inside the existing ordered Playback operation, after the Dynamic sample commits.
/// Operator On feedback may precede controller reconciliation. Its eventual state, including
/// full-control auto-off, must reach every alias through the Playback event stream as well.
/// Only targets touched by committed lifecycle changes are projected; unchanged frames emit
/// nothing. A rejected output transaction must never call this publisher.
pub(in crate::runtime) fn settled_dynamic_playback_events(
    engine: &light_engine::Engine,
    scope: PlaybackShowScope,
    snapshot: &EngineSnapshot,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    changes: &[EventDraft],
    auto_offs: &[PlaybackIdentity],
    output_interval_millis: u64,
) -> Vec<EventDraft> {
    let mut changed = HashSet::new();
    let mut global_changed = false;
    for event in changes {
        if let ApplicationEvent::Output(OutputEvent::DynamicRuntimeChanged(change)) = &event.payload
        {
            if let Some(id) = change.dynamic_id {
                changed.insert(id);
            } else {
                global_changed = true;
            }
        }
    }
    if changed.is_empty() && !global_changed && auto_offs.is_empty() {
        return Vec::new();
    }
    let aliases = snapshot
        .playbacks
        .iter()
        .filter_map(|definition| {
            PlaybackIdentity::physical(definition.number)
                .ok()
                .map(|identity| (identity, definition))
        })
        .chain(snapshot.playback_pages.iter().flat_map(|page| {
            page.virtual_playbacks
                .iter()
                .filter_map(move |(number, definition)| {
                    light_playback::VirtualPlaybackAddress::new(page.number, *number)
                        .ok()
                        .map(|address| (PlaybackIdentity::Virtual(address), definition))
                })
        }))
        .collect::<Vec<_>>();
    for (identity, definition) in &aliases {
        if auto_offs.contains(identity)
            && let PlaybackTarget::Dynamic { assignment } = &definition.target
        {
            changed.insert(assignment.target_id());
        }
    }
    aliases
        .into_iter()
        .filter_map(|(identity, definition)| {
            let PlaybackTarget::Dynamic { assignment } = &definition.target else {
                return None;
            };
            if !global_changed && !changed.contains(&assignment.target_id()) {
                return None;
            }
            let active = engine.active_dynamic_playback_at(identity)?;
            let requested = match identity {
                PlaybackIdentity::Physical(number) => {
                    PlaybackRuntimeIdentity::Playback(number.get())
                }
                PlaybackIdentity::Virtual(address) => PlaybackRuntimeIdentity::Virtual(address),
            };
            let projection = PlaybackRuntimeProjection {
                scope,
                requested,
                playback_number: Some(identity.number()),
                target: PlaybackTargetProjection::Dynamic {
                    dynamic_id: assignment.dynamic.dynamic_id,
                    last_known_pool_number: assignment.dynamic.last_known_pool_number,
                    embedded: assignment.dynamic.dynamic_id.is_none(),
                    runtime: Some(Box::new(dynamic_runtime_projection_from_snapshot(
                        snapshot,
                        assignment,
                        active,
                        runtime,
                        output_interval_millis,
                    ))),
                },
            };
            Some(EventDraft::playback_runtime_changed(
                None,
                PlaybackRuntimeChange {
                    projection,
                    transition: None,
                },
                EventSource::Runtime,
                None,
            ))
        })
        .collect()
}
