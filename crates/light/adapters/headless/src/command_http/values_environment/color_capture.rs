//! TL-554: Color/native branch of `prepare_family_edit_context`.
//!
//! Runs only for the first sample of a gesture whose edits include a Direct (`native`) edit; a
//! retained gesture never calls it again, so the premaster output is seeded exactly once. For the
//! reference head (explicit, else the first verified root head of the ordered selection) it
//! installs, on every target and on a Group target's shared context:
//! - `native_model`: the verified original model from the runtime generation's catalogue;
//! - `direct_color_seed`: the complete Direct value captured once from that head's published
//!   premaster output in the accepted Live frame the surface displayed (its TL-594 lease), or the
//!   latest accepted frame when no lease is named (OSC, HTTP integrators).
//!
//! The Programmer planner then applies the edit with that model to every target: targets already
//! Direct of that source are edited in place, every other target (including other fixture types
//! of a mixed selection) takes the reference recipe, which the output replays exactly on
//! compatible heads and matches best effort elsewhere. Missing pieces install nothing; the
//! application then holds the whole action quietly (`NativeColorUnavailable`).
//!
//! G5: an idle reference head (no Color sidecar in the frame, nothing held) outputs its profile
//! defaults, so those are captured through the pinned model instead. Preload seeds from the
//! Programmer's accepted Pending pair (its After branch), published beside the Pending Position
//! readouts; never from the Live publication.
use light_application::{ProgrammingDisplayedLane, ProgrammingValuesEnvironment};
use light_core::programming::{ComponentEdit, ProgrammingOwner};
use light_core::{AttributeValue, FixtureId};
use light_wire::v2::visualization::VisualizationLane;
use std::sync::Arc;

use super::super::super::AppState;
use crate::runtime::output_readouts::DisplayedSource;
use crate::runtime::output_scheduler::physical_adapters::color::native::PublishedColorHead;
use crate::runtime::output_scheduler::physical_adapters::color::native_seed::{
    NativeReference, idle_native_seed, native_reference, native_source_model, pending_native_seed,
    published_native_seed,
};
use crate::runtime::visualization_frame::PublishedVisualizationFrame;

pub(super) fn prepare_native_color_context(
    state: &AppState,
    session: light_core::SessionId,
    preload: bool,
    intent: &light_application::ProgrammingValueIntent,
    edits: &[ComponentEdit],
    members: &[FixtureId],
    environment: &mut ProgrammingValuesEnvironment,
) {
    if !edits
        .iter()
        .any(|edit| matches!(edit, ComponentEdit::Native { .. }))
    {
        return;
    }
    let frame = if preload {
        None
    } else {
        match displayed_frame(state, session, intent) {
            Ok(frame) => frame,
            Err(()) => {
                environment.displayed_source_hold =
                    Some(light_application::ProgrammingValuesHold::DisplayedSourceUnavailable);
                return;
            }
        }
    };
    let snapshot = frame.as_ref().map_or_else(
        || state.output.engine().snapshot(),
        |frame| Arc::clone(&frame.source_snapshot),
    );
    let explicit = intent
        .color_adoption
        .native_reference
        .map(|reference| (reference.fixture_id, Some(reference.head_id)));
    let Some(reference) = native_reference(&snapshot, members, explicit, false) else {
        return;
    };
    let Some(model) = native_source_model(&snapshot, reference.identity()) else {
        return;
    };
    let seed = match frame.map(|frame| capture_seed(state, &snapshot, &frame, &reference)) {
        _ if preload => pending_seed(state, session, &snapshot, &reference),
        Some(Ok(seed)) => seed,
        // The leased frame is no longer retained or no longer this generation: the surface
        // re-reads and the next sample adopts its fresh source. Latest has nothing to retry.
        Some(Err(())) if intent.displayed_source.is_some() => {
            environment.displayed_source_hold =
                Some(light_application::ProgrammingValuesHold::DisplayedSourceUnavailable);
            return;
        }
        _ => None,
    };
    let mut install = |context: &mut light_application::ProgrammingFamilyContext| {
        context.native_model = Some(Arc::clone(&model));
        context.direct_color_seed = seed.clone();
    };
    for fixture in members {
        install(environment.family_contexts.entry(*fixture).or_default());
        if let Some(seed) = &seed {
            environment
                .current_values
                .entry((*fixture, ProgrammingOwner::Color.key()))
                .or_insert_with(|| seed.clone());
        }
    }
    if let Some(group) = &intent.group_id {
        install(
            environment
                .group_family_contexts
                .entry(group.clone())
                .or_default(),
        );
    }
}

