//! TL-594 track B: Preload readers of the accepted Pending publication.
//!
//! Behind the one C3/C4 family-adapter gate (engine contract AND the explicit opt-in,
//! [`PendingEpisodeResource::gated`]), the Preload lane of `GET /output/dmx?include_preload`
//! and the Preload lane of the visualization snapshot/stream present the desk's last accepted
//! Pending frame exactly as the executor published it through `PendingPublicationGate`, with its
//! own identity: the episode, and the ticket as `frame.sequence`. Nothing is re-evaluated.
//!
//! When nothing is published for the current show and desk Programmer, readers report a
//! passive [`OutputPreloadState`]. They never fall back to Live, to the Live stamp, or to a
//! fresh Preload projection.
//!
//! With the gate off, [`published_preload`] returns `None` and every caller keeps its legacy
//! path unchanged.
use super::AppState;
use super::output_scheduler::{PendingEpisodeResource, PendingEpisodeStatus, PendingNativeReadout};
use light_wire::v2::output_control::{
    OutputDmxSnapshot, OutputFrameIdentity, OutputPreloadState, OutputPreloadStatus,
};
use std::sync::Arc;

/// What a gated Preload reader presents for one read.
#[derive(Clone, Debug)]
pub(super) enum PublishedPreload {
    /// The current episode's last accepted Pending readout.
    Published(Arc<PendingNativeReadout>),
    /// Nothing accepted to present; passive, with no fallback.
    Unavailable(OutputPreloadState),
}

impl PublishedPreload {
    pub(super) fn status(&self) -> OutputPreloadStatus {
        match self {
            Self::Published(readout) => OutputPreloadStatus {
                state: OutputPreloadState::Published,
                episode: Some(readout.identity().episode),
            },
            Self::Unavailable(state) => OutputPreloadStatus {
                state: *state,
                episode: None,
            },
        }
    }

    pub(super) fn frame(&self) -> Option<&OutputFrameIdentity> {
        match self {
            Self::Published(readout) => readout.lane().frame.as_ref(),
            Self::Unavailable(_) => None,
        }
    }
}

/// The accepted Pending publication for this desk, or `None` when the family-adapter gate is
/// off (the caller then keeps its legacy Preload path).
///
/// A published readout is presented only when it belongs to the active show and the desk's
/// Programmer; anything else is "not yet available". The gate already hides every earlier
/// episode's readout, so no reader can present a superseded episode.
pub(super) fn published_preload(state: &AppState) -> Option<PublishedPreload> {
    if !PendingEpisodeResource::gated(state) {
        return None;
    }
    let Some(executor) = state.programming.pending_episodes().executor() else {
        return Some(PublishedPreload::Unavailable(
            OutputPreloadState::NotYetAvailable,
        ));
    };
    if let Some(readout) = executor.latest() {
        let identity = readout.identity();
        let show = state.active_show.current().map(|show| show.id);
        let programmer = state.programming.programmers().programmer_id();
        return Some(
            if show == Some(identity.show_id) && programmer == Some(identity.programmer) {
                PublishedPreload::Published(readout)
            } else {
                PublishedPreload::Unavailable(OutputPreloadState::NotYetAvailable)
            },
        );
    }
    Some(PublishedPreload::Unavailable(match executor.status() {
        PendingEpisodeStatus::Idle => OutputPreloadState::Idle,
        PendingEpisodeStatus::QueuePreviewUnavailable => {
            OutputPreloadState::QueuePreviewUnavailable
        }
        PendingEpisodeStatus::Waiting(_)
        | PendingEpisodeStatus::Running(_)
        | PendingEpisodeStatus::Stopped => OutputPreloadState::NotYetAvailable,
    }))
}

/// Put the accepted Pending lane (or the passive status alone) on a desk output snapshot.
/// The lane is the published DTO byte for byte, stamped with its own ticket identity.
pub(super) fn attach_dmx_preload(output: &mut OutputDmxSnapshot, published: &PublishedPreload) {
    output.preload = match published {
        PublishedPreload::Published(readout) => Some(readout.lane().clone()),
        PublishedPreload::Unavailable(_) => None,
    };
    output.preload_status = Some(published.status());
}

/// The Preload visualization lane from the publication: the accepted pair's own values,
/// profile values, options, revision and frame identity. No dynamic stack is carried (the
/// publication holds evaluated values, not the programming stack). When nothing is published
/// the lane is empty and says so; Live is never substituted.
pub(super) fn visualization_content(
    state: &AppState,
    published: &PublishedPreload,
) -> serde_json::Value {
    let status = published.status();
    let pending = serde_json::json!({
        "state": status.state,
        "episode": status.episode,
        "frame": published.frame(),
    });
    match published {
        PublishedPreload::Published(readout) => {
            let lane = readout.lane();
            let values = readout.values();
            serde_json::json!({
                "scope": { "show_id": lane.show_id },
                "revision": lane.revision,
                "generated_at": lane.frame.as_ref().map_or_else(
                    || chrono::Utc::now().to_rfc3339(),
                    |frame| frame.sampled_at.clone(),
                ),
                "grand_master": values.grand_master,
                "blackout": values.blackout,
                "preload": true,
                "values": super::operator_api::visualization_wire_values(&values.values),
                "dynamic_stack": [],
                "profile_output_values":
                    super::operator_api::visualization_wire_values(&values.profile),
                "pending": pending,
            })
        }
        PublishedPreload::Unavailable(_) => {
            let options = state.output.render_options();
            serde_json::json!({
                "scope": { "show_id": state.active_show.current().map(|show| show.id.0) },
                "revision": state.output.snapshot().revision,
                "generated_at": chrono::Utc::now(),
                "grand_master": options.grand_master,
                "blackout": options.blackout,
                "preload": true,
                "values": [],
                "dynamic_stack": [],
                "profile_output_values": [],
                "pending": pending,
            })
        }
    }
}
