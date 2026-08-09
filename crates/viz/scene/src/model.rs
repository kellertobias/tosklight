//! Reading a fixture's GLB model into renderable geometry.
//!
//! A fixture package may carry `assets/model.glb`. What the renderer needs from it is flat: one
//! triangle list per named part, in metres, in the fixture's own space, plus which part is the
//! yoke and which is the head so pan and tilt can move them. Of the material only the base colour
//! and the two shading factors are kept — enough to tell wool serge from bare aluminium, and no
//! more, because the desk's picture is made of light rather than of shading detail on a lamp body
//! and every byte parsed here is a byte the operator waits for. Textures are ignored outright.
//!
//! The reader is strict about structure and forgiving about content: anything it cannot use is
//! skipped and named in the result, so a fixture with an unusable model falls back to its
//! procedural proxy with a reason rather than disappearing.

use glam::{Mat4, Quat, Vec3};

/// One fixture model, already flattened into world-of-the-fixture space.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FixtureModel {
    pub parts: Vec<ModelPart>,
    /// Bounding half-extent in metres, used to scale a model authored at another size.
    pub extent: Vec3,
    /// Where the tilt axis runs through the model, in model space.
    ///
    /// A head tilts about its own trunnions, not about the point the fixture hangs from. Without
    /// this the head swings around the base like a wrecking ball instead of pointing.
    pub head_pivot: Vec3,
    /// Where the light leaves the model, in model space, when the model says.
    ///
    /// A fixture whose profile does not describe its own optics has no idea where its lens is,
    /// and the answer it falls back to is the fixture's origin — which for every model in the
    /// shipped set is the *rigging point*. The beam then starts at the clamp, a lamp's length
    /// above the lens it should come out of. The model already knows; this is it saying so.
    pub emitter_anchor: Option<Vec3>,
    /// How big the emitting face is, across the two directions the light does not travel in.
    ///
    /// Measured from the model's own lens geometry. Without it a fixture whose profile says nothing
    /// about its optics gets a face sized by its class — a per-type guess that has no relation to
    /// the model it is drawn on, so a 150 mm guess sat in the middle of a 260 mm lens and the lit
    /// face looked a third of the size of the thing projecting the light.
    pub emitter_size: Option<glam::Vec2>,
    /// Which way that surface looks, in model space, when the model says.
    ///
    /// A lantern's lens is in its bottom and it hangs pointing down, which is why `-Y` is the rest
    /// aim of every emitter. That is a property of the body rather than a law: a laser projector's
    /// window is in its *front* face, and a beam leaving such a fixture downwards comes out of the
    /// side of the box. The face the light leaves already says which way it looks.
    pub emitter_axis: Option<Vec3>,
    /// Whether anything in the model tilts, so a caller knows [`Self::head_pivot`] means something.
    pub has_head: bool,
    /// What the reader had to skip, for the diagnostics surface.
    pub warnings: Vec<String>,
}

impl FixtureModel {
    pub fn is_empty(&self) -> bool {
        self.parts.iter().all(|part| part.indices.is_empty())
    }

    /// The uniform scale that fits this model to a fixture's declared body size.
    ///
    /// Anything read out of the model in model space — the lens, the tilt pivot — has to go
    /// through this before it can sit beside a fixture's own metres, and it has to be the same
    /// number the geometry is drawn with or the beam leaves from beside the lamp.
    pub fn scale_to(&self, size: Vec3) -> f32 {
        (size.max(Vec3::splat(0.02)) / (self.extent * 2.0).max(Vec3::splat(0.001)))
            .min_element()
            .clamp(0.05, 20.0)
    }

    pub fn triangle_count(&self) -> usize {
        self.parts
            .iter()
            .map(|part| part.indices.len() / 3)
            .sum::<usize>()
    }
}

