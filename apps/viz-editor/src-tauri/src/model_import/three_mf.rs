//! 3D Manufacturing Format: a zip of XML model parts, Z up, in the unit the file declares.
//!
//! Every build item is placed with its transform, components are expanded with theirs — also
//! across model parts, as the production extension writes them — and base materials and colour
//! groups give surface colours. Coordinates become metres, and Z up becomes glTF's Y up the way a
//! CAD tool's own glTF export turns it: `(x, y, z)` becomes `(x, z, -y)`.

use super::glb_builder::{Mesh, build_glb};
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;
use std::io::{Cursor, Read};

const DEFAULT_MODEL: &str = "3D/3dmodel.model";
/// A decompressed part may not grow past this; a bigger one is a zip bomb, not a venue.
const MAX_PART_BYTES: u64 = 512 * 1024 * 1024;
/// Components nested deeper than this are a loop, not an assembly.
const MAX_DEPTH: usize = 32;

/// A 3MF affine transform: `m00 m01 m02 m10 m11 m12 m20 m21 m22 m30 m31 m32`, row vectors.
type Affine = [f64; 12];
const IDENTITY: Affine = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

#[derive(Default)]
struct Object {
    name: String,
    vertices: Vec<[f64; 3]>,
    /// Each triangle and the colour it is drawn in, if any.
    triangles: Vec<([u32; 3], Option<[f32; 4]>)>,
    /// Other objects this one is assembled from: part path, object id, transform.
    components: Vec<(String, String, Affine)>,
}

#[derive(Default)]
struct Part {
    /// Metres per model unit.
    scale: f64,
    objects: HashMap<String, Object>,
    /// Build items: object id, transform, and the part path that object lives in.
    items: Vec<(String, Affine, String)>,
}

pub fn convert(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("the file is not a 3MF package ({error})"))?;
    let root = root_model_path(&mut archive);
    let mut parts: HashMap<String, Part> = HashMap::new();
    load_part(&mut archive, &mut parts, &root)?;
    let items = parts[&root].items.clone();
    if items.is_empty() {
        return Err("the 3MF build has no items to place".into());
    }
    let mut meshes = Vec::new();
    for (object, transform, path) in items {
        emit(
            &mut archive,
            &mut parts,
            &path,
            &object,
            transform,
            0,
            &mut meshes,
        )?;
    }
    build_glb(&meshes)
}

