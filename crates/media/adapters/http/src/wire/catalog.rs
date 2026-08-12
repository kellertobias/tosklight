//! The library, as the API reports it.

use media_domain::catalog::{CatalogItem, CatalogSnapshot, ItemKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
    pub folder: u16,
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