/// Which moving part of a fixture a piece of geometry belongs to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModelPartKind {
    /// Bolted to the truss: never moves.
    #[default]
    Base,
    /// Turns with pan.
    Yoke,
    /// Turns with pan and tilt.
    Head,
}

impl ModelPartKind {
    /// Classify a node by the name conventions glTF authors use for lamps.
    ///
    /// A profile may name the nodes explicitly; this is what happens when it does not.
    pub fn from_node_name(name: &str) -> Self {
        let folded = name.to_ascii_lowercase();
        if folded.contains("head") || folded.contains("lamp") || folded.contains("tilt") {
            Self::Head
        } else if folded.contains("yoke") || folded.contains("arm") || folded.contains("pan") {
            Self::Yoke
        } else {
            Self::Base
        }
    }
}

/// One drawable piece of a model.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ModelPart {
    pub name: String,
    pub kind: ModelPartKind,
    /// Interleaved position and normal, already in fixture space.
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Linear base colour.
    pub colour: [f32; 3],
    /// How rough the surface is, `0` mirror-smooth and `1` matt.
    ///
    /// Read because it is the difference between a wool drape and the aluminium hanging it:
    /// shaded with one figure for the whole model, fabric picks up the same highlight as
    /// bare metal and a rig stops reading as made of different things.
    pub roughness: f32,
    /// How metallic the surface is, `0` dielectric and `1` bare metal.
    pub metallic: f32,
}

/// Why a model could not be read.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelError(pub String);

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn fail(message: impl Into<String>) -> ModelError {
    ModelError(message.into())
}

// Little-endian "glTF", "JSON" and "BIN\0", as glTF 2.0 spells them in a GLB container.
const GLB_MAGIC: u32 = 0x4654_6c67;
const CHUNK_JSON: u32 = 0x4e4f_534a;
const CHUNK_BIN: u32 = 0x004e_4942;
/// A lamp body is a prop, not a hero asset. Anything past this is a modelling mistake and would
/// cost the operator frames for detail nobody can see at stage distance.
const MAX_TRIANGLES: usize = 120_000;

/// Read a self-contained GLB 2.0 file.
pub fn read_glb(bytes: &[u8]) -> Result<FixtureModel, ModelError> {
    let (json, binary) = split_chunks(bytes)?;
    let document: serde_json::Value =
        serde_json::from_slice(json).map_err(|error| fail(format!("model JSON: {error}")))?;
    let mut model = FixtureModel::default();
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);

    let nodes = array(&document, "nodes");
    let meshes = array(&document, "meshes");
    let roots = scene_roots(&document, nodes.len());

    let mut stack: Vec<(usize, Mat4, ModelPartKind)> = roots
        .into_iter()
        .map(|index| (index, Mat4::IDENTITY, ModelPartKind::Base))
        .collect();
    let mut visited = vec![false; nodes.len()];

    while let Some((index, parent, inherited)) = stack.pop() {
        let Some(node) = nodes.get(index) else {
            continue;
        };
        // A cycle in the node graph is malformed, and walking it would never end.
        if std::mem::replace(&mut visited[index], true) {
            model
                .warnings
                .push(format!("node {index} is referenced more than once"));
            continue;
        }
        let name = node
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let transform = parent * node_transform(node);
        let kind = match ModelPartKind::from_node_name(&name) {
            ModelPartKind::Base => inherited,
            named => named,
        };

        if let Some(mesh_index) = node.get("mesh").and_then(serde_json::Value::as_u64)
            && let Some(mesh) = meshes.get(mesh_index as usize)
        {
            match read_mesh(&document, binary, mesh, transform, &name, kind) {
                Ok(parts) => {
                    for part in parts {
                        for position in &part.positions {
                            let point = Vec3::from_array(*position);
                            bounds_min = bounds_min.min(point);
                            bounds_max = bounds_max.max(point);
                        }
                        model.parts.push(part);
                    }
                }
                Err(error) => model.warnings.push(error.0),
            }
        }

        for child in node
            .get("children")
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if let Some(child) = child.as_u64() {
                stack.push((child as usize, transform, kind));
            }
        }
    }

    if model.is_empty() {
        return Err(fail("model contains no triangles"));
    }
    if model.triangle_count() > MAX_TRIANGLES {
        return Err(fail(format!(
            "model has {} triangles, more than the {MAX_TRIANGLES} a fixture body may use",
            model.triangle_count()
        )));
    }
    model.extent = ((bounds_max - bounds_min) * 0.5).max(Vec3::splat(0.001));
    model.head_pivot = head_pivot(&model);
    model.has_head = model
        .parts
        .iter()
        .any(|part| part.kind == ModelPartKind::Head);
    let face = emitter_face(&model, bounds_min, bounds_max);
    model.emitter_anchor = face.map(|face| face.anchor);
    model.emitter_axis = face.map(|face| face.axis);
    model.emitter_size = face.map(|face| face.size);
    Ok(model)
}

