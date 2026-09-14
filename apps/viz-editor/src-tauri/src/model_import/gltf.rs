//! A glTF JSON file with its buffers and images packed into one GLB.

use super::glb_builder::{pad, write_container};
use super::sibling;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;

/// Extensions a file may require that change nothing about what the reader draws.
const HARMLESS_REQUIRED_EXTENSIONS: &[&str] = &[
    "KHR_materials_unlit",
    "KHR_texture_transform",
    "KHR_materials_emissive_strength",
];

/// Resolve every external or data-URI buffer and image and write the document as one GLB.
///
/// Materials, textures, nodes and accessors are kept as authored; only where their bytes live
/// changes — every buffer view now points into one binary chunk.
pub fn embed(
    bytes: &[u8],
    read_sibling: &impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut document: Value = serde_json::from_slice(text)
        .map_err(|error| format!("the file is not valid glTF JSON ({error})"))?;
    if document
        .pointer("/asset/version")
        .and_then(Value::as_str)
        .is_none_or(|version| !version.starts_with("2."))
    {
        return Err("only glTF 2.0 files can be imported".into());
    }
    refuse_required_extensions(&document)?;

    let mut binary = Vec::new();
    let offsets = pack_buffers(&mut document, &mut binary, read_sibling)?;
    if let Some(views) = document
        .get_mut("bufferViews")
        .and_then(Value::as_array_mut)
    {
        for (index, view) in views.iter_mut().enumerate() {
            let buffer = view.get("buffer").and_then(Value::as_u64).unwrap_or(0) as usize;
            let base = *offsets
                .get(buffer)
                .ok_or_else(|| format!("buffer view {index} refers to a missing buffer"))?;
            let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
            view["buffer"] = 0.into();
            view["byteOffset"] = (base + offset).into();
        }
    }
    pack_images(&mut document, &mut binary, read_sibling)?;
    if binary.is_empty() {
        document.as_object_mut().map(|root| root.remove("buffers"));
    } else {
        document["buffers"] = serde_json::json!([{ "byteLength": binary.len() }]);
    }
    Ok(write_container(&document, &binary))
}

fn refuse_required_extensions(document: &Value) -> Result<(), String> {
    let unsupported: Vec<&str> = document
        .get("extensionsRequired")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .filter(|name| !HARMLESS_REQUIRED_EXTENSIONS.contains(name))
        .collect();
    if unsupported.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the file requires the glTF extension {}, which cannot be drawn; export it again without \
         mesh compression (Draco or meshopt) or other required extensions",
        unsupported.join(", ")
    ))
}

/// Append every buffer to `binary`, returning where each one starts.
fn pack_buffers(
    document: &mut Value,
    binary: &mut Vec<u8>,
    read_sibling: &impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<usize>, String> {
    let mut offsets = Vec::new();
    let Some(buffers) = document.get("buffers").and_then(Value::as_array) else {
        return Ok(offsets);
    };
    for (index, buffer) in buffers.iter().enumerate() {
        let uri = buffer.get("uri").and_then(Value::as_str).ok_or_else(|| {
            format!("buffer {index} has no uri; a .gltf file must name where its data is")
        })?;
        let (data, _) = resolve_uri(uri, "buffer", read_sibling)?;
        let length = buffer
            .get("byteLength")
            .and_then(Value::as_u64)
            .unwrap_or(data.len() as u64) as usize;
        if data.len() < length {
            return Err(format!(
                "buffer {} holds {} bytes but the file says it has {length}",
                describe_uri(uri),
                data.len()
            ));
        }
        // Eight-byte alignment keeps every accessor of every component size aligned after the move.
        while !binary.len().is_multiple_of(8) {
            binary.push(0);
        }
        offsets.push(binary.len());
        binary.extend_from_slice(&data[..length]);
    }
    Ok(offsets)
}

/// Move every image with a uri into its own buffer view.
fn pack_images(
    document: &mut Value,
    binary: &mut Vec<u8>,
    read_sibling: &impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let Some(images) = document.get("images").and_then(Value::as_array).cloned() else {
        return Ok(());
    };
    let mut packed = Vec::with_capacity(images.len());
    for mut image in images {
        if let Some(uri) = image.get("uri").and_then(Value::as_str).map(str::to_owned) {
            let (data, mime) = resolve_uri(&uri, "texture", read_sibling)?;
            pad(binary, 0);
            let view = serde_json::json!({
                "buffer": 0, "byteOffset": binary.len(), "byteLength": data.len()
            });
            binary.extend_from_slice(&data);
            let views = document
                .as_object_mut()
                .expect("a glTF document is an object")
                .entry("bufferViews")
                .or_insert_with(|| Value::Array(Vec::new()));
            let views = views.as_array_mut().ok_or("bufferViews is not a list")?;
            views.push(view);
            image["bufferView"] = (views.len() - 1).into();
            let mime = image
                .get("mimeType")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or(mime)
                .or_else(|| mime_from_extension(&uri).map(str::to_owned))
                .unwrap_or_else(|| "image/png".into());
            image["mimeType"] = mime.into();
            image.as_object_mut().map(|entry| entry.remove("uri"));
        }
        packed.push(image);
    }
    document["images"] = packed.into();
    Ok(())
}

/// The bytes a uri names, and the MIME type a data uri declares.
fn resolve_uri(
    uri: &str,
    purpose: &str,
    read_sibling: &impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<(Vec<u8>, Option<String>), String> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (header, payload) = rest
            .split_once(',')
            .ok_or_else(|| format!("a {purpose} data URI has no data"))?;
        let mime = header
            .split(';')
            .next()
            .filter(|mime| !mime.is_empty())
            .map(str::to_owned);
        let data = if header.ends_with(";base64") {
            STANDARD
                .decode(payload.trim())
                .map_err(|error| format!("a {purpose} data URI is not valid base64 ({error})"))?
        } else {
            percent_decode(payload).into_bytes()
        };
        return Ok((data, mime));
    }
    if uri.contains("://") {
        return Err(format!(
            "{purpose} {uri} is on the network; a model's files must be beside it on this computer"
        ));
    }
    let relative = percent_decode(uri);
    Ok((sibling(read_sibling, &relative, purpose)?, None))
}

fn describe_uri(uri: &str) -> String {
    if uri.starts_with("data:") {
        "(embedded)".into()
    } else {
        percent_decode(uri)
    }
}

fn mime_from_extension(uri: &str) -> Option<&'static str> {
    let extension = uri.rsplit('.').next()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ktx2" => "image/ktx2",
        _ => return None,
    })
}

/// Undo URI percent-encoding, as glTF writers apply to spaces and non-ASCII names.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(value)) => {
                decoded.push(value);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
