use light_core::AttributeKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GeometryGraph {
    #[serde(default)]
    pub nodes: Vec<GeometryNode>,
    #[serde(default)]
    pub emitters: Vec<GeometryEmitter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeometryNode {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub transform: Transform3,
    #[serde(default)]
    pub pivot: Vector3,
    #[serde(default)]
    pub glb_node: Option<String>,
    #[serde(default)]
    pub motion: Option<GeometryMotion>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Transform3 {
    pub translation: Vector3,
    pub rotation_degrees: Vector3,
    pub scale: Vector3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeometryMotion {
    pub attribute: AttributeKey,
    pub kind: GeometryMotionKind,
    pub axis: Vector3,
    pub physical_min: f32,
    pub physical_max: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryMotionKind {
    Rotation,
    Translation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeometryEmitter {
    pub id: Uuid,
    pub name: String,
    pub node_id: Uuid,
    pub head_id: Uuid,
    #[serde(default)]
    pub origin: Vector3,
    #[serde(default)]
    pub orientation_degrees: Vector3,
    pub beam_angle_degrees: f32,
    pub field_angle_degrees: f32,
    #[serde(default)]
    pub feather: f32,
    #[serde(default)]
    pub focus: f32,
    /// Whether this emitter projects light along a meaningful aim direction.
    /// Broad sources such as strobes and strip fixtures set this to false.
    #[serde(default = "default_directional_emitter")]
    pub directional: bool,
    pub layout: EmitterLayout,
}

fn default_directional_emitter() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EmitterLayout {
    Point,
    Matrix {
        columns: u16,
        rows: u16,
        spacing: Vector3,
    },
    Ring {
        count: u16,
        radius_millimetres: f32,
    },
    Strip {
        count: u16,
        spacing_millimetres: f32,
    },
    ExplicitPixels {
        positions: Vec<Vector3>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryTemplate {
    Fixed,
    MovingHead,
    Bar,
    Matrix,
    SharedPanMultiHead,
}