/// The centre of everything that tilts, which is where the trunnions are.
fn head_pivot(model: &FixtureModel) -> Vec3 {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in model
        .parts
        .iter()
        .filter(|part| part.kind == ModelPartKind::Head)
    {
        for position in &part.positions {
            let point = Vec3::from_array(*position);
            min = min.min(point);
            max = max.max(point);
        }
    }
    if min.x > max.x {
        return Vec3::ZERO;
    }
    (min + max) * 0.5
}

/// Node names that mark a surface the light actually comes out of.
///
/// The same trick the reader already uses for pan and tilt: the model says what a part is by
/// what it is called. `lens` covers a lens and the ring round it, `source` the emitter plates and
/// arrays of an LED fixture, `cell` the reflector cells of a blinder or a sunstrip, `diffuser` a
/// milky pane, and `aperture` a laser window or a hazer nozzle. Nothing else in the shipped set
/// contains any of them, so a body, a yoke and a colour frame stay out of it.
/// Names that read as what *carries* the glass rather than the glass itself.
const HOUSING_MARKERS: [&str; 9] = [
    "ring", "bezel", "housing", "frame", "holder", "barrel", "surround", "cups", "plate",
];

const EMITTER_MARKERS: [&str; 6] = ["lens", "source", "diffuser", "emitter", "cell", "aperture"];

/// The surface light leaves the model by: where it is, and which way it looks.
#[derive(Clone, Copy)]
struct EmitterFace {
    /// Extent across the two axes the light does not travel along.
    size: glam::Vec2,
    /// Centred across the face and taken at the **front** of it, because light leaves the front of
    /// a lens rather than the middle of the glass. On a PAR that is the difference between the
    /// mouth of the can and a point a third of the way down inside it.
    anchor: Vec3,
    /// Unit vector out of that face.
    axis: Vec3,
}

