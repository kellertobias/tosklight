use super::manifest::{
    AssetKind, FixturePackageError, MAX_GOBO_DIMENSION, MAX_ICON_DIMENSION,
    MAX_PHOTOGRAPH_DIMENSION, PackageAsset,
};
use super::{invalid, validate_glb};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader};
use quick_xml::{Reader, events::Event};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Component, Path};

pub(super) fn resolve_asset_field(
    value: &mut Option<String>,
    kind: AssetKind,
    files: &mut HashMap<String, Vec<u8>>,
) -> Result<(), FixturePackageError> {
    let Some(path) = value.as_deref() else {
        return Ok(());
    };
    validate_asset_path(path)?;
    let bytes = files
        .remove(path)
        .ok_or_else(|| invalid(format!("{} asset {path} is missing", kind.label())))?;
    let mime = validate_asset(kind, path, &bytes)?;
    *value = Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)));
    Ok(())
}

pub(super) fn extract_asset_field(
    value: &mut Option<String>,
    kind: AssetKind,
    stem: &str,
    assets: &mut Vec<PackageAsset>,
) -> Result<(), FixturePackageError> {
    let Some(data_url) = value.as_deref() else {
        return Ok(());
    };
    let (declared_mime, bytes) = decode_data_url(data_url, kind)?;
    if bytes.len() > kind.max_bytes() {
        return Err(invalid(format!("{} exceeds its size limit", kind.label())));
    }
    let extension = match kind {
        AssetKind::Photograph | AssetKind::Icon => {
            let format = sniff_image(&bytes)?;
            let mime = image_mime(format);
            if declared_mime != mime {
                return Err(invalid(format!(
                    "{} declares {declared_mime} but contains {mime}",
                    kind.label()
                )));
            }
            validate_image_dimensions(kind, &bytes, format)?;
            image_extension(format)
        }
        AssetKind::Model => {
            if !matches!(
                declared_mime,
                "model/gltf-binary" | "application/octet-stream"
            ) {
                return Err(invalid(format!(
                    "3D model has unsupported media type {declared_mime}"
                )));
            }
            validate_glb(&bytes)?;
            "glb"
        }
        AssetKind::ScanScript => {
            if !matches!(
                declared_mime,
                "text/javascript" | "application/javascript" | "application/ecmascript"
            ) {
                return Err(invalid(format!(
                    "scan script has unsupported media type {declared_mime}"
                )));
            }
            validate_scan_script(&bytes)?;
            "js"
        }
        AssetKind::EffectScript => {
            if !matches!(
                declared_mime,
                "text/javascript" | "application/javascript" | "application/ecmascript"
            ) {
                return Err(invalid(format!(
                    "effect script has unsupported media type {declared_mime}"
                )));
            }
            validate_effect_script(&bytes)?;
            "js"
        }
        AssetKind::PhysicsScript => {
            if !matches!(
                declared_mime,
                "text/javascript" | "application/javascript" | "application/ecmascript"
            ) {
                return Err(invalid(format!(
                    "physics control script has unsupported media type {declared_mime}"
                )));
            }
            validate_physics_script(&bytes)?;
            "js"
        }
        AssetKind::Gobo => {
            let format = sniff_image(&bytes)?;
            validate_image_dimensions(kind, &bytes, format)?;
            image_extension(format)
        }
        AssetKind::Projection => {
            if declared_mime != "image/svg+xml" {
                return Err(invalid(format!(
                    "SVG projection has unsupported media type {declared_mime}"
                )));
            }
            validate_projection_svg(&bytes)?;
            "svg"
        }
    };
    let path = format!("{stem}.{extension}");
    *value = Some(path.clone());
    assets.push(PackageAsset { path, bytes });
    Ok(())
}

