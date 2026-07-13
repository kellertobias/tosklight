use axum::{Json, Router, extract::Path, http::{StatusCode, header}, response::{IntoResponse, Response}, routing::get};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::path::{Component, Path as FsPath, PathBuf};

#[derive(RustEmbed)]
#[folder = "../../docs/help"]
struct EmbeddedHelp;

#[derive(Clone, Debug, Serialize)]
struct HelpTopicSummary { id: String, title: String }
#[derive(Debug, Serialize)]
struct HelpCatalog { topics: Vec<HelpTopicSummary>, errors: Vec<String>, live: bool }
#[derive(Debug, Serialize)]
struct HelpTopic { id: String, title: String, markdown: String, live: bool }

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/help", get(catalog))
        .route("/api/v1/help/topics/{id}", get(topic))
        .route("/api/v1/help/assets/{*path}", get(asset))
}

fn live_help_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) { return None; }
    Some(std::env::var_os("LIGHT_HELP_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/help")))
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty() && FsPath::new(path).components().all(|part| matches!(part, Component::Normal(_)))
}

fn markdown_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| line.strip_prefix("# ").map(str::trim).filter(|title| !title.is_empty()).map(str::to_owned))
}

fn read_live_file(root: &FsPath, relative: &str) -> Result<Vec<u8>, std::io::Error> {
    let root = root.canonicalize()?;
    let path = root.join(relative).canonicalize()?;
    if !path.starts_with(&root) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "help path escapes its root"));
    }
    std::fs::read(path)
}

fn markdown_files() -> Result<Vec<(String, String)>, String> {
    if let Some(root) = live_help_dir() {
        let entries = std::fs::read_dir(&root).map_err(|error| format!("Unable to read {}: {error}", root.display()))?;
        let mut files = entries.filter_map(Result::ok).filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|value| value.to_str()) == Some("md")).then_some(path)
        }).filter_map(|path| {
            let id = path.file_name()?.to_str()?.to_owned();
            read_live_file(&root, &id).ok().and_then(|bytes| String::from_utf8(bytes).ok()).map(|markdown| (id, markdown))
        }).collect::<Vec<_>>();
        files.sort_by(|left, right| left.0.cmp(&right.0));
        return Ok(files);
    }
    let mut files = EmbeddedHelp::iter()
        .filter(|path| path.ends_with(".md") && !path.contains('/'))
        .filter_map(|path| EmbeddedHelp::get(path.as_ref()).map(|asset| (path.into_owned(), asset)))
        .filter_map(|(id, asset)| String::from_utf8(asset.data.into_owned()).ok().map(|markdown| (id, markdown)))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

async fn catalog() -> Response {
    let live = live_help_dir().is_some();
    let files = match markdown_files() { Ok(files) => files, Err(error) => return (StatusCode::INTERNAL_SERVER_ERROR, error).into_response() };
    let mut topics = Vec::new();
    let mut errors = Vec::new();
    for (id, markdown) in files {
        if let Some(title) = markdown_title(&markdown) { topics.push(HelpTopicSummary { id, title }); }
        else { errors.push(format!("{id} is missing a first-level '# Title' heading")); }
    }
    Json(HelpCatalog { topics, errors, live }).into_response()
}

async fn topic(Path(id): Path<String>) -> Response {
    if !safe_relative(&id) || !id.ends_with(".md") { return StatusCode::BAD_REQUEST.into_response(); }
    let live = live_help_dir().is_some();
    let markdown = if let Some(root) = live_help_dir() {
        match read_live_file(&root, &id).and_then(|bytes| String::from_utf8(bytes).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))) {
            Ok(markdown) => markdown,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return StatusCode::NOT_FOUND.into_response(),
            Err(error) => return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
        }
    } else {
        let Some(asset) = EmbeddedHelp::get(&id) else { return StatusCode::NOT_FOUND.into_response() };
        let Ok(markdown) = String::from_utf8(asset.data.into_owned()) else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
        markdown
    };
    let Some(title) = markdown_title(&markdown) else { return (StatusCode::UNPROCESSABLE_ENTITY, "Help topic is missing a first-level '# Title' heading").into_response() };
    Json(HelpTopic { id, title, markdown, live }).into_response()
}

async fn asset(Path(path): Path<String>) -> Response {
    if !safe_relative(&path) || path.ends_with(".md") { return StatusCode::BAD_REQUEST.into_response(); }
    let bytes = if let Some(root) = live_help_dir() {
        match read_live_file(&root, &path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return StatusCode::NOT_FOUND.into_response(),
            Err(error) => return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
        }
    } else {
        let Some(asset) = EmbeddedHelp::get(&path) else { return StatusCode::NOT_FOUND.into_response() };
        asset.data.into_owned()
    };
    let content_type = match FsPath::new(&path).extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "webp" => "image/webp",
        "svg" => "image/svg+xml", "avif" => "image/avif", _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_first_h1_and_rejects_unsafe_paths() {
        assert_eq!(markdown_title("intro\n# Command line\ntext").as_deref(), Some("Command line"));
        assert!(safe_relative("images/console.png"));
        assert!(!safe_relative("../secret"));
        assert!(!safe_relative("/absolute"));
    }
    #[test]
    fn embedded_help_contains_command_line_topic() {
        assert!(EmbeddedHelp::iter().any(|path| path == "01-command-line.md"));
    }
}