/// The surface light leaves by, or `None` when nothing is named as an emitting surface.
///
/// Which way it looks is read off where it sits on the body: a lens is in the face of the fixture
/// it shines out of, so the axis the emitting parts are furthest out along is the one the light
/// leaves by. Every lantern in the shipped set answers `-Y` — its lens is in its underside and it
/// hangs pointing down — and a laser projector answers `+Z`, because its window is in its front.
///
/// Measured as a share of the body's own half-extent rather than in metres, or a wide fixture with
/// a shallow lens would read as pointing out of its long side.
fn emitter_face(model: &FixtureModel, bounds_min: Vec3, bounds_max: Vec3) -> Option<EmitterFace> {
    // Every emitting part, kept apart rather than merged. Where they go together decides the anchor;
    // how big *one* of them is decides the face, and those are different questions.
    let mut parts: Vec<(Vec3, Vec3)> = Vec::new();
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in &model.parts {
        let folded = part.name.to_ascii_lowercase();
        if !EMITTER_MARKERS.iter().any(|marker| folded.contains(marker)) {
            continue;
        }
        // What holds the glass is not the glass. A ring, a bezel or a frame carrying a lens matches
        // `lens` by name and is most of what a small lantern's front is made of, which is why
        // measuring the assembly made an ACL's lit face half again too big.
        if HOUSING_MARKERS.iter().any(|marker| folded.contains(marker)) {
            continue;
        }
        let mut part_min = Vec3::splat(f32::INFINITY);
        let mut part_max = Vec3::splat(f32::NEG_INFINITY);
        for position in &part.positions {
            let point = Vec3::from_array(*position);
            part_min = part_min.min(point);
            part_max = part_max.max(point);
        }
        if part_min.x > part_max.x {
            continue;
        }
        min = min.min(part_min);
        max = max.max(part_max);
        parts.push((part_min, part_max));
    }
    if min.x > max.x {
        return None;
    }
    let half = ((bounds_max - bounds_min) * 0.5).max(Vec3::splat(1e-4));
    let offset = ((min + max) * 0.5 - (bounds_min + bounds_max) * 0.5) / half;
    let axis = dominant_axis(offset);
    // The front of the face along that axis: the extreme of the emitting geometry in the direction
    // the light goes, and the middle of it across the other two.
    let centre = (min + max) * 0.5;
    let front = if axis.max_element() > 0.0 { max } else { min };
    let across = Vec3::ONE - axis.abs();

    /*
     * One emitting part, not the run of them.
     *
     * A strip's cells each match `cell`, so bounding them together measures the whole strip and
     * every cell then draws at the size of all of them — which is how a row of lit faces came to
     * hang off both ends of the extrusion. The smallest matching part is one cell on a strip and
     * the glass on a lantern, which is the answer in both cases.
     */
    let span = |extent: Vec3| (extent * across).length();
    let smallest = parts
        .iter()
        .map(|(part_min, part_max)| *part_max - *part_min)
        .reduce(|best, extent| {
            if span(extent) < span(best) {
                extent
            } else {
                best
            }
        })
        .unwrap_or(max - min);
    let spans: Vec<f32> = (0..3)
        .filter(|index| across[*index] > 0.5)
        .map(|index| smallest[index])
        .collect();
    Some(EmitterFace {
        anchor: centre * across + front * axis.abs(),
        axis,
        size: glam::Vec2::new(
            spans.first().copied().unwrap_or(0.05).max(0.002),
            spans.get(1).copied().unwrap_or(0.05).max(0.002),
        ),
    })
}

/// The signed unit axis a vector leans furthest along, `-Y` when it leans nowhere in particular.
///
/// A fixture whose emitting surface sits dead centre in its body — a bare source with no housing
/// around it — has nothing to say about which way it looks, and the rest aim every other emitter
/// has is the right answer for it.
fn dominant_axis(offset: Vec3) -> Vec3 {
    let magnitude = offset.abs();
    let (axis, share) = if magnitude.x >= magnitude.y && magnitude.x >= magnitude.z {
        (Vec3::X, offset.x)
    } else if magnitude.y >= magnitude.z {
        (Vec3::Y, offset.y)
    } else {
        (Vec3::Z, offset.z)
    };
    if share.abs() < 0.02 {
        return Vec3::NEG_Y;
    }
    axis * share.signum()
}

/// Split a GLB container into its JSON and binary chunks.
fn split_chunks(bytes: &[u8]) -> Result<(&[u8], &[u8]), ModelError> {
    if bytes.len() < 20 {
        return Err(fail("model is too short to be a GLB file"));
    }
    if read_u32(bytes, 0) != GLB_MAGIC {
        return Err(fail("model is not a GLB file"));
    }
    if read_u32(bytes, 4) != 2 {
        return Err(fail("model is not GLB 2.0"));
    }
    if read_u32(bytes, 8) as usize != bytes.len() {
        return Err(fail("model declares a different length than it has"));
    }
    let mut cursor = 12;
    let mut json: &[u8] = &[];
    let mut binary: &[u8] = &[];
    while cursor + 8 <= bytes.len() {
        let length = read_u32(bytes, cursor) as usize;
        let kind = read_u32(bytes, cursor + 4);
        cursor += 8;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| fail("model contains a truncated chunk"))?;
        match kind {
            CHUNK_JSON if json.is_empty() => json = &bytes[cursor..end],
            CHUNK_BIN if binary.is_empty() => binary = &bytes[cursor..end],
            _ => {}
        }
        cursor = end;
    }
    if json.is_empty() {
        return Err(fail("model has no JSON chunk"));
    }
    Ok((json, binary))
}

