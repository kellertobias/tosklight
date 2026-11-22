//! Ephemeral, generation-guarded playlist previews. No preview changes durable configuration.
mod composition;
#[cfg(test)]
mod tests;

use crate::{
    ApiError,
    wire::{PlaylistPreviewState, PreviewPlaylistStep, StopPlaylistPreview},
};
use arc_swap::ArcSwap;
use media_application::MediaConfiguration;
use media_application::configuration::validate_automation;
use media_domain::catalog::CatalogSnapshot;
use media_domain::playlist::{AutomationConfiguration, AutomationOutput, Playlist};
use media_domain::{Applied, CommandSource, MediaState, OutputId, OutputState, Timestamp};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const CLOSED_LIMIT: usize = 256;

struct Session {
    view: PlaylistPreviewState,
    original: OutputState,
    base: OutputState,
}

/// The most recent closed preview IDs remain tombstones so delayed Start cannot revive Stop.
#[derive(Default)]
pub struct PreviewSessions {
    active: HashMap<OutputId, Session>,
    closed: VecDeque<Uuid>,
}

impl PreviewSessions {
    pub(crate) fn show(
        &mut self,
        state: &ArcSwap<MediaState>,
        configuration: &MediaConfiguration,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        request: &PreviewPlaylistStep,
    ) -> Result<(PlaylistPreviewState, OutputState), ApiError> {
        let output = OutputId::from_uuid(identity(&request.output_id, "outputId")?);
        let preview = identity(&request.preview_id, "previewId")?;
        sequence(request.sequence)?;
        let playlist: Playlist =
            serde_json::from_value(serde_json::to_value(&request.playlist).map_err(invalid)?)
                .map_err(invalid)?;
        let entry = identity(&request.entry_id, "entryId")?;
        let index = playlist
            .entries
            .iter()
            .position(|each| each.id == entry)
            .ok_or_else(|| invalid("The selected playlist entry does not exist"))?;
        validate(configuration, output, &playlist)?;
        if self.closed.contains(&preview) {
            return Err(conflict(
                "This preview was already stopped; start a new preview",
            ));
        }
        let current = state.load_full();
        let found = current
            .output(output)
            .ok_or_else(|| ApiError::not_found("output-not-found", "The output does not exist"))?;
        if self.active.get(&output).is_some_and(|session| {
            session.view.ownership_generation != found.ownership.generation
                || !found.ownership.web_takeover
        }) {
            let old = self.active.remove(&output).expect("checked above");
            self.remember(identity(&old.view.preview_id, "previewId")?);
        }
        let existing = self.active.get(&output);
        if let Some(session) = existing {
            if session.view.preview_id != request.preview_id {
                return Err(conflict(
                    "Another playlist preview is active; stop it or release playback takeover first",
                ));
            }
            if request.sequence <= session.view.sequence {
                return Err(conflict(
                    "A newer preview action already replaced this step",
                ));
            }
        }
        // A new session captures its baseline in the accepted transaction, never in this read.
        let first = existing.is_none();
        let restart_step = existing.is_none_or(|session| {
            session.view.entry_id != request.entry_id
                || session.view.playlist_id != request.playlist.id
        });
        let restart_auxiliary =
            existing.is_none_or(|session| session.view.playlist_id != request.playlist.id);
        let (before, after) = crate::control::submit_output_transaction(
            state,
            output,
            request.expected_ownership_generation,
            CommandSource::Web,
            now,
            |current| {
                let base = existing
                    .map(|session| session.base.clone())
                    .unwrap_or_else(|| composition::normalized_base(current, now));
                let mut commands = if first {
                    composition::takeover(output)
                } else {
                    Vec::new()
                };
                commands.extend(composition::commands(
                    current,
                    &base,
                    &playlist,
                    index,
                    catalog,
                    restart_step,
                    restart_auxiliary,
                )?);
                Ok(commands)
            },
            refusal,
        )?;
        let view = PlaylistPreviewState {
            output_id: output.to_string(),
            preview_id: preview.to_string(),
            sequence: request.sequence,
            ownership_generation: after.ownership.generation,
            playlist_id: playlist.id.to_string(),
            entry_id: entry.to_string(),
        };
        if let Some(session) = self.active.get_mut(&output) {
            session.view = view.clone();
        } else {
            self.active.insert(
                output,
                Session {
                    view: view.clone(),
                    base: composition::normalized_base(&before, now),
                    original: before,
                },
            );
        }
        Ok((view, after))
    }