/// The accepted Live frame the surface displayed (its leased, pinned source), or the latest
/// accepted frame without a lease. `Err` holds: an unresolvable lease or a lease of another lane.
fn displayed_frame(
    state: &AppState,
    session: light_core::SessionId,
    intent: &light_application::ProgrammingValueIntent,
) -> Result<Option<Arc<PublishedVisualizationFrame>>, ()> {
    let active = state.active_show.current().map(|show| show.id.0);
    let Some(displayed) = intent.displayed_source else {
        return Ok(state
            .output
            .latest_visualization_frame()
            .filter(|frame| frame.scope.show_id == active));
    };
    if displayed.lane != ProgrammingDisplayedLane::Normal {
        return Err(());
    }
    let leases = state.sessions.displayed_sources().leases();
    let now = std::time::Instant::now();
    let source = match intent.undo_group.as_deref() {
        Some(gesture) => leases.pin(
            session,
            VisualizationLane::Normal,
            displayed.lease,
            gesture,
            now,
        ),
        None => leases.resolve(session, VisualizationLane::Normal, displayed.lease, now),
    };
    match source {
        Some(DisplayedSource::Live(frame)) if frame.scope.show_id == active => Ok(Some(frame)),
        _ => Err(()),
    }
}

/// The Direct seed from the reference head's output in `frame`. `Err`: the frame is no longer
/// retained or no longer the engine's generation. `Ok(None)`: the reference head published no
/// Color output in that frame (or its output failed the authoritative guard).
fn capture_seed(
    state: &AppState,
    snapshot: &light_engine::EngineSnapshot,
    frame: &PublishedVisualizationFrame,
    reference: &NativeReference,
) -> Result<Option<AttributeValue>, ()> {
    if !Arc::ptr_eq(&state.output.engine().snapshot(), &frame.source_snapshot) {
        return Err(());
    }
    let accepted = state
        .output
        .live_family_adapters()
        .accepted_color(frame.generation, frame.sampled_at)
        .ok_or(())?;
    let Some(output) = accepted.output(reference.target) else {
        // G5: an idle head (no requested colour, nothing held) outputs its profile defaults.
        if accepted.held(reference.target, reference.target) {
            return Ok(None);
        }
        return Ok(idle_native_seed(snapshot, &reference));
    };
    Ok(published_native_seed(
        snapshot,
        (frame.generation, frame.sampled_at),
        reference,
        PublishedColorHead {
            token: &output.token,
            target: output.target,
            value: &output.value,
            writes: &output.writes,
        },
    )
    .ok())
}

/// G5: the Direct seed of the reference head in the Programmer's accepted Pending pair: its
/// After-branch output, or its profile defaults when that pair gave it no Color output. None
/// without an accepted pair of this Programmer (never the Live publication).
fn pending_seed(
    state: &AppState,
    session: light_core::SessionId,
    snapshot: &light_engine::EngineSnapshot,
    reference: &NativeReference,
) -> Option<AttributeValue> {
    let source = state.output.pending_position_readouts().source()?;
    let programmer = state.programming.get(session)?.id;
    match source.color_output(programmer, reference.target) {
        Some(output) => pending_native_seed(
            snapshot,
            reference,
            PublishedColorHead {
                token: &output.token,
                target: output.target,
                value: &output.value,
                writes: &output.writes,
            },
        )
        .ok(),
        None => source
            .capture(programmer, &[reference.target])
            .and_then(|_| idle_native_seed(snapshot, reference)),
    }
}