fn array<'a>(document: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    document
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// The nodes to start walking from: the default scene, or every node when there is none.
fn scene_roots(document: &serde_json::Value, node_count: usize) -> Vec<usize> {
    let scene = document
        .get("scene")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;
    let roots: Vec<usize> = array(document, "scenes")
        .get(scene)
        .map(|scene| array(scene, "nodes"))
        .unwrap_or_default()
        .iter()
        .filter_map(serde_json::Value::as_u64)
        .map(|index| index as usize)
        .collect();
    if roots.is_empty() {
        (0..node_count).collect()
    } else {
        roots
    }
}

fn node_transform(node: &serde_json::Value) -> Mat4 {
    if let Some(matrix) = node.get("matrix").and_then(serde_json::Value::as_array)
        && matrix.len() == 16
    {
        let mut columns = [0.0_f32; 16];
        for (slot, value) in columns.iter_mut().zip(matrix) {
            *slot = value.as_f64().unwrap_or(0.0) as f32;
        }
        return Mat4::from_cols_array(&columns);
    }
    let translation = read_vec3(node, "translation", Vec3::ZERO);
    let scale = read_vec3(node, "scale", Vec3::ONE);
    let rotation = node
        .get("rotation")
        .and_then(serde_json::Value::as_array)
        .filter(|values| values.len() == 4)
        .map(|values| {
            Quat::from_xyzw(
                values[0].as_f64().unwrap_or(0.0) as f32,
                values[1].as_f64().unwrap_or(0.0) as f32,
                values[2].as_f64().unwrap_or(0.0) as f32,
                values[3].as_f64().unwrap_or(1.0) as f32,
            )
            .normalize()
        })
        .unwrap_or(Quat::IDENTITY);
    Mat4::from_scale_rotation_translation(scale, rotation, translation)
}

fn read_vec3(node: &serde_json::Value, key: &str, fallback: Vec3) -> Vec3 {
    node.get(key)
        .and_then(serde_json::Value::as_array)
        .filter(|values| values.len() == 3)
        .map(|values| {
            Vec3::new(
                values[0].as_f64().unwrap_or(0.0) as f32,
                values[1].as_f64().unwrap_or(0.0) as f32,
                values[2].as_f64().unwrap_or(0.0) as f32,
            )
        })
        .unwrap_or(fallback)
}

