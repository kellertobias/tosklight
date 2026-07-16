use axum::{
    Json, Router,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Component, Path as FsPath, PathBuf},
};

#[derive(RustEmbed)]
#[folder = "../../docs/help"]
struct EmbeddedHelp;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum HelpEntryKind {
    Folder,
    Topic,
}
#[derive(Clone, Debug, Serialize)]
struct HelpCatalogEntry {
    id: Option<String>,
    title: String,
    kind: HelpEntryKind,
    children: Vec<HelpCatalogEntry>,
}
#[derive(Debug, Serialize)]
struct HelpCatalog {
    topics: Vec<HelpCatalogEntry>,
    errors: Vec<String>,
    live: bool,
}
#[derive(Debug, Serialize)]
struct HelpTopic {
    id: String,
    title: String,
    markdown: String,
    live: bool,
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/help", get(catalog))
        .route("/api/v1/help/topics/{*id}", get(topic))
        .route("/api/v1/help/assets/{*path}", get(asset))
}

fn live_help_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    Some(
        std::env::var_os("LIGHT_HELP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/help")),
    )
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && FsPath::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn is_markdown_path(path: &str) -> bool {
    matches!(
        FsPath::new(path)
            .extension()
            .and_then(|value| value.to_str()),
        Some("md" | "markdown")
    )
}

fn markdown_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned)
    })
}

fn read_live_file(root: &FsPath, relative: &str) -> Result<Vec<u8>, std::io::Error> {
    let root = root.canonicalize()?;
    let path = root.join(relative).canonicalize()?;
    if !path.starts_with(&root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "help path escapes its root",
        ));
    }
    std::fs::read(path)
}

fn collect_live_markdown(
    root: &FsPath,
    directory: &FsPath,
    files: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("Unable to read {}: {error}", directory.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_live_markdown(root, &path, files)?;
        } else if path.to_str().is_some_and(is_markdown_path) {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let id = relative
                .components()
                .filter_map(|component| component.as_os_str().to_str())
                .collect::<Vec<_>>()
                .join("/");
            if let Ok(bytes) = read_live_file(root, &id)
                && let Ok(markdown) = String::from_utf8(bytes)
            {
                files.push((id, markdown));
            }
        }
    }
    Ok(())
}

