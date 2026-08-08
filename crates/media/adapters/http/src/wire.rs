//! The typed wire contract.
//!
//! Request types are tolerant of unknown fields; response types are explicit. Nothing here is a
//! hand-built JSON string, and nothing re-derives a domain rule — the API projects state, it does
//! not decide it.
//!
//! Every type here is a *projection*, owned by this adapter. Domain types are deliberately not
//! serialized straight onto the wire: a domain refactor must not silently become a breaking API
//! change, and the TypeScript the frontend consumes is generated from exactly these declarations
//! by `cargo run -p media-http --example generate-contracts`.

use media_domain::catalog::{CatalogItem, CatalogSnapshot, ItemKind};
use media_domain::visualizer::{GeneratedCatalog, VisualizerConfiguration, VisualizerParameters};
use media_domain::{
    LayerState, MaskSource, MaskState, MasterState, MediaAddress, OutputState, SourceFailure,
    SourceStatus,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether the process is up and what it is running.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub status: String,
    pub instance: String,
    pub outputs: usize,
    /// A revision counter, not an identifier. It stays inside the range a browser can hold in a
    /// number, so the client is not forced into `bigint` arithmetic to compare two snapshots.
    #[ts(type = "number")]
    pub catalog_revision: u64,
    pub catalog_items: usize,
}

/// A `(folder, file)` selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddressView {
    pub folder: u8,
    pub file: u8,
    /// Which address space the pair falls in, so the UI can label a selection without
    /// re-implementing the ranges.
    pub class: String,
}

impl AddressView {
    pub fn of(address: MediaAddress) -> Self {
        Self {
            folder: address.folder,
            file: address.file,
            class: match address.classify() {
                media_domain::AddressClass::Blank => "blank",
                media_domain::AddressClass::Library => "library",
                media_domain::AddressClass::TextBank => "text-bank",
                media_domain::AddressClass::GeneratedVisualizer => "generated-visualizer",
            }
            .to_owned(),
        }
    }
}

/// A layer's source lifecycle, flattened for a client that only wants to render a badge.
///
/// `failure` carries operator-safe text only. Absolute paths and decoder internals stay in the
/// log, which is where someone diagnosing a machine looks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatusView {
    pub state: String,
    pub failure: Option<String>,
}

impl SourceStatusView {
    pub fn of(status: SourceStatus) -> Self {
        let (state, failure) = match status {
            SourceStatus::Unselected => ("unselected", None),
            SourceStatus::Loading => ("loading", None),
            SourceStatus::Ready => ("ready", None),
            SourceStatus::Completed => ("completed", None),
            SourceStatus::Failed { failure } => ("failed", Some(describe(failure))),
        };
        Self {
            state: state.to_owned(),
            failure,
        }
    }
}

/// A layer's mask, as the API reports it.
///
/// Reported even when it is doing nothing, because "a mask is selected but faded out" and "no mask
/// is selected" are different situations an operator needs to tell apart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct MaskView {
    pub address: AddressView,
    pub scale_x: f32,
    pub scale_y: f32,
    pub invert: bool,
    pub opacity: f32,
    /// `alpha` or `luminance`.
    pub source: String,
    /// Whether it is currently shaping the layer at all.
    pub active: bool,
}

impl MaskView {
    pub fn of(mask: &MaskState) -> Self {
        Self {
            address: AddressView::of(mask.address),
            scale_x: mask.scale_x,
            scale_y: mask.scale_y,
            invert: mask.invert,
            opacity: mask.opacity,
            source: match mask.source {
                MaskSource::Alpha => "alpha",
                MaskSource::Luminance => "luminance",
            }
            .to_owned(),
            active: mask.is_active(),
        }
    }
}

/// One layer, as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayerView {
    pub index: usize,
    pub address: AddressView,
    pub play_mode: String,
    pub dimmer: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub grayscale: f32,
    pub source_status: SourceStatusView,
    pub mask: MaskView,
    /// Whether this layer contributes pixels right now.
    pub drawing: bool,
}

