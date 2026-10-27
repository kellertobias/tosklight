//! Portable CAD paperwork. One custom object preserves page order without changing the show schema.
use crate::session::Session;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::Emitter;
use uuid::Uuid;
use viz_document::PlanningDocument;

type Answer<T> = Result<T, String>;
const KIND: &str = "cad_print_pages";
const ID: &str = "pages";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutPlanes {
    pub near_millimetres: Option<f64>,
    pub far_millimetres: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadPrintPage {
    #[serde(default = "plan")]
    pub kind: String,
    pub id: String,
    pub tile_id: String,
    pub name: String,
    pub view: String,
    #[serde(default)]
    pub rotation_quarter_turns: i32,
    pub centre_millimetres: [f64; 2],
    pub width_millimetres: f64,
    pub included: bool,
    pub orientation: String,
    #[serde(default)]
    pub show_fixture_ids: bool,
    #[serde(default)]
    pub show_dmx_addresses: bool,
    #[serde(default = "yes")]
    pub show_mounting_hardware: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_planes: Option<CutPlanes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_underlay_ids: Option<Vec<String>>,
    /// Preserve additive fields written by a newer Architect.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
fn plan() -> String {
    "plan".into()
}
fn yes() -> bool {
    true
}

fn guard(document: &PlanningDocument, show_id: Uuid) -> Answer<()> {
    if show_id.is_nil() || document.show_id().0 != show_id {
        return Err("The open show changed; reopen its print pages before saving.".into());
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPageSnapshot {
    pub show_id: Uuid,
    pub revision: u64,
    pub pages: Option<Vec<CadPrintPage>>,
}

fn read(document: &PlanningDocument) -> Answer<PrintPageSnapshot> {
    let object = document.object(KIND, ID).map_err(|e| e.to_string())?;
    let revision = object.as_ref().map_or(0, |object| object.revision);
    let pages = object
        .map(|object| {
            serde_json::from_value(object.body)
                .map_err(|e| format!("Print pages could not be read: {e}"))
        })
        .transpose()?;
    Ok(PrintPageSnapshot {
        show_id: document.show_id().0,
        revision,
        pages,
    })
}
fn validate(pages: &[CadPrintPage]) -> Answer<()> {
    let mut ids = HashSet::new();
    for page in pages {
        if page.id.trim().is_empty() || !ids.insert(&page.id) {
            return Err("Print pages need distinct, nonempty IDs.".into());
        }
        if !matches!(page.kind.as_str(), "plan" | "fixture_list")
            || !matches!(page.orientation.as_str(), "portrait" | "landscape")
            || !matches!(
                page.view.as_str(),
                "top_down" | "front_to_back" | "back_to_front" | "left_to_right" | "right_to_left"
            )
            || !page.width_millimetres.is_finite()
            || page.width_millimetres <= 0.0
            || !page.centre_millimetres.iter().all(|v| v.is_finite())
        {
            return Err(format!(
                "Print page {} has invalid view or paper geometry.",
                page.name
            ));
        }
        if let Some(cuts) = &page.cut_planes {
            if cuts
                .near_millimetres
                .into_iter()
                .chain(cuts.far_millimetres)
                .any(|v| !v.is_finite())
            {
                return Err(format!("Print page {} has invalid cut planes.", page.name));
            }
        }
    }
    Ok(())
}

pub(crate) fn store_pages(
    session: &Session,
    show_id: Uuid,
    pages: Vec<CadPrintPage>,
    only_if_absent: bool,
    expected_revision: u64,
) -> Answer<PrintPageSnapshot> {
    validate(&pages)?;
    session.change(|document| {
        guard(document, show_id)?;
        let existing = read(document)?;
        if only_if_absent && existing.pages.is_some() {
            return Ok(existing);
        }
        if expected_revision != existing.revision {
            return Err("Another window changed these print pages. Retry to merge your page edits with the latest show.".into());
        }
        let body = serde_json::to_value(&pages).map_err(|e| e.to_string())?;
        document
            .put_object(KIND, ID, &body)
            .map_err(|e| e.to_string())?;
        read(document)
    })
}

#[tauri::command]
pub fn cad_print_pages(
    session: tauri::State<'_, Session>,
    show_id: Uuid,
) -> Answer<PrintPageSnapshot> {
    session.with(|document| {
        guard(document, show_id)?;
        read(document)
    })
}

/// Initialization is conditional, so two windows cannot repeat workspace adoption.
#[tauri::command]
pub fn save_cad_print_pages(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    show_id: Uuid,
    pages: Vec<CadPrintPage>,
    only_if_absent: bool,
    expected_revision: u64,
) -> Answer<PrintPageSnapshot> {
    let snapshot = store_pages(&session, show_id, pages, only_if_absent, expected_revision)?;
    app.emit("cad-print-pages-delta", &snapshot)
        .map_err(|e| e.to_string())?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests;
