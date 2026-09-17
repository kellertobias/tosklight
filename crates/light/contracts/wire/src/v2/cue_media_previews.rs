//! Typed transport for Cue previews drawn by a Media Server.
//!
//! A Cue that contains only Media Server content is previewed by the Media Server itself: the
//! desk computes the DMX slots the Cue would transmit and asks the addressed server to render
//! them off-screen. The index names, for every such Cue, exactly which server, output, and layer
//! the picture belongs to, so a desk can never show one server's image for another's Cue.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Whether a Cue previews a whole output or one logical layer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueMediaPreviewScope {
    /// The composited Program image of the Media Server output, master included.
    Program,
    /// One logical layer's composited image, with its transparency kept.
    Layer,
}

/// One Cue whose preview a Media Server draws.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueMediaPreviewEntry {
    pub cue_id: Uuid,
    pub cue_list_id: Uuid,
    /// The patched Media Server fixture (the parent that owns the master and the layer heads).
    pub server_fixture_id: Uuid,
    /// The server's patched output, or `None` when the fixture follows the server's first output.
    #[ts(optional = nullable)]
    pub output_id: Option<String>,
    pub scope: CueMediaPreviewScope,
    /// Zero-based layer for a layer preview.
    #[ts(optional = nullable)]
    pub layer: Option<u16>,
    /// The layer's logical-head fixture for a layer preview.
    #[ts(optional = nullable)]
    pub layer_fixture_id: Option<Uuid>,
    /// Changes whenever the picture would: a different server, output, layer, or programmed
    /// state. A desk uses it as the image's cache identity.
    #[schemars(length(min = 1, max = 128))]
    pub preview_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueMediaPreviewIndex {
    pub show_id: Uuid,
    pub entries: Vec<CueMediaPreviewEntry>,
}

/// Why a Media Server Cue preview has no picture right now.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueMediaPreviewFailureState {
    /// The Media Server did not answer.
    Offline,
    /// The Media Server is still loading the selected media.
    Loading,
    /// The Cue, the server fixture, or the patched output no longer exists, or the server cannot
    /// draw this output.
    Missing,
}

/// Body of a failed Cue media preview request.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueMediaPreviewFailure {
    pub state: CueMediaPreviewFailureState,
    pub error: String,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_names_its_server_output_and_layer() {
        let entry = CueMediaPreviewEntry {
            cue_id: Uuid::from_u128(1),
            cue_list_id: Uuid::from_u128(2),
            server_fixture_id: Uuid::from_u128(3),
            output_id: Some("00000000-0000-4000-8000-000000000001".into()),
            scope: CueMediaPreviewScope::Layer,
            layer: Some(1),
            layer_fixture_id: Some(Uuid::from_u128(4)),
            preview_key: "abc".into(),
        };
        let encoded = serde_json::to_value(&entry).unwrap();
        assert_eq!(encoded["scope"], "layer");
        assert_eq!(encoded["layer"], 1);
        assert_eq!(
            serde_json::from_value::<CueMediaPreviewEntry>(encoded).unwrap(),
            entry
        );
    }

    #[test]
    fn a_failure_names_an_operator_state() {
        let failure = CueMediaPreviewFailure {
            state: CueMediaPreviewFailureState::Offline,
            error: "no answer".into(),
            retryable: true,
        };
        assert_eq!(serde_json::to_value(&failure).unwrap()["state"], "offline");
    }
}
