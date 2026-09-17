//! The 3D models every Media Server ships without an import.
//!
//! Five simple test shapes — Plane, Cube, Sphere, Cylinder, and Pyramid — are generated here
//! rather than read from files, so they are available on a fresh installation, on a library root
//! that holds no `.models` folder, and in tests. Each mesh carries texture coordinates with
//! `(0, 0)` at the top left of the layer image and goes through the same normalization as an
//! imported model, so it frames exactly like one.
//!
//! **Plane is the default.** A new library assigns it to slot 1, and a layer whose selected model
//! cannot be drawn is mapped onto the Plane rather than onto any other model.

use std::f32::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::model_library::{ModelGeometry, ModelVertex};

/// Segments around the round shapes. Enough for a smooth silhouette at output resolution.
const SEGMENTS: u32 = 48;
/// Rings from the sphere's top to its bottom.
const RINGS: u32 = 24;

/// One built-in model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinModel {
    Plane,
    Cube,
    Sphere,
    Cylinder,
    Pyramid,
}

impl BuiltinModel {
    /// Every built-in model, in the order a new library assigns them to slots 1–5.
    pub const ALL: [Self; 5] = [
        Self::Plane,
        Self::Cube,
        Self::Sphere,
        Self::Cylinder,
        Self::Pyramid,
    ];

    /// The model 3D mapping falls back to. Never inferred from anything else.
    pub const DEFAULT: Self = Self::Plane;

    pub const fn id(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            Self::Cube => "cube",
            Self::Sphere => "sphere",
            Self::Cylinder => "cylinder",
            Self::Pyramid => "pyramid",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Plane => "Plane",
            Self::Cube => "Cube",
            Self::Sphere => "Sphere",
            Self::Cylinder => "Cylinder",
            Self::Pyramid => "Pyramid",
        }
    }

    /// The slot a new library assigns this model to.
    pub const fn default_slot(self) -> u8 {
        match self {
            Self::Plane => 1,
            Self::Cube => 2,
            Self::Sphere => 3,
            Self::Cylinder => 4,
            Self::Pyramid => 5,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|model| model.id() == id)
    }

    /// The normalized mesh. Generated once per process and shared.
    pub fn geometry(self) -> Arc<ModelGeometry> {
        static CACHE: [OnceLock<Arc<ModelGeometry>>; 5] = [const { OnceLock::new() }; 5];
        let index = Self::ALL
            .iter()
            .position(|model| *model == self)
            .expect("every built-in model is listed");
        Arc::clone(CACHE[index].get_or_init(|| {
            let mesh = match self {
                Self::Plane => plane(),
                Self::Cube => cube(),
                Self::Sphere => sphere(),
                Self::Cylinder => cylinder(),
                Self::Pyramid => pyramid(),
            };
            Arc::new(
                mesh.geometry
                    .normalized()
                    .expect("built-in models are valid meshes"),
            )
        }))
    }
}

#[derive(Default)]
struct Mesh {
    geometry: ModelGeometry,
}

impl Mesh {
    fn vertex(&mut self, position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> u32 {
        self.geometry.vertices.push(ModelVertex {
            position,
            normal,
            uv,
        });
        (self.geometry.vertices.len() - 1) as u32
    }

    fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.geometry.indices.extend([a, b, c]);
    }

    /// A flat quad from four corners given counter-clockwise from the bottom left as seen from
    /// outside, with the full image on it.
    fn quad(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3]) {
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let first = self.vertex(corners[0], normal, uvs[0]);
        for (corner, uv) in corners.into_iter().zip(uvs).skip(1) {
            self.vertex(corner, normal, uv);
        }
        self.triangle(first, first + 1, first + 2);
        self.triangle(first, first + 2, first + 3);
    }

    /// A disc at height `y`, its image mapped straight down onto it.
    fn cap(&mut self, y: f32, normal_y: f32) {
        let normal = [0.0, normal_y, 0.0];
        let centre = self.vertex([0.0, y, 0.0], normal, [0.5, 0.5]);
        for segment in 0..=SEGMENTS {
            let angle = segment as f32 / SEGMENTS as f32 * TAU;
            let (x, z) = (angle.sin(), angle.cos());
            self.vertex([x, y, z], normal, [0.5 + x * 0.5, 0.5 - z * 0.5 * normal_y]);
        }
        for segment in 0..SEGMENTS {
            let (a, b) = (centre + 1 + segment, centre + 2 + segment);
            if normal_y > 0.0 {
                self.triangle(centre, a, b);
            } else {
                self.triangle(centre, b, a);
            }
        }
    }
}