fn read_mesh(
    document: &serde_json::Value,
    binary: &[u8],
    mesh: &serde_json::Value,
    transform: Mat4,
    name: &str,
    kind: ModelPartKind,
) -> Result<Vec<ModelPart>, ModelError> {
    let mut parts = Vec::new();
    let normal_matrix = glam::Mat3::from_mat4(transform).inverse().transpose();
    for primitive in array(mesh, "primitives") {
        // Only triangles. A GLB that draws a lamp with lines or points is not describing a body.
        let mode = primitive
            .get("mode")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(4);
        if mode != 4 {
            return Err(fail(format!("{name}: only triangle primitives are drawn")));
        }
        let attributes = primitive
            .get("attributes")
            .ok_or_else(|| fail(format!("{name}: primitive has no attributes")))?;
        let position_index = attributes
            .get("POSITION")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| fail(format!("{name}: primitive has no positions")))?;
        let positions = read_vec3_accessor(document, binary, position_index as usize)?;
        let normals = match attributes.get("NORMAL").and_then(serde_json::Value::as_u64) {
            Some(index) => read_vec3_accessor(document, binary, index as usize)?,
            None => vec![[0.0, 1.0, 0.0]; positions.len()],
        };
        let indices = match primitive.get("indices").and_then(serde_json::Value::as_u64) {
            Some(index) => read_index_accessor(document, binary, index as usize)?,
            None => (0..positions.len() as u32).collect(),
        };
        if indices
            .iter()
            .any(|index| *index as usize >= positions.len())
        {
            return Err(fail(format!("{name}: an index points outside the mesh")));
        }
        let surface = primitive_surface(document, primitive);

        parts.push(ModelPart {
            name: name.to_owned(),
            kind,
            positions: positions
                .iter()
                .map(|position| {
                    transform
                        .transform_point3(Vec3::from_array(*position))
                        .to_array()
                })
                .collect(),
            normals: normals
                .iter()
                .map(|normal| {
                    (normal_matrix * Vec3::from_array(*normal))
                        .normalize_or(Vec3::Y)
                        .to_array()
                })
                .collect(),
            indices,
            colour: surface.colour,
            roughness: surface.roughness,
            metallic: surface.metallic,
        });
    }
    Ok(parts)
}

/// The three surface numbers a part carries: base colour, roughness and metallic.
struct PrimitiveSurface {
    colour: [f32; 3],
    roughness: f32,
    metallic: f32,
}

impl Default for PrimitiveSurface {
    /// A neutral painted housing, for a primitive with no material at all.
    fn default() -> Self {
        Self {
            colour: [0.08, 0.085, 0.09],
            roughness: 0.5,
            metallic: 0.2,
        }
    }
}

/// A model's surface, or a neutral housing grey when it has no material.
///
/// glTF's defaults for the two factors are `1.0` each, which would make every untagged part
/// rough bare metal. A lamp body is neither, so a material that names only a colour keeps the
/// painted-housing defaults above and only an explicit factor moves it.
fn primitive_surface(
    document: &serde_json::Value,
    primitive: &serde_json::Value,
) -> PrimitiveSurface {
    let mut surface = PrimitiveSurface::default();
    let Some(index) = primitive
        .get("material")
        .and_then(serde_json::Value::as_u64)
        .map(|index| index as usize)
    else {
        return surface;
    };
    let Some(pbr) = array(document, "materials")
        .get(index)
        .and_then(|material| material.get("pbrMetallicRoughness"))
    else {
        return surface;
    };
    if let Some(values) = pbr
        .get("baseColorFactor")
        .and_then(serde_json::Value::as_array)
        .filter(|values| values.len() >= 3)
    {
        surface.colour = [
            values[0].as_f64().unwrap_or(0.5) as f32,
            values[1].as_f64().unwrap_or(0.5) as f32,
            values[2].as_f64().unwrap_or(0.5) as f32,
        ];
    }
    if let Some(value) = pbr
        .get("roughnessFactor")
        .and_then(serde_json::Value::as_f64)
    {
        surface.roughness = (value as f32).clamp(0.0, 1.0);
    }
    if let Some(value) = pbr
        .get("metallicFactor")
        .and_then(serde_json::Value::as_f64)
    {
        surface.metallic = (value as f32).clamp(0.0, 1.0);
    }
    surface
}

struct AccessorView<'a> {
    bytes: &'a [u8],
    stride: usize,
    count: usize,
    component: u64,
}