impl LayerView {
    pub fn of(index: usize, layer: &LayerState) -> Self {
        Self {
            index,
            address: AddressView::of(layer.address),
            play_mode: layer.play_mode.label().to_owned(),
            dimmer: layer.dimmer,
            scale_x: layer.scale_x,
            scale_y: layer.scale_y,
            position_x: layer.position_x,
            position_y: layer.position_y,
            rotation: layer.rotation,
            grayscale: layer.grayscale,
            source_status: SourceStatusView::of(layer.source_status),
            mask: MaskView::of(&layer.mask),
            drawing: layer.draws(),
        }
    }
}

/// The section that applies to the finished composite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct MasterView {
    pub dimmer: f32,
    pub volume: f32,
    pub tint_red: f32,
    pub tint_green: f32,
    pub tint_blue: f32,
    pub flip_mirror: String,
    pub mask: AddressView,
}

impl MasterView {
    pub fn of(master: MasterState) -> Self {
        Self {
            dimmer: master.dimmer,
            volume: master.volume,
            tint_red: master.tint.red,
            tint_green: master.tint.green,
            tint_blue: master.tint.blue,
            flip_mirror: match master.flip_mirror {
                media_domain::FlipMirror::None => "none",
                media_domain::FlipMirror::Horizontal => "horizontal",
                media_domain::FlipMirror::Vertical => "vertical",
                media_domain::FlipMirror::Both => "both",
            }
            .to_owned(),
            mask: AddressView::of(master.mask),
        }
    }
}

/// One output's whole state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutputView {
    pub id: String,
    pub name: String,
    pub layer_count: usize,
    pub layers: Vec<LayerView>,
    pub master: MasterView,
    /// Whether an external desk currently owns this output's continuously controlled values.
    pub dmx_active: bool,
}

impl OutputView {
    pub fn of(output: &OutputState, name: String, dmx_active: bool) -> Self {
        Self {
            id: output.id.to_string(),
            name,
            layer_count: usize::from(output.personality.layer_count()),
            layers: output
                .layers
                .iter()
                .enumerate()
                .map(|(index, layer)| LayerView::of(index, layer))
                .collect(),
            master: MasterView::of(output.master),
            dmx_active,
        }
    }
}

/// One addressable library item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItemView {
    /// Stable across renames, moves, and reindexing — the identity a UI keys a row on.
    pub id: String,
    pub file: u8,
    pub name: String,
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub frames: Option<u32>,
    pub intrinsic_bpm: Option<f64>,
}

impl CatalogItemView {
    pub fn of(item: &CatalogItem) -> Self {
        Self {
            id: item.id.to_string(),
            file: item.file,
            name: item.name.clone(),
            kind: match item.kind {
                ItemKind::Image => "image",
                ItemKind::Video => "video",
            }
            .to_owned(),
            width: item.width,
            height: item.height,
            frames: item.frames,
            intrinsic_bpm: item.intrinsic_bpm,
        }
    }
}

/// One library folder and everything addressable in it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFolderView {
    pub folder: u8,
    pub name: Option<String>,
    pub items: Vec<CatalogItemView>,
}

/// The library, as the API reports it. Projected from the same immutable snapshot the renderer
/// reads, so the picker can never show something the compositor cannot resolve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    #[ts(type = "number")]
    pub revision: u64,
    pub item_count: usize,
    pub folders: Vec<CatalogFolderView>,
}

impl CatalogView {
    pub fn of(snapshot: &CatalogSnapshot) -> Self {
        Self {
            revision: snapshot.revision.value(),
            item_count: snapshot.item_count(),
            folders: snapshot
                .folders
                .iter()
                .map(|folder| CatalogFolderView {
                    folder: folder.folder,
                    name: folder.name.clone(),
                    items: folder.items.iter().map(CatalogItemView::of).collect(),
                })
                .collect(),
        }
    }
}

/// One configured generated visualizer at the address a desk reaches it by.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerView {
    pub address: AddressView,
    /// Stable across releases and across a reassignment of the address.
    pub type_id: u16,
    /// The kind's own name, which is what documentation and a cue sheet call it.
    pub kind: String,
    /// What this configuration is called, which an operator may change.
    pub name: String,
    /// Which of the shared parameters this kind reads. The rest are present and ignored, so an
    /// editor can show only the controls that do something.
    pub uses: Vec<String>,
    pub parameters: VisualizerParametersView,
}