/// A square facing the camera, the image upright on it. The renderer recognises this mesh and
/// draws the Plane as an output-shaped screen instead, so it always has the output's aspect ratio.
fn plane() -> Mesh {
    let mut mesh = Mesh::default();
    mesh.quad(
        [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        [0.0, 0.0, 1.0],
    );
    mesh
}

/// Six faces, each showing the whole image; the front face looks like the Plane.
fn cube() -> Mesh {
    let mut mesh = Mesh::default();
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        // normal, right, up
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    for (normal, right, up) in faces {
        let corner =
            |r: f32, u: f32| [0, 1, 2].map(|axis| normal[axis] + right[axis] * r + up[axis] * u);
        mesh.quad(
            [
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            normal,
        );
    }
    mesh
}

/// A UV sphere: the image wraps once around it, its top edge at the north pole. The seam sits at
/// the back, so the image centre faces the camera.
fn sphere() -> Mesh {
    let mut mesh = Mesh::default();
    for ring in 0..=RINGS {
        let v = ring as f32 / RINGS as f32;
        let polar = v * PI;
        for segment in 0..=SEGMENTS {
            let u = segment as f32 / SEGMENTS as f32;
            let azimuth = (u - 0.5) * TAU;
            let position = [
                polar.sin() * azimuth.sin(),
                polar.cos(),
                polar.sin() * azimuth.cos(),
            ];
            mesh.vertex(position, position, [u, v]);
        }
    }
    let row = SEGMENTS + 1;
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let a = ring * row + segment;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            if ring != 0 {
                mesh.triangle(a, c, b);
            }
            if ring != RINGS - 1 {
                mesh.triangle(b, c, d);
            }
        }
    }
    mesh
}

/// An upright cylinder as tall as it is wide. The image wraps once around its side, seam at the
/// back; each cap shows the whole image.
fn cylinder() -> Mesh {
    let mut mesh = Mesh::default();
    for segment in 0..=SEGMENTS {
        let u = segment as f32 / SEGMENTS as f32;
        let azimuth = (u - 0.5) * TAU;
        let normal = [azimuth.sin(), 0.0, azimuth.cos()];
        mesh.vertex([normal[0], 1.0, normal[2]], normal, [u, 0.0]);
        mesh.vertex([normal[0], -1.0, normal[2]], normal, [u, 1.0]);
    }
    for segment in 0..SEGMENTS {
        let top = segment * 2;
        let (bottom, next_top, next_bottom) = (top + 1, top + 2, top + 3);
        mesh.triangle(top, bottom, next_bottom);
        mesh.triangle(top, next_bottom, next_top);
    }
    mesh.cap(1.0, 1.0);
    mesh.cap(-1.0, -1.0);
    mesh
}