/// The model part `_rels/.rels` names as the 3D model, or the conventional one.
fn root_model_path(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> String {
    let Ok(rels) = read_entry(archive, "_rels/.rels") else {
        return DEFAULT_MODEL.into();
    };
    let mut reader = quick_xml::Reader::from_reader(rels.as_slice());
    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(element) | Event::Empty(element)
                if local(element.name().as_ref()) == b"Relationship"
                    && attribute(&element, b"Type")
                        .is_some_and(|kind| kind.ends_with("/3dmodel")) =>
            {
                if let Some(target) = attribute(&element, b"Target") {
                    return part_name(&target);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    DEFAULT_MODEL.into()
}

fn part_name(path: &str) -> String {
    path.trim_start_matches('/').to_owned()
}

fn read_entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, String> {
    let index = (0..archive.len())
        .find(|index| {
            archive
                .name_for_index(*index)
                .is_some_and(|entry| entry.eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| format!("the 3MF package has no {name}"))?;
    let entry = archive
        .by_index(index)
        .map_err(|error| format!("{name} could not be read from the 3MF package ({error})"))?;
    if entry.size() > MAX_PART_BYTES {
        return Err(format!("{name} is too large to be a venue model"));
    }
    let mut data = Vec::new();
    entry
        .take(MAX_PART_BYTES)
        .read_to_end(&mut data)
        .map_err(|error| format!("{name} could not be read from the 3MF package ({error})"))?;
    Ok(data)
}

fn load_part(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    parts: &mut HashMap<String, Part>,
    path: &str,
) -> Result<(), String> {
    if parts.contains_key(path) {
        return Ok(());
    }
    let xml = read_entry(archive, path)?;
    let part = parse_model(&xml, path).map_err(|error| format!("{path}: {error}"))?;
    parts.insert(path.to_owned(), part);
    Ok(())
}

/// Append the meshes of `object` in `path`, and of every component under it, placed by `transform`.
fn emit(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    parts: &mut HashMap<String, Part>,
    path: &str,
    id: &str,
    transform: Affine,
    depth: usize,
    meshes: &mut Vec<Mesh>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("object {id} is assembled from itself"));
    }
    load_part(archive, parts, path)?;
    let object = parts[path]
        .objects
        .get(id)
        .ok_or_else(|| format!("{path}: the build refers to object {id}, which is not defined"))?;
    // Every part declares its own unit, and its vertices are measured in it.
    push_meshes(object, transform, parts[path].scale, meshes);
    let components = object.components.clone();
    for (component_path, component, local) in components {
        let component_path = if component_path.is_empty() {
            path.to_owned()
        } else {
            component_path
        };
        let combined = compose(local, transform);
        emit(
            archive,
            parts,
            &component_path,
            &component,
            combined,
            depth + 1,
            meshes,
        )?;
    }
    Ok(())
}

type ColourMesh = (Option<[f32; 4]>, Mesh, HashMap<u32, u32>);

/// One mesh per colour of the object, in metres and Y up.
fn push_meshes(object: &Object, transform: Affine, scale: f64, meshes: &mut Vec<Mesh>) {
    // Each colour's mesh, and where each object vertex landed in it.
    let mut by_colour: Vec<ColourMesh> = Vec::new();
    for (corners, colour) in &object.triangles {
        let slot = match by_colour
            .iter()
            .position(|(existing, _, _)| existing == colour)
        {
            Some(slot) => slot,
            None => {
                let mesh = Mesh {
                    name: object.name.clone(),
                    colour: *colour,
                    ..Mesh::default()
                };
                by_colour.push((*colour, mesh, HashMap::new()));
                by_colour.len() - 1
            }
        };
        let (_, mesh, remap) = &mut by_colour[slot];
        for corner in corners {
            let local = *remap.entry(*corner).or_insert_with(|| {
                let [x, y, z] = apply(transform, object.vertices[*corner as usize]);
                mesh.positions
                    .push([(x * scale) as f32, (z * scale) as f32, (-y * scale) as f32]);
                (mesh.positions.len() - 1) as u32
            });
            mesh.indices.push(local);
        }
    }
    meshes.extend(by_colour.into_iter().map(|(_, mesh, _)| mesh));
}

fn apply(m: Affine, [x, y, z]: [f64; 3]) -> [f64; 3] {
    [
        x * m[0] + y * m[3] + z * m[6] + m[9],
        x * m[1] + y * m[4] + z * m[7] + m[10],
        x * m[2] + y * m[5] + z * m[8] + m[11],
    ]
}

/// `inner` applied first, then `outer`.
fn compose(inner: Affine, outer: Affine) -> Affine {
    let mut result = [0.0; 12];
    for row in 0..4 {
        for column in 0..3 {
            let mut value = if row == 3 { outer[9 + column] } else { 0.0 };
            for k in 0..3 {
                value += inner[row * 3 + k] * outer[k * 3 + column];
            }
            result[row * 3 + column] = value;
        }
    }
    result
}

fn unit_scale(unit: &str) -> Result<f64, String> {
    Ok(match unit {
        "micron" => 1e-6,
        "millimeter" => 1e-3,
        "centimeter" => 1e-2,
        "inch" => 0.0254,
        "foot" => 0.3048,
        "meter" => 1.0,
        other => return Err(format!("the model unit {other} is not a 3MF unit")),
    })
}

/// Colours defined by `basematerials` and `colorgroup`, by resource id.
type Palettes = HashMap<String, Vec<[f32; 4]>>;

/// What the parser is inside of while it walks one model part.
#[derive(Default)]
struct ParseState {
    part: Part,
    palettes: Palettes,
    palette: Option<String>,
    object: Option<(String, Object)>,
    default_colour: Option<(String, usize)>,
}

fn parse_model(xml: &[u8], path: &str) -> Result<Part, String> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut state = ParseState {
        part: Part {
            scale: 1e-3,
            ..Part::default()
        },
        ..ParseState::default()
    };
    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("the model XML is malformed ({error})"))?;
        match event {
            Event::Start(element) => state.open(&element, path)?,
            Event::Empty(element) => {
                state.open(&element, path)?;
                state.close(local(element.name().as_ref()));
            }
            Event::End(element) => state.close(local(element.name().as_ref())),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(state.part)
}

impl ParseState {
    fn open(&mut self, element: &BytesStart<'_>, path: &str) -> Result<(), String> {
        let text = |key: &[u8]| attribute(element, key);
        match local(element.name().as_ref()) {
            b"model" => {
                self.part.scale = unit_scale(text(b"unit").as_deref().unwrap_or("millimeter"))?
            }
            b"basematerials" | b"colorgroup" => {
                let id = text(b"id").unwrap_or_default();
                self.palettes.insert(id.clone(), Vec::new());
                self.palette = Some(id);
            }
            b"base" | b"color" => {
                let key: &[u8] = if local(element.name().as_ref()) == b"base" {
                    b"displaycolor"
                } else {
                    b"color"
                };
                if let Some(palette) = self
                    .palette
                    .as_ref()
                    .and_then(|id| self.palettes.get_mut(id))
                {
                    palette.push(
                        text(key)
                            .as_deref()
                            .and_then(parse_colour)
                            .unwrap_or([0.8, 0.8, 0.8, 1.0]),
                    );
                }
            }
            b"object" => {
                let object = Object {
                    name: text(b"name").unwrap_or_default(),
                    ..Object::default()
                };
                self.default_colour = text(b"pid").map(|pid| {
                    (
                        pid,
                        text(b"pindex")
                            .and_then(|index| index.parse().ok())
                            .unwrap_or(0),
                    )
                });
                self.object = Some((text(b"id").unwrap_or_default(), object));
            }
            b"vertex" => {
                if let Some((_, object)) = self.object.as_mut() {
                    let mut point = [0.0; 3];
                    for (slot, key) in point.iter_mut().zip([b"x", b"y", b"z"]) {
                        *slot = number(text(key), "vertex coordinate")?;
                    }
                    object.vertices.push(point);
                }
            }
            b"triangle" => self.triangle(element)?,
            b"component" => {
                if let Some((_, object)) = self.object.as_mut() {
                    let id = text(b"objectid").ok_or("a component names no object")?;
                    let part = text(b"path")
                        .map(|path| part_name(&path))
                        .unwrap_or_default();
                    object
                        .components
                        .push((part, id, transform(text(b"transform"))?));
                }
            }
            b"item" => {
                let id = text(b"objectid").ok_or("a build item names no object")?;
                let part = text(b"path")
                    .map(|path| part_name(&path))
                    .unwrap_or_else(|| path.to_owned());
                self.part
                    .items
                    .push((id, transform(text(b"transform"))?, part));
            }
            _ => {}
        }
        Ok(())
    }

    fn triangle(&mut self, element: &BytesStart<'_>) -> Result<(), String> {
        let text = |key: &[u8]| attribute(element, key);
        let colour = match (text(b"pid"), text(b"p1")) {
            (pid, Some(index)) => pid
                .or_else(|| self.default_colour.as_ref().map(|(pid, _)| pid.clone()))
                .and_then(|pid| self.colour(&pid, index.parse().unwrap_or(0))),
            (_, None) => self
                .default_colour
                .as_ref()
                .and_then(|(pid, index)| self.colour(pid, *index)),
        };
        let Some((id, object)) = self.object.as_mut() else {
            return Ok(());
        };
        let mut corners = [0_u32; 3];
        for (slot, key) in corners.iter_mut().zip([b"v1", b"v2", b"v3"]) {
            let index = number(text(key), "triangle corner")? as usize;
            if index >= object.vertices.len() {
                return Err(format!(
                    "object {id} has a triangle that refers to a missing vertex"
                ));
            }
            *slot = index as u32;
        }
        object.triangles.push((corners, colour));
        Ok(())
    }

    fn colour(&self, pid: &str, index: usize) -> Option<[f32; 4]> {
        self.palettes.get(pid)?.get(index).copied()
    }

    fn close(&mut self, name: &[u8]) {
        match name {
            b"basematerials" | b"colorgroup" => self.palette = None,
            b"object" => {
                if let Some((id, object)) = self.object.take() {
                    self.part.objects.insert(id, object);
                }
                self.default_colour = None;
            }
            _ => {}
        }
    }
}

fn number(value: Option<String>, what: &str) -> Result<f64, String> {
    let value = value.ok_or_else(|| format!("a {what} is missing"))?;
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("{what} {value} is not a finite number"))
}

fn transform(value: Option<String>) -> Result<Affine, String> {
    let Some(value) = value else {
        return Ok(IDENTITY);
    };
    let numbers: Vec<f64> = value
        .split_whitespace()
        .map(|number| {
            number
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
        })
        .collect::<Option<_>>()
        .ok_or_else(|| format!("transform \"{value}\" is not twelve finite numbers"))?;
    numbers
        .try_into()
        .map_err(|_| format!("transform \"{value}\" is not twelve finite numbers"))
}

/// `#RRGGBB` or `#RRGGBBAA` as linear RGBA, as glTF's base colour factor is.
fn parse_colour(text: &str) -> Option<[f32; 4]> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    let srgb = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let alpha = if hex.len() == 8 { channel(6)? } else { 255 };
    Some([
        srgb(channel(0)?),
        srgb(channel(2)?),
        srgb(channel(4)?),
        alpha as f32 / 255.0,
    ])
}

fn local(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn attribute(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|entry| local(entry.key.as_ref()) == key)
        .and_then(|entry| String::from_utf8(entry.value.into_owned()).ok())
}
