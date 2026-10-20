//! Rough venue construction carried by a visual-only object's placement.
//!
//! Coordinates are integer millimetres, matching the patch, rather than renderer units. Wall
//! dimensions belong to the placement; openings belong to that wall and move with it. Slab
//! outlines are local horizontal coordinates and may be concave, but never self-intersecting.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Parametric construction choices. This is authoring data, not a promise that a renderer or
/// editing surface implements a kind yet. An absent contract keeps existing scenery unchanged.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
pub struct VenuePlanPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueSlabPurpose {
    Roof,
    Balcony,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueFootprint {
    Round,
    Square,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueTableShape {
    Round,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueChairStyle {
    BlackMetal,
    White,
    BarStool,
}

/// An opening's origin is its lower-left corner measured from the wall's local start and base.
/// Its stable identity survives resizing and reordering; it is not a separate patch fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
pub struct WallOpening {
    pub id: Uuid,
    pub offset_millimetres: u32,
    pub sill_millimetres: u32,
    pub width_millimetres: u32,
    pub height_millimetres: u32,
    pub opening: WallOpeningKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueWindowShape {
    Rectangular,
    Round,
    Arched,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueDoorLeaves {
    Single,
    Double,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueDoorSwing {
    Inward,
    Outward,
}

/// Viewed from the wall's local front. Double doors have one hinge at each outer jamb.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum VenueDoorHinge {
    Left,
    Right,
}
