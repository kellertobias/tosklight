//! Rough venue construction carried by a visual-only object's placement.
//!
//! Coordinates are integer millimetres, matching the patch, rather than renderer units. Wall
//! dimensions belong to the placement; openings belong to that wall and move with it. Slab
//! outlines are local horizontal coordinates and may be concave, but never self-intersecting.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

/// Parametric construction choices. This is authoring data, not a promise that a renderer or
/// editing surface implements a kind yet. An absent contract keeps existing scenery unchanged.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VenueArchitecture {
    Wall {
        #[serde(default)]
        openings: Vec<WallOpening>,
        /// Height of a ceiling generated when this wall participates in a closed room.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ceiling_height_millimetres: Option<u32>,
    },
    Slab {
        purpose: VenueSlabPurpose,
        outline_millimetres: Vec<VenuePlanPoint>,
        /// Bottom face above the placement's floor datum; independent slabs can be stacked.
        elevation_millimetres: i32,
        thickness_millimetres: u32,
    },
    Pillar {
        shape: VenueFootprint,
    },
    Table {
        shape: VenueTableShape,
    },
    Chair {
        style: VenueChairStyle,
    },
    /// A closed, solid counter; its width, height and depth belong to the placement.
    Bar,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VenuePlanPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueSlabPurpose {
    Roof,
    Balcony,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueFootprint {
    Round,
    Square,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueTableShape {
    Round,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueChairStyle {
    BlackMetal,
    White,
    BarStool,
}

/// An opening's origin is its lower-left corner measured from the wall's local start and base.
/// Its stable identity survives resizing and reordering; it is not a separate patch fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WallOpening {
    pub id: Uuid,
    pub offset_millimetres: u32,
    pub sill_millimetres: u32,
    pub width_millimetres: u32,
    pub height_millimetres: u32,
    pub opening: WallOpeningKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WallOpeningKind {
    Window {
        shape: VenueWindowShape,
    },
    Door {
        leaves: VenueDoorLeaves,
        swing: VenueDoorSwing,
        hinge: VenueDoorHinge,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueWindowShape {
    Rectangular,
    Round,
    Arched,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueDoorLeaves {
    Single,
    Double,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueDoorSwing {
    Inward,
    Outward,
}

/// Viewed from the wall's local front. Double doors have one hinge at each outer jamb.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VenueDoorHinge {
    Left,
    Right,
}

impl VenueArchitecture {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Wall {
                openings,
                ceiling_height_millimetres,
            } => {
                if *ceiling_height_millimetres == Some(0) {
                    return Err("venue ceiling height must be positive".into());
                }
                validate_openings(openings)
            }
            Self::Slab {
                outline_millimetres,
                thickness_millimetres,
                ..
            } => {
                if *thickness_millimetres == 0 {
                    return Err("venue slab thickness must be positive".into());
                }
                validate_outline(outline_millimetres)
            }
            _ => Ok(()),
        }
    }

    /// Checks dimensions against the authoritative placed size, in patch axes: width, height,
    /// depth. Call after resolving the profile default when the placement has no explicit size.
    pub fn validate_placement_millimetres(&self, size: [f64; 3]) -> Result<(), String> {
        self.validate()?;
        if size.iter().any(|value| !value.is_finite() || *value <= 0.0) {
            return Err("venue width, height and depth must be finite and positive".into());
        }
        if let Self::Wall { openings, .. } = self {
            for opening in openings {
                if f64::from(opening.offset_millimetres) + f64::from(opening.width_millimetres)
                    > size[0]
                    || f64::from(opening.sill_millimetres) + f64::from(opening.height_millimetres)
                        > size[1]
                {
                    return Err(format!(
                        "venue opening {} extends outside its wall",
                        opening.id
                    ));
                }
            }
        }
        let equal_footprint = matches!(
            self,
            Self::Pillar {
                shape: VenueFootprint::Round | VenueFootprint::Square
            } | Self::Table {
                shape: VenueTableShape::Round
            }
        );
        if equal_footprint && (size[0] - size[2]).abs() > 0.001 {
            return Err("round and square venue footprints require equal width and depth".into());
        }
        Ok(())
    }
}

fn validate_openings(openings: &[WallOpening]) -> Result<(), String> {
    if openings.len() > 1024 {
        return Err("a venue wall supports at most 1024 openings".into());
    }
    let mut ids = BTreeSet::new();
    for (index, opening) in openings.iter().enumerate() {
        if opening.id.is_nil() || !ids.insert(opening.id) {
            return Err(
                "venue opening identities must be non-nil and unique within their wall".into(),
            );
        }
        if opening.width_millimetres == 0 || opening.height_millimetres == 0 {
            return Err("venue opening width and height must be positive".into());
        }
        match opening.opening {
            WallOpeningKind::Window {
                shape: VenueWindowShape::Round,
            } if opening.width_millimetres != opening.height_millimetres => {
                return Err("a round venue window requires equal width and height".into());
            }
            WallOpeningKind::Window {
                shape: VenueWindowShape::Arched,
            } if u64::from(opening.height_millimetres) * 2
                < u64::from(opening.width_millimetres) =>
            {
                return Err("an arched venue window must accommodate its semicircular top".into());
            }
            WallOpeningKind::Door { .. } if opening.sill_millimetres != 0 => {
                return Err("a venue door must start at the wall base".into());
            }
            _ => {}
        }
        if openings[..index]
            .iter()
            .any(|other| openings_overlap(opening, other))
        {
            return Err("venue wall openings must not overlap".into());
        }
    }
    Ok(())
}

/// Bounding envelopes must not overlap; touching jambs or sills are allowed.
fn openings_overlap(a: &WallOpening, b: &WallOpening) -> bool {
    let end = |offset: u32, length: u32| u64::from(offset) + u64::from(length);
    u64::from(a.offset_millimetres) < end(b.offset_millimetres, b.width_millimetres)
        && u64::from(b.offset_millimetres) < end(a.offset_millimetres, a.width_millimetres)
        && u64::from(a.sill_millimetres) < end(b.sill_millimetres, b.height_millimetres)
        && u64::from(b.sill_millimetres) < end(a.sill_millimetres, a.height_millimetres)
}

fn validate_outline(points: &[VenuePlanPoint]) -> Result<(), String> {
    if !(3..=256).contains(&points.len()) {
        return Err(
            "a venue slab outline requires 3 to 256 vertices without a repeated closing point"
                .into(),
        );
    }
    let n = points.len();
    let mut area = 0_i128;
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % n];
        if a == b {
            return Err("a venue slab outline cannot contain an empty edge".into());
        }
        area += i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y);
        // Adjacent collinear edges may continue straight, but may not fold back over themselves.
        let c = points[(i + 2) % n];
        if cross(a, b, c) == 0 && (on_segment(a, b, c) || on_segment(b, c, a)) {
            return Err("a venue slab outline cannot double back along an edge".into());
        }
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if intersects(a, b, points[j], points[(j + 1) % n]) {
                return Err("a venue slab outline must not intersect or touch itself".into());
            }
        }
    }
    if area == 0 {
        return Err("a venue slab outline must enclose a nonzero area".into());
    }
    Ok(())
}

fn cross(a: VenuePlanPoint, b: VenuePlanPoint, c: VenuePlanPoint) -> i128 {
    (i128::from(b.x) - i128::from(a.x)) * (i128::from(c.y) - i128::from(a.y))
        - (i128::from(b.y) - i128::from(a.y)) * (i128::from(c.x) - i128::from(a.x))
}

fn on_segment(a: VenuePlanPoint, b: VenuePlanPoint, p: VenuePlanPoint) -> bool {
    cross(a, b, p) == 0
        && (a.x.min(b.x)..=a.x.max(b.x)).contains(&p.x)
        && (a.y.min(b.y)..=a.y.max(b.y)).contains(&p.y)
}

fn intersects(a: VenuePlanPoint, b: VenuePlanPoint, c: VenuePlanPoint, d: VenuePlanPoint) -> bool {
    let [ab_c, ab_d, cd_a, cd_b] = [
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    ];
    (ab_c.signum() * ab_d.signum() < 0 && cd_a.signum() * cd_b.signum() < 0)
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}

#[cfg(test)]
mod tests;
