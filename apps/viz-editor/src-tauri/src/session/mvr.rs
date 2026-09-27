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
    session.with(|document| {
        let archive = read_archive(&path)?;
        let preview = document
            .preview_mvr(&archive)
            .map_err(|error| error.to_string())?;
        Ok(MvrPreviewDto {
            fixtures: preview
                .fixtures
                .into_iter()
                .map(|fixture| MvrPreviewFixtureDto {
                    uuid: fixture.uuid.to_string(),
                    name: fixture.name,
                    gdtf_spec: fixture.gdtf_spec,
                    gdtf_mode: fixture.gdtf_mode,
                    universe: fixture.universe,
                    address: fixture.address,
                    matched: fixture.matched,
                    conflicted: fixture.conflicted,
                })
                .collect(),
            scenery: preview.scenery,
            missing_profiles: preview.missing_profiles,
            address_conflicts: preview.address_conflicts,
        })
    })
}

/// Imports the archive, with whatever the operator decided about the fixtures that needed a
/// decision. A fixture with no decision keeps the desk's own default handling.
#[tauri::command]
pub fn import_mvr(
    app: tauri::AppHandle,
    window: tauri::Window,
    session: tauri::State<'_, Session>,
    path: String,
    resolutions: HashMap<String, ResolutionDto>,
) -> Answer<MvrImportReport> {
    let resolutions = decode_resolutions(resolutions)?;
    let report = session.change(|document| {
        let archive = read_archive(&path)?;
        let outcome = document
            .import_mvr(archive, resolutions.clone())
            .map_err(|error| error.to_string())?;
        Ok(MvrImportReport {
            imported_fixtures: outcome.imported_fixtures,
            unresolved_fixtures: outcome.unresolved_fixtures,
            warnings: outcome.warnings,
        })
    })?;
    announce_document_change(&app, &window)?;
    Ok(report)
}

fn read_archive(path: &str) -> Answer<light_mvr::MvrDocument> {
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