    pub(crate) fn stop(
        &mut self,
        state: &ArcSwap<MediaState>,
        now: Timestamp,
        request: &StopPlaylistPreview,
    ) -> Result<OutputState, ApiError> {
        let output = OutputId::from_uuid(identity(&request.output_id, "outputId")?);
        let preview = identity(&request.preview_id, "previewId")?;
        sequence(request.sequence)?;
        let current = state.load_full();
        let found = current
            .output(output)
            .ok_or_else(|| ApiError::not_found("output-not-found", "The output does not exist"))?;
        let Some(session) = self.active.get(&output) else {
            self.remember(preview);
            return Ok(found.clone());
        };
        if session.view.preview_id != request.preview_id {
            self.remember(preview);
            return Ok(found.clone());
        }
        if request.sequence <= session.view.sequence {
            return Err(conflict(
                "A newer preview action already replaced this stop",
            ));
        }
        let session = self.active.remove(&output).expect("checked above");
        self.remember(preview);
        if found.ownership.generation != session.view.ownership_generation
            || !found.ownership.web_takeover
        {
            return Ok(found.clone());
        }
        match crate::control::submit_output_transaction(
            state,
            output,
            session.view.ownership_generation,
            CommandSource::Web,
            now,
            |current| Ok(composition::restore(current, &session.original)),
            refusal,
        ) {
            Ok((_, after)) => Ok(after),
            // A desk or manual owner acquired between the read and CAS. Never restore over it.
            Err(_) => state.load().output(output).cloned().ok_or_else(|| {
                ApiError::not_found("output-not-found", "The output no longer exists")
            }),
        }
    }

    fn remember(&mut self, id: Uuid) {
        self.closed.retain(|old| *old != id);
        self.closed.push_back(id);
        while self.closed.len() > CLOSED_LIMIT {
            self.closed.pop_front();
        }
    }
}

fn validate(
    configuration: &MediaConfiguration,
    output: OutputId,
    playlist: &Playlist,
) -> Result<(), ApiError> {
    let mut candidate = configuration.clone();
    candidate.automation = AutomationConfiguration {
        playlists: vec![playlist.clone()],
        events: Vec::new(),
        outputs: vec![AutomationOutput {
            output_id: output,
            enabled: false,
            fallback_playlist_id: Some(playlist.id),
            dmx_silence_seconds: 10,
        }],
    };
    validate_automation(&candidate).map_err(invalid)
}
fn identity(value: &str, field: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| invalid(format!("{field} must be a nonempty UUID")))
}
fn sequence(value: u64) -> Result<(), ApiError> {
    if value == 0 || value > 9_007_199_254_740_991 {
        return Err(invalid("sequence must be a positive safe integer"));
    }
    Ok(())
}
fn invalid(message: impl std::fmt::Display) -> ApiError {
    ApiError::bad_request("playlist-preview-invalid", message.to_string())
}
fn conflict(message: &str) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::CONFLICT,
        "playlist-preview-stale",
        message,
    )
}
fn refusal(refusal: Applied) -> ApiError {
    match refusal {
        Applied::RejectedNotOwner => {
            conflict("Playback ownership changed; this preview can no longer write")
        }
        Applied::RejectedUnknownOutput => {
            ApiError::not_found("output-not-found", "The output no longer exists")
        }
        Applied::RejectedUnknownLayer => {
            invalid("The selected layers no longer exist on this output")
        }
        _ => invalid("The preview could not be applied"),
    }
}
