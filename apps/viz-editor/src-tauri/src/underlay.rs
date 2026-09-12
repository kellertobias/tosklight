//! Venue drawings placed under the CAD plan.
//!
//! A DXF or SVG is read once, at import, and what the show keeps is the drawing reduced to
//! millimetre polylines — not the source file. That keeps a portable show portable: a plan stays
//! with the document through save-as, backup and transfer to another machine, with no absolute
//! path to a file that may not travel with it, and no second parser on the drawing path.
//!
//! Each drawing is assigned to one CAD view, the axis it was drawn for: a ground plan belongs to
//! the top-down view, a section to an elevation. Placement is the operator's, in millimetres.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use uuid::Uuid;
use viz_drawing::{UnderlayGeometry, parse_drawing};

use crate::session::Session;

/// The stored-object kind a placed drawing lives in.
const KIND: &str = "cad_underlay";

/// Broadcast when a drawing is added, placed or removed, so every window redraws.
const UNDERLAY_DELTA_EVENT: &str = "cad-underlay-delta";

type Answer<T> = Result<T, String>;

/// One placed drawing, as the show stores it and the CAD screen draws it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadUnderlay {
    pub id: String,
    pub name: String,
    /// `dxf` or `svg`, for the operator to see what a drawing came from.
    pub source_format: String,
    /// The CAD view this drawing belongs to, as the frontend names it (`top_down`, `front_to_back`…).
    pub view: String,
    /// Where the drawing's own origin sits in the show, in millimetres.
    pub origin_millimetres: [f64; 2],
    /// Placement scale on top of the drawing's real size; 1 is the size it was drawn at.
    pub scale: f64,
    pub rotation_degrees: f64,
    /// Whether the CAD viewport draws it at all. Print pages hide drawings page by page.
    pub visible: bool,
    /// What the file said its units were, so the panel can say what was assumed.
    pub units: String,
    pub geometry: UnderlayGeometry,
}

/// What an import would produce, reported before anything is written.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadUnderlayPreview {
    pub name: String,
    pub source_format: String,
    pub units: String,
    pub polyline_count: usize,
    pub point_count: usize,
    /// `[minX, minY, maxX, maxY]` in millimetres.
    pub extents_millimetres: [f64; 4],
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Drawing")
        .to_owned()
}

fn format_of(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".svg") {
        "svg".to_owned()
    } else {
        "dxf".to_owned()
    }
}

fn read(path: &str) -> Answer<(String, UnderlayGeometry)> {
    let name = file_name(path);
    let bytes =
        std::fs::read(path).map_err(|error| format!("{name} could not be read: {error}"))?;
    let geometry = parse_drawing(&name, &bytes).map_err(|error| error.to_string())?;
    Ok((name, geometry))
}

fn point_count(geometry: &UnderlayGeometry) -> usize {
    geometry
        .polylines
        .iter()
        .map(|polyline| polyline.points.len())
        .sum()
}

fn store(session: &Session, underlay: &CadUnderlay) -> Answer<()> {
    let body = serde_json::to_value(underlay).map_err(|error| error.to_string())?;
    session.change(|document| {
        document
            .put_object(KIND, &underlay.id, &body)
            .map_err(|error| error.to_string())
    })
}

fn announce(app: &tauri::AppHandle, session: &Session) -> Answer<()> {
    let underlays = read_underlays(session)?;
    app.emit(UNDERLAY_DELTA_EVENT, underlays)
        .map_err(|error| error.to_string())
}

fn read_underlays(session: &Session) -> Answer<Vec<CadUnderlay>> {
    session.with(|document| {
        let stored = document.objects(KIND).map_err(|error| error.to_string())?;
        Ok(stored
            .into_iter()
            .filter_map(|object| serde_json::from_value::<CadUnderlay>(object.body).ok())
            .collect())
    })
}

/// Read a drawing and report what placing it would add, without writing anything.
#[tauri::command]
pub fn preview_cad_underlay(path: String) -> Answer<CadUnderlayPreview> {
    let (name, geometry) = read(&path)?;
    Ok(CadUnderlayPreview {
        source_format: format_of(&name),
        units: geometry.units.label().to_owned(),
        polyline_count: geometry.polylines.len(),
        point_count: point_count(&geometry),
        extents_millimetres: geometry.extents_millimetres,
        name,
    })
}

/// Place a drawing on one CAD view, storing its geometry in the show.
#[tauri::command]
pub fn import_cad_underlay(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    path: String,
    view: String,
) -> Answer<CadUnderlay> {
    let (name, geometry) = read(&path)?;
    let underlay = CadUnderlay {
        id: Uuid::new_v4().to_string(),
        source_format: format_of(&name),
        name,
        view,
        origin_millimetres: [0.0, 0.0],
        scale: 1.0,
        rotation_degrees: 0.0,
        visible: true,
        units: geometry.units.label().to_owned(),
        geometry,
    };
    store(&session, &underlay)?;
    announce(&app, &session)?;
    Ok(underlay)
}

/// The placed drawings the open show carries.
#[tauri::command]
pub fn cad_underlays(session: tauri::State<'_, Session>) -> Answer<Vec<CadUnderlay>> {
    read_underlays(&session)
}

/// Store one drawing's placement. The geometry travels with it, so a rename or a move is one write.
#[tauri::command]
pub fn save_cad_underlay(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    underlay: CadUnderlay,
) -> Answer<CadUnderlay> {
    if underlay.name.trim().is_empty() {
        return Err("a placed drawing needs a name".to_owned());
    }
    if !underlay.scale.is_finite() || underlay.scale <= 0.0 {
        return Err("a placed drawing needs a scale greater than zero".to_owned());
    }
    store(&session, &underlay)?;
    announce(&app, &session)?;
    Ok(underlay)
}

/// Remove one placed drawing from the show.
#[tauri::command]
pub fn delete_cad_underlay(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    id: String,
) -> Answer<()> {
    session.change(|document| {
        document
            .delete_object(KIND, &id)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    announce(&app, &session)
}