pub(super) fn validate_inline_projection_set(
    profile: &crate::FixtureProfile,
) -> Result<(), FixturePackageError> {
    let Some(projections) = profile.projection_assets.as_ref() else {
        return Ok(());
    };
    let model = profile
        .model_asset
        .as_deref()
        .ok_or_else(|| invalid("SVG projections require their source 3D model"))?;
    let (_, model_bytes) = decode_data_url(model, AssetKind::Model)?;
    let actual_hash = Sha256::digest(&model_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual_hash != projections.source_model_sha256 {
        return Err(invalid(
            "SVG projections are stale for the profile's source 3D model",
        ));
    }
    for projection in &projections.views {
        let (mime, bytes) = decode_data_url(&projection.artwork_asset, AssetKind::Projection)?;
        if mime != "image/svg+xml" {
            return Err(invalid(format!(
                "SVG projection has unsupported media type {mime}"
            )));
        }
        validate_projection_svg_contract(&bytes, projection)?;
    }
    Ok(())
}

fn decode_data_url(value: &str, kind: AssetKind) -> Result<(&str, Vec<u8>), FixturePackageError> {
    let payload = value.strip_prefix("data:").ok_or_else(|| {
        invalid(format!(
            "{} must be a self-contained data URL",
            kind.label()
        ))
    })?;
    let (metadata, encoded) = payload
        .split_once(',')
        .ok_or_else(|| invalid(format!("{} data URL is malformed", kind.label())))?;
    let mime = metadata
        .strip_suffix(";base64")
        .ok_or_else(|| invalid(format!("{} data URL must use base64", kind.label())))?;
    if mime.is_empty() || mime.contains(';') {
        return Err(invalid(format!(
            "{} data URL media type is invalid",
            kind.label()
        )));
    }
    let maximum_encoded = kind.max_bytes().saturating_add(2) / 3 * 4;
    if encoded.len() > maximum_encoded {
        return Err(invalid(format!("{} exceeds its size limit", kind.label())));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| invalid(format!("{} data URL is invalid: {error}", kind.label())))?;
    Ok((mime, bytes))
}

fn validate_asset_path(path: &str) -> Result<(), FixturePackageError> {
    if path.contains('\0') || path.contains('\\') || path.starts_with('/') {
        return Err(invalid(format!("asset path {path} is unsafe")));
    }
    let path_value = Path::new(path);
    if !path.starts_with("assets/")
        || path_value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(format!("asset path {path} is unsafe")));
    }
    Ok(())
}

fn validate_asset(
    kind: AssetKind,
    path: &str,
    bytes: &[u8],
) -> Result<&'static str, FixturePackageError> {
    if bytes.is_empty() || bytes.len() > kind.max_bytes() {
        return Err(invalid(format!("{} exceeds its size limit", kind.label())));
    }
    match kind {
        AssetKind::Photograph | AssetKind::Icon => {
            let format = sniff_image(bytes)?;
            let expected = image_extension(format);
            let actual = Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let extension_matches =
                actual == expected || (format == ImageFormat::Jpeg && actual == "jpeg");
            if !extension_matches {
                return Err(invalid(format!(
                    "{} path extension does not match its content",
                    kind.label()
                )));
            }
            validate_image_dimensions(kind, bytes, format)?;
            Ok(image_mime(format))
        }
        AssetKind::Model => {
            if !path.to_ascii_lowercase().ends_with(".glb") {
                return Err(invalid("3D model must use the .glb extension"));
            }
            validate_glb(bytes)?;
            Ok("model/gltf-binary")
        }
        AssetKind::ScanScript => {
            if !path.to_ascii_lowercase().ends_with(".js") {
                return Err(invalid("scan script must use the .js extension"));
            }
            validate_scan_script(bytes)?;
            Ok("text/javascript")
        }
        AssetKind::EffectScript => {
            if !path.to_ascii_lowercase().ends_with(".js") {
                return Err(invalid("effect script must use the .js extension"));
            }
            validate_effect_script(bytes)?;
            Ok("text/javascript")
        }
        AssetKind::PhysicsScript => {
            if !path.to_ascii_lowercase().ends_with(".js") {
                return Err(invalid("physics control script must use the .js extension"));
            }
            validate_physics_script(bytes)?;
            Ok("text/javascript")
        }
        AssetKind::Gobo => {
            let format = sniff_image(bytes)?;
            validate_image_dimensions(kind, bytes, format)?;
            Ok(image_mime(format))
        }
        AssetKind::Projection => {
            if !path.to_ascii_lowercase().ends_with(".svg") {
                return Err(invalid("SVG projection must use the .svg extension"));
            }
            validate_projection_svg(bytes)?;
            Ok("image/svg+xml")
        }
    }
}

