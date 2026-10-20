//! Shared rough stairs and real slab apertures. All lengths and transforms are millimetres.
use super::*;

#[derive(Clone, Debug)]
pub struct VenueStairVolume {
    pub footprint_millimetres: Vec<[f64; 2]>,
    pub bottom_millimetres: f64,
    pub top_millimetres: f64,
}

/// Rigid column-major local-to-world pose, including the ordinary object's half-height lift.
pub fn venue_stair_volume(size: [f64; 3], pose: [[f64; 4]; 4]) -> VenueStairVolume {
    let footprint = rectangle(size[0], size[2]);
    let bottom: Vec<_> = footprint
        .iter()
        .map(|p| transform(pose, [p[0], -size[1] / 2.0, p[1]]))
        .collect();
    let top: Vec<_> = footprint
        .iter()
        .map(|p| transform(pose, [p[0], size[1] / 2.0, p[1]]))
        .collect();
    VenueStairVolume {
        footprint_millimetres: bottom.iter().map(|p| [p[0], p[2]]).collect(),
        bottom_millimetres: bottom.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        top_millimetres: top.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Generate the same local mesh for native rendering, CAD projection, picking and plan exports.
/// Ordinary objects ignore context; slab top, bottom and reveals all contain the real aperture.
pub fn venue_geometry_in_context(
    architecture: &VenueArchitecture,
    size: [f64; 3],
    pose: [[f64; 4]; 4],
    stairs: &[VenueStairVolume],
) -> Result<VenueGeometry, String> {
    let VenueArchitecture::Slab {
        outline_millimetres,
        elevation_millimetres,
        thickness_millimetres,
        ..
    } = architecture
    else {
        return venue_geometry(architecture, size);
    };
    architecture.validate_placement_millimetres(size)?;
    let bottom = f64::from(*elevation_millimetres);
    let top = bottom + f64::from(*thickness_millimetres);
    let world_bottom = transform(pose, [0.0, bottom, 0.0])[1];
    let world_top = transform(pose, [0.0, top, 0.0])[1];
    let holes: Vec<Vec<[f64; 2]>> = stairs
        .iter()
        .filter(|stair| {
            // A supporting floor at the stair's base remains intact; every reached upper level cuts.
            world_bottom > stair.bottom_millimetres + 0.01
                && world_bottom <= stair.top_millimetres + 0.01
                && world_top > stair.bottom_millimetres
        })
        .map(|stair| {
            stair
                .footprint_millimetres
                .iter()
                .map(|p| {
                    let local = inverse_transform(pose, [p[0], world_bottom, p[1]]);
                    [local[0], local[2]]
                })
                .collect()
        })
        .collect();
    if holes.is_empty() {
        return venue_geometry(architecture, size);
    }
    let outline: Vec<_> = outline_millimetres
        .iter()
        .map(|p| [f64::from(p.x), f64::from(p.y)])
        .collect();
    let mut pieces: Vec<Vec<[f64; 2]>> = triangulate(&outline)
        .iter()
        .map(|indices| indices.map(|i| outline[i]).to_vec())
        .collect();
    for hole in &holes {
        pieces = pieces
            .into_iter()
            .flat_map(|piece| subtract(piece, hole))
            .collect();
    }
    let remaining: f64 = pieces.iter().map(|p| signed_area(p).abs()).sum();
    if (remaining - signed_area(&outline).abs()).abs() < 0.01 {
        return venue_geometry(architecture, size);
    }
    let mut result = VenueGeometry {
        has_stair_cutouts: true,
        plan: boundary::boundary(&pieces),
        ..Default::default()
    };
    for piece in pieces {
        result.prism(&piece, bottom, top);
    }
    Ok(result)
}

pub(super) fn build(g: &mut VenueGeometry, size: [f64; 3], steps: u32) {
    let rise = size[1] / f64::from(steps);
    let run = size[2] / f64::from(steps);
    for step in 0..steps {
        let height = rise * f64::from(step + 1);
        g.cuboid(
            [
                0.0,
                -size[1] / 2.0 + height / 2.0,
                -size[2] / 2.0 + run * (f64::from(step) + 0.5),
            ],
            [size[0], height, run],
        );
    }
    g.footprint(size, false);
    for step in 1..steps {
        let z = -size[2] / 2.0 + run * f64::from(step);
        g.plan.push([[-size[0] / 2.0, z], [size[0] / 2.0, z]]);
    }
    // A simple up arrow makes the rise direction unambiguous in a top-down plan.
    let start = [0.0, -size[2] * 0.35];
    let end = [0.0, size[2] * 0.35];
    g.plan.push([start, end]);
    for x in [-size[0] * 0.12, size[0] * 0.12] {
        g.plan.push([[x, end[1] - size[0] * 0.16], end]);
    }
}
fn transform(m: [[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| m[0][i] * p[0] + m[1][i] * p[1] + m[2][i] * p[2] + m[3][i])
}
fn inverse_transform(m: [[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    let p = std::array::from_fn::<_, 3, _>(|i| p[i] - m[3][i]);
    std::array::from_fn(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2])
}
fn clip(polygon: &[[f64; 2]], a: [f64; 2], b: [f64; 2], inside: bool) -> Vec<[f64; 2]> {
    let sign = if inside { 1.0 } else { -1.0 };
    let mut result = Vec::new();
    for i in 0..polygon.len() {
        let p = polygon[i];
        let q = polygon[(i + 1) % polygon.len()];
        let dp = cross(a, b, p) * sign;
        let dq = cross(a, b, q) * sign;
        if dp >= -1e-8 {
            result.push(p);
        }
        if (dp > 0.0 && dq < 0.0) || (dp < 0.0 && dq > 0.0) {
            let t = dp / (dp - dq);
            result.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
        }
    }
    result.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-7);
    if result.len() > 2 && signed_area(&result).abs() > 1e-5 {
        result
    } else {
        Vec::new()
    }
}
fn subtract(mut inside: Vec<[f64; 2]>, hole: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    let mut hole = hole.to_vec();
    if signed_area(&hole) < 0.0 {
        hole.reverse();
    }
    let mut outside = Vec::new();
    for i in 0..hole.len() {
        if inside.is_empty() {
            break;
        }
        let a = hole[i];
        let b = hole[(i + 1) % hole.len()];
        let piece = clip(&inside, a, b, false);
        if !piece.is_empty() {
            outside.push(piece);
        }
        inside = clip(&inside, a, b, true);
    }
    outside
}
/// A legacy centred floor box, represented with the same size and physical bottom face.
pub fn venue_floor_slab(size: [f64; 3]) -> VenueArchitecture {
    VenueArchitecture::Slab {
        purpose: VenueSlabPurpose::Floor,
        outline_millimetres: rectangle(size[0], size[2])
            .iter()
            .map(|p| VenuePlanPoint {
                x: p[0].round() as i32,
                y: p[1].round() as i32,
            })
            .collect(),
        elevation_millimetres: (-size[1] / 2.0).round() as i32,
        thickness_millimetres: size[1].round().max(1.0) as u32,
    }
}
mod boundary;
#[cfg(test)]
mod tests;