fn markdown_files() -> Result<Vec<(String, String)>, String> {
    if let Some(root) = live_help_dir() {
        let mut files = Vec::new();
        collect_live_markdown(&root, &root, &mut files)?;
        files.sort_by(|left, right| left.0.cmp(&right.0));
        return Ok(files);
    }
    let mut files = EmbeddedHelp::iter()
        .filter(|path| is_markdown_path(path))
        .filter_map(|path| EmbeddedHelp::get(path.as_ref()).map(|asset| (path.into_owned(), asset)))
        .filter_map(|(id, asset)| {
            String::from_utf8(asset.data.into_owned())
                .ok()
                .map(|markdown| (id, markdown))
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

#[derive(Default)]
struct HelpDirectory {
    index: Option<(String, String)>,
    files: BTreeMap<String, (String, String)>,
    directories: BTreeMap<String, HelpDirectory>,
}

impl HelpDirectory {
    fn insert(&mut self, id: String, title: String) {
        let path = id.clone();
        let parts = path.split('/').collect::<Vec<_>>();
        self.insert_parts(&parts, id, title);
    }

    fn insert_parts(&mut self, parts: &[&str], id: String, title: String) {
        match parts {
            ["index.md"] => self.index = Some((id, title)),
            [file] => {
                self.files.insert((*file).to_owned(), (id, title));
            }
            [directory, rest @ ..] => self
                .directories
                .entry((*directory).to_owned())
                .or_default()
                .insert_parts(rest, id, title),
            [] => {}
        }
    }

    fn entries(self, path: &str, errors: &mut Vec<String>) -> Vec<HelpCatalogEntry> {
        let mut entries = Vec::new();
        for (name, directory) in self.directories {
            let directory_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}/{name}")
            };
            let (id, title) = if let Some((id, title)) = directory.index.clone() {
                (Some(id), title)
            } else {
                errors.push(format!("{directory_path} is missing an index.md file"));
                (None, folder_fallback_title(&name))
            };
            let children = directory.entries(&directory_path, errors);
            entries.push((
                name,
                HelpCatalogEntry {
                    id,
                    title,
                    kind: HelpEntryKind::Folder,
                    children,
                },
            ));
        }
        entries.extend(self.files.into_iter().map(|(name, (id, title))| {
            (
                name,
                HelpCatalogEntry {
                    id: Some(id),
                    title,
                    kind: HelpEntryKind::Topic,
                    children: Vec::new(),
                },
            )
        }));
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }
}

fn folder_fallback_title(name: &str) -> String {
    name.trim_start_matches(|character: char| character.is_ascii_digit() || character == '-')
        .replace(['-', '_'], " ")
}

fn build_catalog(files: Vec<(String, String)>) -> (Vec<HelpCatalogEntry>, Vec<String>) {
    let mut root = HelpDirectory::default();
    let mut errors = Vec::new();
    for (id, markdown) in files {
        if let Some(title) = markdown_title(&markdown) {
            root.insert(id, title);
        } else {
            errors.push(format!("{id} is missing a first-level '# Title' heading"));
        }
    }
    let topics = root.entries("", &mut errors);
    (topics, errors)
}

async fn catalog() -> Response {
    let live = live_help_dir().is_some();
    let files = match markdown_files() {
        Ok(files) => files,
        Err(error) => return (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    };
    let (topics, errors) = build_catalog(files);
    Json(HelpCatalog {
        topics,
        errors,
        live,
    })
    .into_response()
}

async fn topic(Path(id): Path<String>) -> Response {
    if !safe_relative(&id) || !is_markdown_path(&id) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let live = live_help_dir().is_some();
    let markdown = if let Some(root) = live_help_dir() {
        match read_live_file(&root, &id).and_then(|bytes| {
            String::from_utf8(bytes)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        }) {
            Ok(markdown) => markdown,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return StatusCode::NOT_FOUND.into_response();
            }
            Err(error) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
            }
        }
    } else {
        let Some(asset) = EmbeddedHelp::get(&id) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Ok(markdown) = String::from_utf8(asset.data.into_owned()) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        markdown
    };
    let Some(title) = markdown_title(&markdown) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Help topic is missing a first-level '# Title' heading",
        )
            .into_response();
    };
    Json(HelpTopic {
        id,
        title,
        markdown,
        live,
    })
    .into_response()
}

async fn asset(Path(path): Path<String>) -> Response {
    if !safe_relative(&path) || is_markdown_path(&path) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let bytes = if let Some(root) = live_help_dir() {
        match read_live_file(&root, &path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return StatusCode::NOT_FOUND.into_response();
            }
            Err(error) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
            }
        }
    } else {
        let Some(asset) = EmbeddedHelp::get(&path) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        asset.data.into_owned()
    };
    let content_type = match FsPath::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_first_h1_and_rejects_unsafe_paths() {
        assert_eq!(
            markdown_title("intro\n# Command line\ntext").as_deref(),
            Some("Command line")
        );
        assert!(safe_relative("images/console.png"));
        assert!(!safe_relative("../secret"));
        assert!(!safe_relative("/absolute"));
        assert!(is_markdown_path("00-quickstart.markdown"));
        assert!(is_markdown_path("folder/index.md"));
    }
    #[test]
    fn builds_nested_catalog_from_folder_indexes() {
        let (topics, errors) = build_catalog(vec![
            ("00-quickstart.markdown".into(), "# Quickstart".into()),
            ("20-Show-Setup/index.md".into(), "# Show File Setup".into()),
            ("20-Show-Setup/01-patch.md".into(), "# Patch".into()),
            ("99-Development/index.md".into(), "# Development".into()),
            (
                "99-Development/01-open.md".into(),
                "# Open Questions".into(),
            ),
        ]);
        assert!(errors.is_empty());
        assert_eq!(topics.len(), 3);
        assert_eq!(topics[0].title, "Quickstart");
        assert_eq!(topics[0].id.as_deref(), Some("00-quickstart.markdown"));
        assert_eq!(topics[1].title, "Show File Setup");
        assert_eq!(topics[1].id.as_deref(), Some("20-Show-Setup/index.md"));
        assert_eq!(topics[1].children[0].title, "Patch");
        assert_eq!(topics[2].title, "Development");
    }
    #[test]
    fn embedded_help_contains_nested_command_line_topic() {
        assert!(EmbeddedHelp::iter().any(|path| path == "30-Programmer/01-command-line.md"));
    }
}