/// A scan script has to survive the trip to a scan engine as source text, so the only structural
/// guarantee a package can make is that it is text at all. Whether it exports a usable `scan`
/// function is settled where it is compiled, which is the one place that can say what went wrong.
fn validate_scan_script(bytes: &[u8]) -> Result<(), FixturePackageError> {
    std::str::from_utf8(bytes)
        .map_err(|error| invalid(format!("scan script is not valid UTF-8: {error}")))?;
    Ok(())
}

fn validate_effect_script(bytes: &[u8]) -> Result<(), FixturePackageError> {
    std::str::from_utf8(bytes)
        .map_err(|error| invalid(format!("effect script is not valid UTF-8: {error}")))?;
    Ok(())
}

fn validate_physics_script(bytes: &[u8]) -> Result<(), FixturePackageError> {
    std::str::from_utf8(bytes).map_err(|error| {
        invalid(format!(
            "physics control script is not valid UTF-8: {error}"
        ))
    })?;
    Ok(())
}

/// Validate the deliberately small SVG language fixture packages are allowed to carry.
///
/// The generator emits only an `svg` root and opaque polygonal `path` elements. Keeping the
/// package boundary stricter than a browser means renderers and documentation tools never need to
/// execute CSS, fetch a resource, resolve a font, or interpret an event handler.
fn validate_projection_svg(bytes: &[u8]) -> Result<(), FixturePackageError> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack = Vec::<Vec<u8>>::new();
    let mut saw_root = false;
    let mut saw_path = false;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| invalid(format!("SVG projection is malformed XML: {error}")))?;
        match event {
            Event::Start(element) => {
                let name = element.name().as_ref().to_vec();
                if !saw_root {
                    if name.as_slice() != b"svg" {
                        return Err(invalid("SVG projection root must be svg"));
                    }
                    validate_svg_attributes(&reader, &element, true)?;
                    saw_root = true;
                } else {
                    if name.as_slice() != b"path" || stack.len() != 1 {
                        return Err(invalid(
                            "SVG projection may contain only path elements under its root",
                        ));
                    }
                    validate_svg_attributes(&reader, &element, false)?;
                    saw_path = true;
                }
                stack.push(name);
            }
            Event::Empty(element) => {
                let name = element.name().as_ref().to_vec();
                if !saw_root {
                    return Err(invalid("SVG projection root cannot be self-closing"));
                }
                if name.as_slice() != b"path" || stack.len() != 1 {
                    return Err(invalid(
                        "SVG projection may contain only path elements under its root",
                    ));
                }
                validate_svg_attributes(&reader, &element, false)?;
                saw_path = true;
            }
            Event::End(element) => {
                let expected = stack
                    .pop()
                    .ok_or_else(|| invalid("SVG projection closes an element it did not open"))?;
                if expected.as_slice() != element.name().as_ref() {
                    return Err(invalid("SVG projection element nesting is invalid"));
                }
            }
            Event::Text(text) if text.as_ref().iter().all(u8::is_ascii_whitespace) => {}
            Event::Eof => break,
            _ => {
                return Err(invalid(
                    "SVG projection contains unsupported text, comments, declarations, or directives",
                ));
            }
        }
        buffer.clear();
    }
    if !saw_root || !saw_path || !stack.is_empty() {
        return Err(invalid(
            "SVG projection must be one complete svg root with at least one path",
        ));
    }
    Ok(())
}

