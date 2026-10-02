//! TL-554 Direct (native) Color pages 3/4 and the modal overflow of one ordered selection.
//!
//! The page layout is built from the reference head's compiled `ColorDescriptor` against the
//! runtime generation (never the library): the operator's explicit choice when it is a verified
//! root head of the selection, otherwise the first verified root head in selection order. Pages
//! 3/4 hold at most eight controls in optical-path order; every further control is overflow for
//! the full Color modal, so nothing is omitted. Controls are identified by channel and function
//! UUIDs; labels are display text only.
//!
//! Reading pages is inert: no lease, no history, no intent change. The reference head's current
//! premaster values come read-only from the latest accepted Live frame (`inspect_native_values`).

use super::AppState;
use super::command_http::ToIntentWire;
use super::output_scheduler::physical_adapters::color::native::{
    NATIVE_PAGE_CONTROLS, NativeColorControl, PublishedColorHead, inspect_native_values,
};
use super::output_scheduler::physical_adapters::color::native_seed::{
    NativeReference, native_reference, native_source_model,
};
use light_core::programming::{
    DirectCompatibility, DirectDestination, PROGRAMMING_CONTRACT_VERSION,
    direct_color_compatibility,
};
use light_core::{FixtureId, NativeColorIdentity};
use light_engine::{EngineSnapshot, profile_head_destinations};
use light_wire::v2::native_color as wire;
use std::sync::Arc;
use uuid::Uuid;

/// Encoder slots per native page.
const PAGE_SLOTS: usize = 4;

pub(super) fn native_color_pages(
    state: &AppState,
    requested: &[FixtureId],
    explicit: Option<(FixtureId, Option<Uuid>)>,
) -> wire::NativeColorPagesSnapshot {
    let engine = state.output.engine();
    let snapshot = engine.snapshot();
    let mut pages = wire::NativeColorPagesSnapshot {
        semantic: engine.supported_programming_contract() >= PROGRAMMING_CONTRACT_VERSION,
        show_revision: snapshot.revision,
        fixture_ids: requested.iter().map(|id| id.0).collect(),
        unavailable: None,
        reference: None,
        candidates: Vec::new(),
        pages: Vec::new(),
        overflow: Vec::new(),
        fixtures: Vec::new(),
        values: None,
    };
    if !pages.semantic {
        pages.unavailable = Some(wire::NativeColorPagesUnavailable::Contract);
        return pages;
    }
    let mut members = Vec::with_capacity(requested.len());
    for id in requested {
        if !members.contains(id) {
            members.push(*id);
        }
    }
    let names = head_names(state, &members);
    pages.candidates = candidates(&snapshot, &members, &names);
    let Some(reference) = native_reference(&snapshot, &members, explicit, true) else {
        pages.unavailable = Some(wire::NativeColorPagesUnavailable::NoVerifiedHead);
        return pages;
    };
    let controls: Vec<_> = reference
        .controls()
        .iter()
        .map(|control| control_descriptor(&snapshot, &reference, control))
        .collect();
    let split = controls.len().min(NATIVE_PAGE_CONTROLS);
    pages.pages = controls[..split]
        .chunks(PAGE_SLOTS)
        .zip(3u8..)
        .map(|(chunk, number)| {
            let mut slots: Vec<_> = chunk.iter().cloned().map(Some).collect();
            slots.resize(PAGE_SLOTS, None);
            wire::NativeColorPage {
                number,
                controls: slots,
            }
        })
        .collect();
    pages.overflow = controls[split..].to_vec();
    pages.fixtures = previews(&snapshot, &members, reference.identity());
    pages.values = reference_values(state, &snapshot, &reference);
    pages.reference = Some(reference_dto(&snapshot, &reference, &names));
    pages
}

/// `(owner, head) → head name` of the selection's Color report heads.
fn head_names(state: &AppState, members: &[FixtureId]) -> Vec<(FixtureId, Uuid, String)> {
    let wanted = members.iter().copied().collect();
    state
        .output
        .engine()
        .color_report_heads(Some(&wanted))
        .1
        .into_iter()
        .map(|head| (head.owner, head.head_id, head.head_name))
        .collect()
}

fn head_name(names: &[(FixtureId, Uuid, String)], target: FixtureId, head: Uuid) -> String {
    names
        .iter()
        .find(|(owner, id, _)| *owner == target && *id == head)
        .or_else(|| names.iter().find(|(_, id, _)| *id == head))
        .map(|(_, _, name)| name.clone())
        .unwrap_or_default()
}

fn fixture_label(snapshot: &EngineSnapshot, target: FixtureId) -> (Option<u32>, String) {
    snapshot
        .fixtures
        .iter()
        .find_map(|fixture| {
            if fixture.fixture_id == target {
                return Some((fixture.fixture_number, fixture.name.clone()));
            }
            fixture
                .logical_heads
                .iter()
                .find(|head| head.fixture_id == target)
                .map(|_| (fixture.fixture_number, fixture.name.clone()))
        })
        .unwrap_or((None, String::new()))
}

