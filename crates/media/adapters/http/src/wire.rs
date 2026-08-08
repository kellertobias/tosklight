//! The typed wire contract.
//!
//! Request types are tolerant of unknown fields; response types are explicit. Nothing here is a
//! hand-built JSON string, and nothing re-derives a domain rule — the API projects state, it does
//! not decide it.

use media_domain::catalog::CatalogSnapshot;
use media_domain::{LayerState, MasterState, MediaAddress, OutputId, SourceStatus};
use serde::{Deserialize, Serialize};

/// Whether the process is up and what it is running.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub status: &'static str,
    pub instance: String,
    pub outputs: usize,
    pub catalog_revision: u64,
    pub catalog_items: usize,
}

/// One layer, as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayerView {
    pub index: usize,
    pub folder: u8,
    pub file: u8,
    pub play_mode: String,
    pub dimmer: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub grayscale: f32,
    pub source_status: SourceStatus,
    /// Whether this layer contributes pixels right now.
    pub drawing: bool,
}

impl LayerView {
    pub fn of(index: usize, layer: &LayerState) -> Self {
        Self {
            index,
            folder: layer.address.folder,
            file: layer.address.file,
            play_mode: layer.play_mode.label().to_owned(),
            dimmer: layer.dimmer,
            scale_x: layer.scale_x,
            scale_y: layer.scale_y,
            position_x: layer.position_x,
            position_y: layer.position_y,
            rotation: layer.rotation,
            grayscale: layer.grayscale,
            source_status: layer.source_status,
            drawing: layer.draws(),
        }
    }
}

/// One output's whole state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputView {
    pub id: OutputId,
    pub name: String,
    pub personality: String,
    pub layers: Vec<LayerView>,
    pub master: MasterState,
    /// Whether an external desk currently owns this output's continuously controlled values.
    pub dmx_active: bool,
}

/// The library, as the API reports it. The same immutable snapshot the renderer reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    pub revision: u64,
    pub snapshot: CatalogSnapshot,
}

/// An intent-shaped layer update: only the fields being changed.
///
/// Absent means "leave alone", which is why every field is optional. Sending a dimmer must never
/// rewrite the layer's media selection.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLayer {
    pub folder: Option<u8>,
    pub file: Option<u8>,
    pub dimmer: Option<f32>,
}

impl UpdateLayer {
    /// The address this update selects, given what the layer already points at.
    ///
    /// Either component may be changed on its own, which is how a desk-style folder-then-file
    /// selection works.
    pub const fn address(&self, current: MediaAddress) -> MediaAddress {
        MediaAddress::new(
            match self.folder {
                Some(folder) => folder,
                None => current.folder,
            },
            match self.file {
                Some(file) => file,
                None => current.file,
            },
        )
    }

    pub const fn changes_address(&self) -> bool {
        self.folder.is_some() || self.file.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_leaves_out_what_it_does_not_change() {
        let body: UpdateLayer = serde_json::from_str(r#"{"dimmer":0.5}"#).unwrap();
        assert_eq!(body.dimmer, Some(0.5));
        assert_eq!(body.folder, None);
        assert!(
            !body.changes_address(),
            "a dimmer change must not touch the selection"
        );
    }

    #[test]
    fn either_half_of_an_address_can_change_on_its_own() {
        let current = MediaAddress::new(3, 7);
        let folder_only: UpdateLayer = serde_json::from_str(r#"{"folder":5}"#).unwrap();
        assert_eq!(folder_only.address(current), MediaAddress::new(5, 7));

        let file_only: UpdateLayer = serde_json::from_str(r#"{"file":9}"#).unwrap();
        assert_eq!(file_only.address(current), MediaAddress::new(3, 9));

        let both: UpdateLayer = serde_json::from_str(r#"{"folder":1,"file":2}"#).unwrap();
        assert_eq!(both.address(current), MediaAddress::new(1, 2));
    }

    #[test]
    fn an_empty_update_changes_nothing() {
        let body: UpdateLayer = serde_json::from_str("{}").unwrap();
        assert_eq!(body, UpdateLayer::default());
        assert_eq!(
            body.address(MediaAddress::new(4, 4)),
            MediaAddress::new(4, 4)
        );
    }
}
