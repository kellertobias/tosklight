//! MVR archive preview, import, export, and operator resolutions.

use super::*;

#[tauri::command]
pub fn export_mvr(session: tauri::State<'_, Session>, path: String) -> Answer<usize> {
    session.with(|document| {
        let export = document.export_mvr().map_err(|error| error.to_string())?;
        std::fs::write(Path::new(&path), &export.data).map_err(|error| error.to_string())?;
        Ok(export.summary.fixtures)
    })
}

/// What the archive holds and how it lands here, before anything is written.
#[tauri::command]
pub fn preview_mvr(session: tauri::State<'_, Session>, path: String) -> Answer<MvrPreviewDto> {
    session.prepare_mvr(&path)
}

/// Imports the archive, with whatever the operator decided about the fixtures that needed a
/// decision. A fixture with no decision keeps the desk's own default handling.
#[tauri::command]
pub fn import_mvr(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    token: String,
    resolutions: HashMap<String, ResolutionDto>,
) -> Answer<MvrImportReport> {
    let report = session.apply_mvr(&token, decode_resolutions(resolutions)?)?;
    Ok(super::mvr_preview::after_notification(
        report,
        announce_document_change(&app, &window),
    ))
}

#[tauri::command]
pub fn cancel_mvr_preview(session: tauri::State<'_, Session>, token: String) {
    session.cancel_mvr(&token);
}

pub(super) fn read_archive(path: &str) -> Answer<light_mvr::MvrDocument> {
    let data = std::fs::read(Path::new(path)).map_err(|error| error.to_string())?;
    PlanningDocument::read_mvr(&data).map_err(|error| error.to_string())
}

/// The operator's decisions, as the import service understands them. An action it does not know
/// is refused rather than quietly treated as the default.
fn decode_resolutions(
    resolutions: HashMap<String, ResolutionDto>,
) -> Answer<HashMap<Uuid, MvrImportResolution>> {
    resolutions
        .into_iter()
        .map(|(uuid, resolution)| {
            let uuid = Uuid::parse_str(&uuid)
                .map_err(|error| format!("{uuid} is not a fixture identifier: {error}"))?;
            let decision = match resolution.action.as_str() {
                "import" => MvrImportResolution::Import,
                "skip" => MvrImportResolution::Skip,
                "import_unpatched" => MvrImportResolution::ImportUnpatched,
                "replace" => MvrImportResolution::Replace,
                "address" => MvrImportResolution::Address {
                    universe: resolution.universe.ok_or("an address needs a universe")?,
                    address: resolution.address.ok_or("an address needs an address")?,
                },
                other => return Err(format!("{other} is not an import resolution")),
            };
            Ok((uuid, decision))
        })
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct ResolutionDto {
    pub action: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvrPreviewDto {
    pub token: String,
    pub warnings: Vec<String>,
    pub fixtures: Vec<MvrPreviewFixtureDto>,
    pub scenery: usize,
    pub missing_profiles: Vec<String>,
    pub address_conflicts: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvrPreviewFixtureDto {
    pub uuid: String,
    pub name: String,
    pub gdtf_spec: String,
    pub gdtf_mode: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    pub matched: bool,
    pub conflicted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvrImportReport {
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub warnings: Vec<String>,
}