fn accessor_view<'a>(
    document: &serde_json::Value,
    binary: &'a [u8],
    index: usize,
    element_bytes: usize,
) -> Result<AccessorView<'a>, ModelError> {
    let accessor = array(document, "accessors")
        .get(index)
        .ok_or_else(|| fail(format!("accessor {index} is missing")))?;
    let count = accessor
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;
    let component = accessor
        .get("componentType")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let view_index = accessor
        .get("bufferView")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| fail(format!("accessor {index} has no buffer view")))?;
    let view = array(document, "bufferViews")
        .get(view_index as usize)
        .ok_or_else(|| fail(format!("buffer view {view_index} is missing")))?;
    let offset = view
        .get("byteOffset")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
    let stride = view
        .get("byteStride")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(element_bytes as u64) as usize;
    let needed = offset + stride.max(element_bytes) * count.saturating_sub(1) + element_bytes;
    if needed > binary.len() {
        return Err(fail(format!(
            "accessor {index} reads past the end of the model's buffer"
        )));
    }
    Ok(AccessorView {
        bytes: &binary[offset..],
        stride: stride.max(element_bytes),
        count,
        component,
    })
}

fn read_vec3_accessor(
    document: &serde_json::Value,
    binary: &[u8],
    index: usize,
) -> Result<Vec<[f32; 3]>, ModelError> {
    let view = accessor_view(document, binary, index, 12)?;
    if view.component != 5126 {
        return Err(fail(format!(
            "accessor {index} is not float; only float positions and normals are read"
        )));
    }
    Ok((0..view.count)
        .map(|element| {
            let base = element * view.stride;
            [
                read_f32(view.bytes, base),
                read_f32(view.bytes, base + 4),
                read_f32(view.bytes, base + 8),
            ]
        })
        .collect())
}