pub(super) fn validate_projection_svg_contract(
    bytes: &[u8],
    projection: &crate::ProfileProjectionAsset,
) -> Result<(), FixturePackageError> {
    validate_projection_svg(bytes)?;
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    let mut buffer = Vec::new();
    let element = loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| invalid(format!("SVG projection is malformed XML: {error}")))?
        {
            Event::Start(element) => break element.into_owned(),
            Event::Eof => return Err(invalid("SVG projection has no root element")),
            _ => buffer.clear(),
        }
    };
    let mut view = None;
    let mut view_box = None;
    let mut width = None;
    let mut height = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| invalid(format!("SVG projection attribute is malformed: {error}")))?;
        let value = attribute
            .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|error| invalid(format!("SVG projection attribute is invalid: {error}")))?;
        match attribute.key.as_ref() {
            b"data-tosklight-view" => view = Some(value.into_owned()),
            b"viewBox" => view_box = Some(parse_svg_numbers(&value)?),
            b"width" => width = Some(parse_svg_millimetres(&value)?),
            b"height" => height = Some(parse_svg_millimetres(&value)?),
            _ => {}
        }
    }
    let expected = projection.view_box_millimetres;
    let actual = view_box.ok_or_else(|| invalid("SVG projection has no viewBox"))?;
    if view.as_deref() != Some(projection.view.wire())
        || actual.len() != 4
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| (*actual - expected).abs() > 0.01)
        || width.is_none_or(|value| (value - projection.physical_width_millimetres).abs() > 0.01)
        || height.is_none_or(|value| (value - projection.physical_height_millimetres).abs() > 0.01)
    {
        return Err(invalid(
            "SVG projection view, viewBox, or physical dimensions do not match profile metadata",
        ));
    }
    Ok(())
}

fn parse_svg_numbers(value: &str) -> Result<Vec<f32>, FixturePackageError> {
    value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f32>()
                .map_err(|_| invalid("SVG projection contains an invalid number"))
        })
        .collect()
}

fn parse_svg_millimetres(value: &str) -> Result<f32, FixturePackageError> {
    value
        .strip_suffix("mm")
        .ok_or_else(|| invalid("SVG projection physical dimensions must use mm"))?
        .parse::<f32>()
        .map_err(|_| invalid("SVG projection physical dimension is invalid"))
}

fn validate_svg_attributes(
    reader: &Reader<Cursor<&[u8]>>,
    element: &quick_xml::events::BytesStart<'_>,
    root: bool,
) -> Result<(), FixturePackageError> {
    let mut names = std::collections::HashSet::new();
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| invalid(format!("SVG projection attribute is malformed: {error}")))?;
        let name = attribute.key.as_ref();
        if !names.insert(name.to_vec()) {
            return Err(invalid("SVG projection repeats an attribute"));
        }
        let allowed = if root {
            matches!(
                name,
                b"xmlns"
                    | b"viewBox"
                    | b"width"
                    | b"height"
                    | b"data-tosklight-view"
                    | b"data-generator"
                    | b"data-generator-version"
                    | b"data-pose-contract-version"
            )
        } else {
            matches!(name, b"d" | b"fill" | b"fill-rule" | b"data-part")
        };
        if !allowed {
            return Err(invalid(format!(
                "SVG projection attribute {} is not supported",
                String::from_utf8_lossy(name)
            )));
        }
        let value = attribute
            .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|error| invalid(format!("SVG projection attribute is invalid: {error}")))?;
        match name {
            b"xmlns" if value.as_ref() != "http://www.w3.org/2000/svg" => {
                return Err(invalid("SVG projection namespace is invalid"));
            }
            b"viewBox" => validate_svg_numbers(&value, 4)?,
            b"width" | b"height" => {
                let number = value
                    .strip_suffix("mm")
                    .ok_or_else(|| invalid("SVG projection physical dimensions must use mm"))?;
                validate_positive_svg_number(number)?;
            }
            b"d" => validate_svg_path(&value)?,
            b"fill" if !valid_opaque_fill(&value) => {
                return Err(invalid(
                    "SVG projection paths require an opaque hexadecimal fill",
                ));
            }
            b"fill-rule" if value.as_ref() != "evenodd" && value.as_ref() != "nonzero" => {
                return Err(invalid("SVG projection fill-rule is invalid"));
            }
            _ => {}
        }
    }
    let required: &[&[u8]] = if root {
        &[
            b"xmlns",
            b"viewBox",
            b"width",
            b"height",
            b"data-tosklight-view",
        ]
    } else {
        &[b"d", b"fill"]
    };
    if required.iter().any(|name| !names.contains(*name)) {
        return Err(invalid("SVG projection is missing a required attribute"));
    }
    Ok(())
}

