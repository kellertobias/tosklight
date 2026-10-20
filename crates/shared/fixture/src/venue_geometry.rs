//! Shared rough construction in millimetres. X is width, Y is height, Z is depth.
//! Ordinary objects are centred on their placed box; slab outlines use the placement datum.
use crate::*;

#[derive(Clone, Debug, Default)]
pub struct VenueGeometry {
    pub(crate) wall_plan_symbols: usize,
    pub(crate) wall_body_triangles: usize,
    pub joined_wall: bool,
    /// Physical exterior edges in local coordinates; omits coplanar ownership/triangulation seams.
    pub exterior: Vec<[[f64; 3]; 2]>,
    /// Semantic centre-plane window/door frames for a joined wall.
    pub elevation: Vec<[[f64; 3]; 2]>,
    /// Context removed physical floor area; false preserves a legacy floor unchanged.
    pub has_stair_cutouts: bool,
    pub triangles: Vec<[[f64; 3]; 3]>,
    /// Parallel surface roles; structural furniture legs are bare metal.
    pub metallic: Vec<bool>,
    /// Complete plan drawing, including door leaves and swing arcs. Never camera-filtered.
    pub plan: Vec<[[f64; 2]; 2]>,
}

pub fn venue_geometry(
    architecture: &VenueArchitecture,
    size: [f64; 3],
) -> Result<VenueGeometry, String> {
    architecture.validate_placement_millimetres(size)?;
    let mut result = VenueGeometry::default();
    match architecture {
        VenueArchitecture::Stairs { steps } => stairs::build(&mut result, size, *steps),
        VenueArchitecture::Wall { openings, .. } => wall::build(&mut result, size, openings),
        VenueArchitecture::Slab {
            outline_millimetres,
            elevation_millimetres,
            thickness_millimetres,
            ..
        } => {
            let outline: Vec<_> = outline_millimetres
                .iter()
                .map(|p| [f64::from(p.x), f64::from(p.y)])
                .collect();
            result.prism(
                &outline,
                f64::from(*elevation_millimetres),
                f64::from(*elevation_millimetres) + f64::from(*thickness_millimetres),
            );
            result.outline(&outline);
        }
        VenueArchitecture::Pillar { shape } => {
            if *shape == VenueFootprint::Round {
                result.cylinder([0.0; 3], size);
            } else {
                result.cuboid([0.0; 3], size);
            }
            result.footprint(size, *shape == VenueFootprint::Round);
        }
        VenueArchitecture::Bar => {
            result.cuboid([0.0; 3], size);
            result.footprint(size, false);
        }
        VenueArchitecture::Table { shape } => furniture::table(&mut result, size, *shape),
        VenueArchitecture::Chair { style } => furniture::chair(&mut result, size, *style),
    }
    Ok(result)
}

impl VenueGeometry {
    fn triangle(&mut self, a: [f64; 3], b: [f64; 3], c: [f64; 3]) {
        self.triangles.push([a, b, c]);
        self.metallic.push(false);
    }
    fn quad(&mut self, p: [[f64; 3]; 4]) {
        self.triangle(p[0], p[1], p[2]);
        self.triangle(p[0], p[2], p[3]);
    }
    fn outline(&mut self, p: &[[f64; 2]]) {
        for i in 0..p.len() {
            self.plan.push([p[i], p[(i + 1) % p.len()]]);
        }
    }
    fn footprint(&mut self, size: [f64; 3], round: bool) {
        self.outline(&if round {
            ellipse(size[0], size[2], 40)
        } else {
            rectangle(size[0], size[2])
        });
    }
    fn cuboid(&mut self, centre: [f64; 3], size: [f64; 3]) {
        let outline: Vec<_> = rectangle(size[0], size[2])
            .iter()
            .map(|p| [p[0] + centre[0], p[1] + centre[2]])
            .collect();
        self.prism(
            &outline,
            centre[1] - size[1] / 2.0,
            centre[1] + size[1] / 2.0,
        );
    }
    fn cylinder(&mut self, centre: [f64; 3], size: [f64; 3]) {
        let outline: Vec<_> = ellipse(size[0], size[2], 32)
            .iter()
            .map(|p| [p[0] + centre[0], p[1] + centre[2]])
            .collect();
        self.prism(
            &outline,
            centre[1] - size[1] / 2.0,
            centre[1] + size[1] / 2.0,
        );
    }
    fn prism(&mut self, outline: &[[f64; 2]], bottom: f64, top: f64) {
        for indices in triangulate(outline) {
            let [a, b, c] = indices.map(|i| outline[i]);
            self.triangle([a[0], top, a[1]], [c[0], top, c[1]], [b[0], top, b[1]]);
            self.triangle(
                [a[0], bottom, a[1]],
                [b[0], bottom, b[1]],
                [c[0], bottom, c[1]],
            );
        }
        let positive = signed_area(outline) > 0.0;
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let mut p = [
                [a[0], bottom, a[1]],
                [b[0], bottom, b[1]],
                [b[0], top, b[1]],
                [a[0], top, a[1]],
            ];
            if positive {
                p.reverse();
            }
            self.quad(p);
        }
    }
}
fn rectangle(w: f64, d: f64) -> Vec<[f64; 2]> {
    vec![
        [-w / 2.0, -d / 2.0],
        [w / 2.0, -d / 2.0],
        [w / 2.0, d / 2.0],
        [-w / 2.0, d / 2.0],
    ]
}
fn ellipse(w: f64, d: f64, n: usize) -> Vec<[f64; 2]> {
    (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [w / 2.0 * a.cos(), d / 2.0 * a.sin()]
        })
        .collect()
}
fn signed_area(p: &[[f64; 2]]) -> f64 {
    (0..p.len())
        .map(|i| {
            let a = p[i];
            let b = p[(i + 1) % p.len()];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>()
        / 2.0
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
/// Ear clipping supports concave balcony outlines in either winding.
fn triangulate(p: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let mut remaining: Vec<_> = (0..p.len()).collect();
    if signed_area(p) < 0.0 {
        remaining.reverse();
    }
    let mut result = Vec::new();
    while remaining.len() > 2 {
        let ear = (0..remaining.len()).find(|&i| {
            let a = remaining[(i + remaining.len() - 1) % remaining.len()];
            let b = remaining[i];
            let c = remaining[(i + 1) % remaining.len()];
            cross(p[a], p[b], p[c]) > 1e-8
                && !remaining.iter().any(|&j| {
                    j != a
                        && j != b
                        && j != c
                        && cross(p[a], p[b], p[j]) >= -1e-8
                        && cross(p[b], p[c], p[j]) >= -1e-8
                        && cross(p[c], p[a], p[j]) >= -1e-8
                })
        });
        let Some(i) = ear else { break };
        result.push([
            remaining[(i + remaining.len() - 1) % remaining.len()],
            remaining[i],
            remaining[(i + 1) % remaining.len()],
        ]);
        remaining.remove(i);
    }
    result
}
mod furniture;
mod joins;
pub use joins::{VenueWallPlacement, venue_joined_walls};
mod rooms;
mod stairs;
pub use rooms::{VenueRoom, VenueWallRun, venue_rooms};
pub use stairs::{
    VenueStairVolume, venue_floor_slab, venue_geometry_in_context, venue_stair_volume,
};
#[cfg(test)]
mod tests;
mod wall;