/// A square-based pyramid. Each side shows the image as a triangle, apex at its top centre; the
/// base shows the whole image.
fn pyramid() -> Mesh {
    let mut mesh = Mesh::default();
    let apex = [0.0, 1.0, 0.0];
    let base = [
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, -1.0, -1.0],
        [-1.0, -1.0, -1.0],
    ];
    for side in 0..4 {
        let (left, right) = (base[side], base[(side + 1) % 4]);
        let normal = face_normal(left, right, apex);
        let a = mesh.vertex(left, normal, [0.0, 1.0]);
        let b = mesh.vertex(right, normal, [1.0, 1.0]);
        let c = mesh.vertex(apex, normal, [0.5, 0.0]);
        mesh.triangle(a, b, c);
    }
    mesh.quad([base[3], base[2], base[1], base[0]], [0.0, -1.0, 0.0]);
    mesh
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = n.iter().map(|x| x * x).sum::<f32>().sqrt();
    n.map(|x| x / length)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outward(geometry: &ModelGeometry) -> bool {
        geometry.indices.chunks(3).all(|triangle| {
            let [a, b, c] = [0, 1, 2].map(|i| geometry.vertices[triangle[i] as usize].position);
            let normal = face_normal(a, b, c);
            let centre = [0, 1, 2].map(|axis| (a[axis] + b[axis] + c[axis]) / 3.0);
            // Every shape is convex and centred on the origin, so an outward face points away
            // from it. Degenerate pole slivers have no direction to check.
            normal.iter().any(|x| x.is_nan())
                || normal.iter().zip(centre).map(|(n, c)| n * c).sum::<f32>() > -1e-4
        })
    }

    #[test]
    fn every_built_in_model_is_a_normalized_textured_mesh() {
        for model in BuiltinModel::ALL {
            let geometry = model.geometry();
            assert!(geometry.triangle_count() >= 2, "{model:?}");
            assert_eq!(geometry.indices.len() % 3, 0, "{model:?}");
            let farthest = geometry
                .vertices
                .iter()
                .map(|v| v.position.iter().map(|p| p * p).sum::<f32>().sqrt())
                .fold(0.0_f32, f32::max);
            assert!(
                (farthest - 1.0).abs() < 1e-4,
                "{model:?} reaches {farthest}"
            );
            assert!(
                geometry
                    .vertices
                    .iter()
                    .all(|v| v.uv.iter().all(|t| (0.0..=1.0).contains(t))),
                "{model:?} keeps its texture coordinates on the image"
            );
            assert!(outward(&geometry), "{model:?} faces wind outward");
            assert!(
                Arc::ptr_eq(&geometry, &model.geometry()),
                "{model:?} is shared"
            );
        }
    }

    #[test]
    fn each_model_has_its_own_shape() {
        let depth = |model: BuiltinModel| {
            let geometry = model.geometry();
            let z = geometry.vertices.iter().map(|v| v.position[2]);
            z.clone().fold(f32::MIN, f32::max) - z.fold(f32::MAX, f32::min)
        };
        assert_eq!(depth(BuiltinModel::Plane), 0.0, "the Plane is flat");
        for model in &BuiltinModel::ALL[1..] {
            assert!(depth(*model) > 0.5, "{model:?} has depth");
        }
        let counts: Vec<usize> = BuiltinModel::ALL
            .iter()
            .map(|model| model.geometry().triangle_count())
            .collect();
        assert_eq!(counts[0], 2);
        assert_eq!(counts[1], 12);
        assert_eq!(counts[4], 6);
        assert!(counts[2] > 100 && counts[3] > 100, "{counts:?}");
    }

    #[test]
    fn the_plane_faces_the_camera_with_the_image_upright() {
        let plane = BuiltinModel::Plane.geometry();
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let top_left = plane.vertices.iter().find(|v| v.uv == [0.0, 0.0]).unwrap();
        assert!((top_left.position[0] + half).abs() < 1e-5);
        assert!((top_left.position[1] - half).abs() < 1e-5);
        assert!(plane.vertices.iter().all(|v| v.normal == [0.0, 0.0, 1.0]));
    }

    #[test]
    fn ids_labels_and_default_slots_are_stable() {
        assert_eq!(BuiltinModel::DEFAULT, BuiltinModel::Plane);
        let ids: Vec<_> = BuiltinModel::ALL.iter().map(|m| m.id()).collect();
        assert_eq!(ids, ["plane", "cube", "sphere", "cylinder", "pyramid"]);
        let labels: Vec<_> = BuiltinModel::ALL.iter().map(|m| m.label()).collect();
        assert_eq!(labels, ["Plane", "Cube", "Sphere", "Cylinder", "Pyramid"]);
        let slots: Vec<_> = BuiltinModel::ALL.iter().map(|m| m.default_slot()).collect();
        assert_eq!(slots, [1, 2, 3, 4, 5]);
        for model in BuiltinModel::ALL {
            assert_eq!(BuiltinModel::from_id(model.id()), Some(model));
            assert_eq!(
                serde_json::to_value(model).unwrap(),
                serde_json::Value::from(model.id())
            );
        }
        assert_eq!(BuiltinModel::from_id("torus"), None);
    }
}
