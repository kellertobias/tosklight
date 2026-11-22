//! Explicit temporary output preview actions; these are never persisted or replayed.
use super::{OutputView, PlaylistView};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPlaylistStep {
    pub output_id: String,
    pub preview_id: String,
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub expected_ownership_generation: u64,
    /// Authoring draft, rather than a second persisted playlist or client-computed layer writes.
    #[ts(
        type = "Omit<PlaylistView, 'auxiliaryLayers'> & { auxiliaryLayers: Array<Omit<AuxiliaryLayerView, 'controls'> & { controls: Partial<LayerPresetView> }> }"
    )]
    pub playlist: PlaylistView,
    pub entry_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StopPlaylistPreview {
    pub output_id: String,
    pub preview_id: String,
    #[ts(type = "number")]
    pub sequence: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPreviewState {
    pub output_id: String,
    pub preview_id: String,
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub ownership_generation: u64,
    pub playlist_id: String,
    pub entry_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPreviewResult {
    pub active: Option<PlaylistPreviewState>,
    pub output: OutputView,
}
