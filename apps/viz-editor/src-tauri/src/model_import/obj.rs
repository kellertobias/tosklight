//! Wavefront OBJ, with base colours from its MTL material libraries.
//!
//! OBJ carries no unit, so coordinates are read as metres, and it has no fixed up axis, so they
//! are read Y up as nearly every modelling tool writes it. Texture coordinates and normals in the
//! file are accepted and not used: textures are not drawn, and normals are worked out afresh.

use super::glb_builder::{Mesh, build_glb};
use super::sibling;
use std::collections::HashMap;

/// One output mesh while it is being filled: its name, colour name and vertex remapping.
struct Group {
    name: String,
    material: Option<String>,
    remap: HashMap<usize, u32>,
    mesh: Mesh,
}

pub fn convert(
    bytes: &[u8],
    read_sibling: &impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut colours: HashMap<String, [f32; 4]> = HashMap::new();
    let mut groups: Vec<Group> = Vec::new();
    let mut name = String::new();
    let mut material: Option<String> = None;
    // The group faces go into until the object, group or material changes.
    let mut active: Option<usize> = None;

    for (number, line) in logical_lines(&text) {
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        let rest: Vec<&str> = words.collect();
        match keyword {
            "v" => vertices.push(read_vertex(&rest, number)?),
            "f" => {
                let corners = rest
                    .iter()
                    .map(|corner| vertex_index(corner, vertices.len(), number))
                    .collect::<Result<Vec<_>, _>>()?;
                if corners.len() < 3 {
                    return Err(format!(
                        "line {number}: a face needs at least three corners"
                    ));
                }
                let index =
                    *active.get_or_insert_with(|| group_index(&mut groups, &name, &material));
                let group = &mut groups[index];
                for corner in 1..corners.len() - 1 {
                    for vertex in [corners[0], corners[corner], corners[corner + 1]] {
                        let local = *group.remap.entry(vertex).or_insert_with(|| {
                            group.mesh.positions.push(vertices[vertex]);
                            (group.mesh.positions.len() - 1) as u32
                        });
                        group.mesh.indices.push(local);
                    }
                }
            }
            "o" | "g" => {
                name = rest.join(" ");
                active = None;
            }
            "usemtl" => {
                material = Some(rest.join(" "));
                active = None;
            }
            "mtllib" => {
                // Names may contain spaces; most writers put one library on the line.
                let library = line.trim_start()["mtllib".len()..].trim();
                let bytes = sibling(read_sibling, library, "material library")?;
                colours.extend(read_mtl(&String::from_utf8_lossy(&bytes)));
            }
            _ => {}
        }
    }

    let meshes: Vec<Mesh> = groups
        .into_iter()
        .map(|group| {
            let mut mesh = group.mesh;
            mesh.name = group.name;
            mesh.colour = group
                .material
                .as_ref()
                .and_then(|material| colours.get(material).copied());
            mesh
        })
        .collect();
    build_glb(&meshes)
}

/// Lines with their 1-based numbers, comments removed and `\` continuations joined.
fn logical_lines(text: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut pending = String::new();
    let mut start = 0;
    for (index, raw) in text.lines().enumerate() {
        if pending.is_empty() {
            start = index + 1;
        }
        let line = raw.split('#').next().unwrap_or_default();
        if let Some(continued) = line.trim_end().strip_suffix('\\') {
            pending.push_str(continued);
            pending.push(' ');
            continue;
        }
        pending.push_str(line);
        lines.push((start, std::mem::take(&mut pending)));
    }
    if !pending.is_empty() {
        lines.push((start, pending));
    }
    lines
}

fn group_index(groups: &mut Vec<Group>, name: &str, material: &Option<String>) -> usize {
    let found = groups
        .iter()
        .position(|group| group.name == name && &group.material == material);
    found.unwrap_or_else(|| {
        groups.push(Group {
            name: name.to_owned(),
            material: material.clone(),
            remap: HashMap::new(),
            mesh: Mesh::default(),
        });
        groups.len() - 1
    })
}

fn read_vertex(values: &[&str], number: usize) -> Result<[f32; 3], String> {
    let mut position = [0.0_f32; 3];
    if values.len() < 3 {
        return Err(format!("line {number}: a vertex needs x, y and z"));
    }
    for (slot, value) in position.iter_mut().zip(values) {
        *slot = value
            .parse::<f32>()
            .map_err(|_| format!("line {number}: vertex coordinate {value} is not a number"))?;
        if !slot.is_finite() {
            return Err(format!(
                "line {number}: vertex coordinate {value} is not a finite number"
            ));
        }
    }
    Ok(position)
}

/// The zero-based vertex a face corner such as `3`, `3/1`, `3//2`, `3/1/2` or `-1` refers to.
fn vertex_index(corner: &str, count: usize, number: usize) -> Result<usize, String> {
    let text = corner.split('/').next().unwrap_or_default();
    let value: i64 = text
        .parse()
        .map_err(|_| format!("line {number}: face corner {corner} is not a vertex number"))?;
    let index = match value {
        0 => None,
        positive if positive > 0 => Some(positive as usize - 1),
        negative => count.checked_sub(negative.unsigned_abs() as usize),
    };
    index.filter(|index| *index < count).ok_or_else(|| {
        format!("line {number}: face corner {corner} refers to a vertex that is not defined")
    })
}

/// Material names and their diffuse colour with opacity.
fn read_mtl(text: &str) -> HashMap<String, [f32; 4]> {
    let mut colours = HashMap::new();
    let mut current: Option<(String, [f32; 4])> = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let Some((keyword, rest)) = line
            .split_once(char::is_whitespace)
            .map(|(keyword, rest)| (keyword, rest.trim()))
        else {
            continue;
        };
        let numbers: Vec<f32> = rest
            .split_whitespace()
            .filter_map(|value| value.parse().ok())
            .filter(|value: &f32| value.is_finite())
            .collect();
        if keyword == "newmtl" {
            colours.extend(current.take());
            current = Some((rest.to_owned(), [0.8, 0.8, 0.8, 1.0]));
            continue;
        }
        match (keyword, current.as_mut()) {
            ("Kd", Some((_, colour))) if numbers.len() >= 3 => {
                for (slot, value) in colour.iter_mut().zip(&numbers[..3]) {
                    *slot = value.clamp(0.0, 1.0);
                }
            }
            ("d", Some((_, colour))) if !numbers.is_empty() => {
                colour[3] = numbers[0].clamp(0.0, 1.0)
            }
            ("Tr", Some((_, colour))) if !numbers.is_empty() => {
                colour[3] = (1.0 - numbers[0]).clamp(0.0, 1.0);
            }
            _ => {}
        }
    }
    colours.extend(current);
    colours
}