fn validate_svg_numbers(value: &str, count: usize) -> Result<(), FixturePackageError> {
    let numbers = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid("SVG projection contains an invalid number"))?;
    if numbers.len() != count || !numbers.iter().all(|number| number.is_finite()) {
        return Err(invalid("SVG projection numeric tuple is invalid"));
    }
    if count == 4 && (numbers[2] <= 0.0 || numbers[3] <= 0.0) {
        return Err(invalid(
            "SVG projection viewBox must have positive dimensions",
        ));
    }
    Ok(())
}

fn validate_positive_svg_number(value: &str) -> Result<(), FixturePackageError> {
    let number = value
        .parse::<f32>()
        .map_err(|_| invalid("SVG projection physical dimension is invalid"))?;
    if !number.is_finite() || number <= 0.0 {
        return Err(invalid(
            "SVG projection physical dimension must be positive",
        ));
    }
    Ok(())
}

fn validate_svg_path(value: &str) -> Result<(), FixturePackageError> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_digit()
                || byte.is_ascii_whitespace()
                || matches!(
                    byte,
                    b'M' | b'L' | b'Z' | b'm' | b'l' | b'z' | b'.' | b',' | b'-' | b'+'
                )
        })
        || !value.bytes().any(|byte| matches!(byte, b'M' | b'm'))
        || !value.bytes().any(|byte| matches!(byte, b'Z' | b'z'))
    {
        return Err(invalid(
            "SVG projection path must use only closed move/line geometry",
        ));
    }
    let move_count = value
        .bytes()
        .filter(|byte| matches!(byte, b'M' | b'm'))
        .count();
    let close_count = value
        .bytes()
        .filter(|byte| matches!(byte, b'Z' | b'z'))
        .count();
    let numeric = value
        .chars()
        .map(|character| match character {
            'M' | 'm' | 'L' | 'l' | 'Z' | 'z' | ',' => ' ',
            other => other,
        })
        .collect::<String>();
    let numbers = numeric
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid("SVG projection path contains an invalid number"))?;
    if move_count != 1
        || close_count != 1
        || numbers.len() < 6
        || numbers.len() % 2 != 0
        || !numbers.iter().all(|number| number.is_finite())
    {
        return Err(invalid(
            "SVG projection path must be one finite closed polygon",
        ));
    }
    Ok(())
}

fn valid_opaque_fill(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

fn sniff_image(bytes: &[u8]) -> Result<ImageFormat, FixturePackageError> {
    match image::guess_format(bytes) {
        Ok(format @ (ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)) => Ok(format),
        Ok(_) | Err(_) => Err(invalid("fixture images must be PNG, JPEG, or WebP")),
    }
}

fn validate_image_dimensions(
    kind: AssetKind,
    bytes: &[u8],
    format: ImageFormat,
) -> Result<(), FixturePackageError> {
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|error| invalid(format!("{} cannot be decoded: {error}", kind.label())))?;
    let limit = match kind {
        AssetKind::Photograph => MAX_PHOTOGRAPH_DIMENSION,
        AssetKind::Icon => MAX_ICON_DIMENSION,
        AssetKind::Gobo => MAX_GOBO_DIMENSION,
        AssetKind::Model
        | AssetKind::ScanScript
        | AssetKind::EffectScript
        | AssetKind::PhysicsScript
        | AssetKind::Projection => unreachable!(),
    };
    if width == 0 || height == 0 || width > limit || height > limit {
        return Err(invalid(format!(
            "{} dimensions must be 1-{limit} pixels",
            kind.label()
        )));
    }
    Ok(())
}

fn image_mime(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::WebP => "image/webp",
        _ => unreachable!(),
    }
}

fn image_extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::WebP => "webp",
        _ => unreachable!(),
    }
}
