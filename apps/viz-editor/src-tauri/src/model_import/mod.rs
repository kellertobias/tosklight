//! Venue model files in formats other than GLB, turned into one self-contained GLB at import.
//!
//! Every renderer reads a venue model as a GLB, and the show keeps it as one. So a glTF with its
//! buffers and textures beside it, a 3MF from a CAD or print tool, or an OBJ with its material
//! library is converted here once, when it is imported, and nothing downstream learns a new format.

mod glb_builder;
mod gltf;
mod obj;
mod three_mf;

#[cfg(test)]
mod tests;

use light_fixture::MAX_FIXTURE_MODEL_BYTES;

/// The file extensions a venue model may be imported from, lower case and without the dot.
pub const VENUE_MODEL_EXTENSIONS: &[&str] = &["glb", "gltf", "3mf", "obj"];

/// Why a model file could not be turned into a GLB, naming the file it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError {
    pub file: String,
    pub message: String,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.file, self.message)
    }
}

impl std::error::Error for ImportError {}

/// Turn the bytes of a model file into a self-contained GLB 2.0, choosing the reader by extension.
///
/// `file_name` names the file for its extension and for every message. `read_sibling` reads a file
/// the model refers to — a glTF buffer or texture, an OBJ material library — by the relative path
/// the model spells, resolved beside the model file.
pub fn to_glb(
    file_name: &str,
    bytes: &[u8],
    read_sibling: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, ImportError> {
    let error = |message: String| ImportError {
        file: file_name.to_owned(),
        message,
    };
    let extension = std::path::Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let glb = match extension.as_str() {
        "glb" => bytes.to_vec(),
        "gltf" => gltf::embed(bytes, &read_sibling).map_err(error)?,
        "obj" => obj::convert(bytes, &read_sibling).map_err(error)?,
        "3mf" => three_mf::convert(bytes).map_err(error)?,
        _ => {
            return Err(error(format!(
                "{} is not a 3D model format that can be imported; choose a {} file",
                if extension.is_empty() {
                    "a file without an extension".to_owned()
                } else {
                    format!(".{extension}")
                },
                VENUE_MODEL_EXTENSIONS
                    .iter()
                    .map(|extension| format!(".{extension}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    };
    if glb.len() > MAX_FIXTURE_MODEL_BYTES {
        return Err(error(format!(
            "the model is {} MB once its files are packed together; a 3D model may be at most {} MB",
            glb.len().div_ceil(1024 * 1024),
            MAX_FIXTURE_MODEL_BYTES / (1024 * 1024)
        )));
    }
    Ok(glb)
}

/// Read a sibling the model names, with a message that says which one and why it matters.
fn sibling(
    read_sibling: &impl Fn(&str) -> Result<Vec<u8>, String>,
    relative: &str,
    purpose: &str,
) -> Result<Vec<u8>, String> {
    read_sibling(relative).map_err(|reason| {
        format!("{purpose} {relative} must be beside the model file, but it could not be read ({reason})")
    })
}