/// The shared parameter block, as the API reports it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerParametersView {
    pub count: u32,
    pub size: f32,
    pub speed: f32,
    pub amount: f32,
    pub radius: f32,
    pub thickness: f32,
    pub reactivity: f32,
    pub decay: f32,
    pub zoom: f32,
    pub iterations: u32,
    pub threshold: f32,
    pub smoothing: f32,
    pub gravity: f32,
    pub lifetime: f32,
    pub curvature: f32,
    pub primary_red: f32,
    pub primary_green: f32,
    pub primary_blue: f32,
    pub secondary_red: f32,
    pub secondary_green: f32,
    pub secondary_blue: f32,
    pub mirror: bool,
    pub filled: bool,
    pub wireframe: bool,
    pub mode: u8,
}

impl VisualizerParametersView {
    pub fn of(parameters: &VisualizerParameters) -> Self {
        Self {
            count: parameters.count,
            size: parameters.size,
            speed: parameters.speed,
            amount: parameters.amount,
            radius: parameters.radius,
            thickness: parameters.thickness,
            reactivity: parameters.reactivity,
            decay: parameters.decay,
            zoom: parameters.zoom,
            iterations: parameters.iterations,
            threshold: parameters.threshold,
            smoothing: parameters.smoothing,
            gravity: parameters.gravity,
            lifetime: parameters.lifetime,
            curvature: parameters.curvature,
            primary_red: parameters.primary.red,
            primary_green: parameters.primary.green,
            primary_blue: parameters.primary.blue,
            secondary_red: parameters.secondary.red,
            secondary_green: parameters.secondary.green,
            secondary_blue: parameters.secondary.blue,
            mirror: parameters.mirror,
            filled: parameters.filled,
            wireframe: parameters.wireframe,
            mode: parameters.mode,
        }
    }
}

impl VisualizerView {
    pub fn of(address: MediaAddress, configuration: &VisualizerConfiguration) -> Self {
        Self {
            address: AddressView::of(address),
            type_id: configuration.kind.type_id(),
            kind: configuration.kind.label().to_owned(),
            name: configuration.name.clone(),
            uses: configuration
                .kind
                .parameters()
                .iter()
                .map(|parameter| {
                    serde_json::to_value(parameter)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default()
                })
                .collect(),
            parameters: VisualizerParametersView::of(&configuration.parameters),
        }
    }

    /// Every assignment, in address order.
    pub fn all(catalog: &GeneratedCatalog) -> Vec<Self> {
        catalog
            .entries
            .iter()
            .map(|entry| Self::of(entry.address, &entry.configuration))
            .collect()
    }
}

/// An intent-shaped layer update: only the fields being changed.
///
/// Absent means "leave alone", which is why every field is optional. Sending a dimmer must never
/// rewrite the layer's media selection.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLayer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// Operator-safe text for a source failure.
///
/// The domain names the failure; wording it for a person is this adapter's job, and the wording
/// says what to do about it rather than what the decoder saw.
fn describe(failure: SourceFailure) -> String {
    match failure {
        SourceFailure::MissingFile => "the file is not in the library folder",
        SourceFailure::UnsupportedCodec => "this file is not in a playable format; import it",
        SourceFailure::DecodeFailed => "the file could not be decoded; it may be damaged",
        SourceFailure::GpuUploadFailed => "the graphics device rejected this frame",
    }
    .to_owned()
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

    #[test]
    fn an_address_carries_the_space_it_falls_in() {
        assert_eq!(AddressView::of(MediaAddress::BLANK).class, "blank");
        assert_eq!(AddressView::of(MediaAddress::new(1, 1)).class, "library");
        assert_eq!(
            AddressView::of(MediaAddress::new(200, 1)).class,
            "text-bank"
        );
        assert_eq!(
            AddressView::of(MediaAddress::new(220, 1)).class,
            "generated-visualizer"
        );
    }

    #[test]
    fn a_failed_source_reports_operator_safe_text_and_nothing_else() {
        let ready = SourceStatusView::of(SourceStatus::Ready);
        assert_eq!(ready.state, "ready");
        assert_eq!(ready.failure, None);

        let failed = SourceStatusView::of(SourceStatus::Failed {
            failure: SourceFailure::MissingFile,
        });
        assert_eq!(failed.state, "failed");
        assert!(
            failed.failure.is_some_and(|text| !text.is_empty()),
            "a failed layer must say something an operator can act on"
        );
    }
}