/// Root heads with a verified native identity in the runtime Color context, selection order.
fn candidates(
    snapshot: &EngineSnapshot,
    members: &[FixtureId],
    names: &[(FixtureId, Uuid, String)],
) -> Vec<wire::NativeColorReferenceCandidate> {
    members
        .iter()
        .flat_map(|target| {
            identities(snapshot, *target)
                .into_iter()
                .map(move |identity| (*target, identity.head_id))
        })
        .map(|(target, head_id)| {
            let (fixture_number, fixture_name) = fixture_label(snapshot, target);
            wire::NativeColorReferenceCandidate {
                fixture_id: target.0,
                fixture_number,
                fixture_name,
                head_id,
                head_name: head_name(names, target, head_id),
            }
        })
        .collect()
}

/// The verified native identities of the heads `target` owns, in mode order.
fn identities(snapshot: &EngineSnapshot, target: FixtureId) -> Vec<NativeColorIdentity> {
    profile_head_destinations(snapshot, target)
        .into_iter()
        .filter_map(|destination| {
            let fixture = &snapshot.fixtures[destination.fixture_index];
            fixture
                .definition
                .runtime_color_context
                .as_deref()?
                .identities()
                .iter()
                .find(|identity| identity.head_id == destination.head_id)
                .cloned()
        })
        .collect()
}

/// Predicted replay of the reference recipe: identical verified identity and native layout
/// replays exactly (TL-595 compatibility, never names or slots); anything else is best effort.
fn previews(
    snapshot: &EngineSnapshot,
    members: &[FixtureId],
    reference: &NativeColorIdentity,
) -> Vec<wire::NativeColorFixturePreview> {
    members
        .iter()
        .filter(|target| !profile_head_destinations(snapshot, **target).is_empty())
        .map(|target| {
            let exact = identities(snapshot, *target).iter().any(|identity| {
                native_source_model(snapshot, identity).is_some_and(|model| {
                    direct_color_compatibility(
                        reference,
                        &DirectDestination::Verified(model.as_ref()),
                    ) == DirectCompatibility::Compatible
                })
            });
            wire::NativeColorFixturePreview {
                fixture_id: target.0,
                replay: if exact {
                    wire::NativeColorReplayPreview::Exact
                } else {
                    wire::NativeColorReplayPreview::Fallback
                },
            }
        })
        .collect()
}

fn control_descriptor(
    snapshot: &EngineSnapshot,
    reference: &NativeReference,
    control: &NativeColorControl,
) -> wire::NativeColorControlDescriptor {
    let channel = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == reference.descriptor.root)
        .and_then(|fixture| fixture.definition.runtime_color_context.as_deref())
        .and_then(|context| {
            context
                .mode()
                .channels
                .iter()
                .find(|channel| channel.id == control.channel_id)
        });
    let function_label = |id: Uuid| {
        channel
            .and_then(|channel| channel.functions.iter().find(|function| function.id == id))
            .map(|function| function.name.clone())
            .unwrap_or_default()
    };
    wire::NativeColorControlDescriptor {
        id: format!("native.{}", control.channel_id),
        channel_id: control.channel_id,
        label: channel.map_or_else(
            || control.attribute.0.to_string(),
            |channel| channel.fixture_attribute.0.to_string(),
        ),
        raw_max: control.raw_max,
        resolution: wire::native_resolution(control.raw_max),
        ultraviolet: control.ultraviolet,
        functions: control
            .functions
            .iter()
            .map(|function| wire::NativeColorFunctionDescriptor {
                function_id: function.binding.function_id,
                label: function_label(function.binding.function_id),
                raw_from: function.raw_from,
                raw_to: function.raw_to,
                continuous: function.continuous,
            })
            .collect(),
    }
}

fn reference_dto(
    snapshot: &EngineSnapshot,
    reference: &NativeReference,
    names: &[(FixtureId, Uuid, String)],
) -> wire::NativeColorReference {
    let (fixture_number, fixture_name) = fixture_label(snapshot, reference.target);
    wire::NativeColorReference {
        fixture_id: reference.target.0,
        fixture_number,
        fixture_name,
        head_id: reference.head_id,
        head_name: head_name(names, reference.target, reference.head_id),
        chosen: reference.chosen,
        identity: reference.identity().to_intent_wire(),
    }
}

/// The reference head's premaster values in the latest accepted Live frame of this generation.
fn reference_values(
    state: &AppState,
    snapshot: &Arc<EngineSnapshot>,
    reference: &NativeReference,
) -> Option<wire::NativeColorValues> {
    let frame = state.output.latest_visualization_frame()?;
    if !Arc::ptr_eq(&frame.source_snapshot, snapshot) {
        return None;
    }
    let accepted = state
        .output
        .live_family_adapters()
        .accepted_color(frame.generation, frame.sampled_at)?;
    let output = accepted.output(reference.target)?;
    let values = inspect_native_values(
        reference.head(),
        &PublishedColorHead {
            token: &output.token,
            target: output.target,
            value: &output.value,
            writes: &output.writes,
        },
    )
    .ok()?;
    Some(wire::NativeColorValues {
        frame: frame.identity(),
        controls: values
            .into_iter()
            .map(|value| wire::NativeColorValueReadout {
                channel_id: value.channel_id,
                function_id: value.function_id,
                raw: value.raw,
            })
            .collect(),
    })
}

#[cfg(test)]
#[path = "native_color_pages_tests.rs"]
mod tests;
