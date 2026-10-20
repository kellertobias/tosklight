//! Individual pieces of emitting glass, including disconnected lenses within one GLB mesh.

use super::{EMITTER_MARKERS, FixtureModel, HOUSING_MARKERS};
use glam::{Vec2, Vec3};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct EmittingSurface {
    pub anchor: Vec3,
    pub size: Vec2,
}

impl FixtureModel {
    /// Recover lens cells from mesh connectivity. Shared positions connect vertices split for
    /// normals or UV seams; disconnected domes remain separate even inside an array mesh.
    pub fn emitting_surfaces(&self) -> Vec<EmittingSurface> {
        let Some(axis) = self.emitter_axis else {
            return Vec::new();
        };
        // The GLB reader derives cardinal surface normals. A custom geometry graph can rotate
        // that metadata; callers should keep their authored layout for such a model.
        if axis.abs().max_element() < 0.9999 {
            return Vec::new();
        }
        let across = Vec3::ONE - axis.abs();
        let mut surfaces: Vec<EmittingSurface> = Vec::new();
        for part in &self.parts {
            let name = part.name.to_ascii_lowercase();
            if part.metallic > 0.5
                || !EMITTER_MARKERS.iter().any(|marker| name.contains(marker))
                || HOUSING_MARKERS.iter().any(|marker| name.contains(marker))
            {
                continue;
            }
            let mut parents: Vec<usize> = (0..part.positions.len()).collect();
            let mut positions = HashMap::new();
            for (index, position) in part.positions.iter().enumerate() {
                let key = position.map(|value| (value * 1_000_000.0).round() as i64);
                if let Some(previous) = positions.insert(key, index) {
                    join(&mut parents, previous, index);
                }
            }
            for triangle in part.indices.chunks_exact(3) {
                join(&mut parents, triangle[0] as usize, triangle[1] as usize);
                join(&mut parents, triangle[1] as usize, triangle[2] as usize);
            }
            let mut bounds = HashMap::new();
            for (index, position) in part.positions.iter().enumerate() {
                let group = root(&mut parents, index);
                let point = Vec3::from_array(*position);
                let entry = bounds.entry(group).or_insert((point, point));
                entry.0 = entry.0.min(point);
                entry.1 = entry.1.max(point);
            }
            for (min, max) in bounds.into_values() {
                let front = if axis.max_element() > 0.0 { max } else { min };
                let anchor = (min + max) * 0.5 * across + front * axis.abs();
                let extent = max - min;
                let dimensions: Vec<f32> = (0..3)
                    .filter(|i| across[*i] > 0.5)
                    .map(|i| extent[i])
                    .collect();
                let size = Vec2::new(dimensions[0], dimensions[1]);
                if size.min_element() < 0.001 {
                    continue;
                }
                // A domed lens and its moulded concentric ridges are one piece of glass.
                if let Some(old) = surfaces
                    .iter_mut()
                    .find(|old| ((old.anchor - anchor) * across).length() < 0.0001)
                {
                    old.size = old.size.max(size);
                    if anchor.dot(axis) > old.anchor.dot(axis) {
                        old.anchor = anchor;
                    }
                } else {
                    surfaces.push(EmittingSurface { anchor, size });
                }
            }
        }
        surfaces.sort_by(|a, b| {
            a.anchor
                .x
                .total_cmp(&b.anchor.x)
                .then(a.anchor.y.total_cmp(&b.anchor.y))
                .then(a.anchor.z.total_cmp(&b.anchor.z))
        });
        surfaces
    }
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

fn join(parents: &mut [usize], left: usize, right: usize) {
    let left = root(parents, left);
    let right = root(parents, right);
    parents[right] = left;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_diagonal_emitting_axis_keeps_its_authored_layout_without_panicking() {
        let model = FixtureModel {
            emitter_axis: Some(Vec3::ONE.normalize()),
            ..Default::default()
        };
        assert!(model.emitting_surfaces().is_empty());
    }
}