fn read_index_accessor(
    document: &serde_json::Value,
    binary: &[u8],
    index: usize,
) -> Result<Vec<u32>, ModelError> {
    let element_bytes = match array(document, "accessors")
        .get(index)
        .and_then(|accessor| accessor.get("componentType"))
        .and_then(serde_json::Value::as_u64)
    {
        Some(5121) => 1,
        Some(5123) => 2,
        Some(5125) => 4,
        _ => return Err(fail(format!("accessor {index} is not an index accessor"))),
    };
    let view = accessor_view(document, binary, index, element_bytes)?;
    Ok((0..view.count)
        .map(|element| {
            let base = element * view.stride;
            match element_bytes {
                1 => view.bytes[base] as u32,
                2 => u16::from_le_bytes([view.bytes[base], view.bytes[base + 1]]) as u32,
                _ => read_u32(view.bytes, base),
            }
        })
        .collect())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a GLB in memory: two boxes, one named as a head, so the reader is tested against a
    /// real container rather than a hand-made structure it happens to agree with.
    fn glb(head_name: &str) -> Vec<u8> {
        glb_with_surface(
            head_name,
            serde_json::json!({"baseColorFactor": [0.2, 0.3, 0.4, 1.0]}),
        )
    }

    /// The same container with a chosen `pbrMetallicRoughness`, for the surface tests.
    fn glb_with_surface(head_name: &str, pbr: serde_json::Value) -> Vec<u8> {
        let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let normals: [[f32; 3]; 3] = [[0.0, 0.0, 1.0]; 3];
        let indices: [u16; 3] = [0, 1, 2];

        let mut binary = Vec::new();
        for position in positions {
            for value in position {
                binary.extend_from_slice(&value.to_le_bytes());
            }
        }
        let normal_offset = binary.len();
        for normal in normals {
            for value in normal {
                binary.extend_from_slice(&value.to_le_bytes());
            }
        }
        let index_offset = binary.len();
        for index in indices {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }

        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [
                {"name": "Base", "children": [1], "mesh": 0},
                {"name": head_name, "translation": [0.0, 0.5, 0.0], "mesh": 0}
            ],
            "meshes": [{
                "primitives": [{
                    "attributes": {"POSITION": 0, "NORMAL": 1},
                    "indices": 2,
                    "material": 0
                }]
            }],
            "materials": [{"pbrMetallicRoughness": pbr}],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"}
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": normal_offset},
                {"buffer": 0, "byteOffset": normal_offset, "byteLength": index_offset - normal_offset},
                {"buffer": 0, "byteOffset": index_offset, "byteLength": 6}
            ],
            "buffers": [{"byteLength": binary.len()}]
        });
        let mut json = serde_json::to_vec(&json).expect("json");
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }

        let mut glb = Vec::new();
        glb.extend_from_slice(&GLB_MAGIC.to_le_bytes());
        glb.extend_from_slice(&2_u32.to_le_bytes());
        let total = 12 + 8 + json.len() + 8 + binary.len();
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        glb.extend_from_slice(&binary);
        glb
    }

    #[test]
    fn a_real_glb_container_is_read_into_parts() {
        let model = read_glb(&glb("Head")).expect("the model reads");
        assert_eq!(model.parts.len(), 2);
        assert_eq!(model.triangle_count(), 2);
        assert!(model.warnings.is_empty(), "{:?}", model.warnings);
        assert_eq!(model.parts[0].colour, [0.2, 0.3, 0.4]);
    }

    /// Fabric and aluminium in one rig have to shade differently, so the two factors travel with
    /// the colour. A material that names only a colour keeps the painted-housing defaults rather
    /// than glTF's own, which are rough bare metal and wrong for every lamp body in the set.
    #[test]
    fn a_parts_roughness_and_metallic_come_off_its_material() {
        let painted = read_glb(&glb("Head")).expect("the model reads");
        assert_eq!(painted.parts[0].roughness, 0.5);
        assert_eq!(painted.parts[0].metallic, 0.2);

        let drape = read_glb(&glb_with_surface(
            "Head",
            serde_json::json!({
                "baseColorFactor": [0.1, 0.1, 0.1, 1.0],
                "roughnessFactor": 0.96,
                "metallicFactor": 0.0
            }),
        ))
        .expect("the model reads");
        assert_eq!(drape.parts[0].roughness, 0.96);
        assert_eq!(drape.parts[0].metallic, 0.0);
    }

    /// Pan and tilt have to move something. A part named as the head is the head.
    #[test]
    fn a_node_named_as_the_head_becomes_the_moving_part() {
        let model = read_glb(&glb("Head")).expect("the model reads");
        assert!(
            model
                .parts
                .iter()
                .any(|part| part.kind == ModelPartKind::Head),
            "the head part was not recognised"
        );
        // Its transform is baked in, so the child sits above the base.
        let head = model
            .parts
            .iter()
            .find(|part| part.kind == ModelPartKind::Head)
            .expect("head");
        assert!(head.positions.iter().all(|position| position[1] >= 0.5));
    }

    /// The pivot has to sit in the head, not at the origin, or a tilt swings the head around the
    /// hanging point instead of pointing it.
    #[test]
    fn the_tilt_pivot_sits_in_the_head() {
        let model = read_glb(&glb("Head")).expect("the model reads");
        assert!(
            model.head_pivot.y >= 0.5,
            "the pivot is in the head that sits above the base: {:?}",
            model.head_pivot
        );

        // A model with nothing that tilts pivots on its own origin.
        let fixed = read_glb(&glb("Cover")).expect("the model reads");
        assert_eq!(fixed.head_pivot, Vec3::ZERO);
    }

    #[test]
    fn a_model_with_no_recognised_part_names_is_all_base() {
        let model = read_glb(&glb("Cover")).expect("the model reads");
        assert!(
            model
                .parts
                .iter()
                .all(|part| part.kind == ModelPartKind::Base)
        );
    }

    #[test]
    fn something_that_is_not_a_glb_is_refused_by_name() {
        let error = read_glb(b"not a model at all").expect_err("refused");
        assert!(error.0.contains("GLB"), "{error}");
    }

    #[test]
    fn a_truncated_container_is_refused_rather_than_read_past() {
        let mut bytes = glb("Head");
        bytes.truncate(bytes.len() - 40);
        assert!(read_glb(&bytes).is_err());
    }
}
