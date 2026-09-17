//! Numbered 3D models a layer can be mapped onto.
//!
//! A layer's `3D model` channel selects one of these by number. Slot zero is deliberately absent
//! and always means "draw flat"; usable models occupy `1..=255` and keep their number across
//! restarts, so a cue that maps a layer onto model 12 keeps doing so after the library is edited.
//!
//! The library records which stored file — or which [built-in model](BuiltinModel) — belongs to
//! which slot. An imported mesh is stored by the library adapter; this module owns only the
//! addressing rules, the transport-neutral mesh value the renderer draws, and the normalization
//! every mesh goes through.
//!
//! A new library holds the five built-in models in slots 1–5, Plane first. A layer whose selected
//! slot cannot be drawn is mapped onto the Plane.

use serde::{Deserialize, Serialize};

use crate::builtin_models::BuiltinModel;
use crate::layer::ModelMapping;

/// The most vertices one imported model may carry after triangulation. Far more than a stage
/// prop needs, and small enough that a stray photogrammetry scan cannot exhaust GPU memory.
pub const MAX_MODEL_VERTICES: usize = 4_000_000;

/// One assigned model slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelEntry {
    pub slot: u8,
    pub name: String,
    /// The stored file's name inside the library's model store. Never a path. Empty for a
    /// built-in model, which has no file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file: String,
    /// The built-in model this slot holds instead of an imported file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin: Option<BuiltinModel>,
    /// Vertex and triangle counts measured at import, so the library can describe a model
    /// without loading it.
    #[serde(default)]
    pub vertices: u32,
    #[serde(default)]
    pub triangles: u32,
}

impl ModelEntry {
    /// A slot holding a built-in model under its own name.
    pub fn builtin(slot: u8, model: BuiltinModel) -> Self {
        let geometry = model.geometry();
        Self {
            slot,
            name: model.label().to_owned(),
            file: String::new(),
            builtin: Some(model),
            vertices: u32::try_from(geometry.vertices.len()).unwrap_or(u32::MAX),
            triangles: u32::try_from(geometry.triangle_count()).unwrap_or(u32::MAX),
        }
    }

    /// A slot holding an imported file.
    pub fn imported(slot: u8, name: impl Into<String>, file: impl Into<String>) -> Self {
        Self {
            slot,
            name: name.into(),
            file: file.into(),
            builtin: None,
            vertices: 0,
            triangles: 0,
        }
    }
}

/// The numbered model slots.
///
/// `Default` is the library a new installation starts with: the built-in models in slots 1–5.
/// A stored library with no entries stays empty; an operator may have cleared every slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelLibrary {
    #[serde(default)]
    pub entries: Vec<ModelEntry>,
}

impl Default for ModelLibrary {
    fn default() -> Self {
        let mut library = Self::empty();
        library.add_builtins_to_free_slots();
        library
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelLibraryError {
    #[error("model slot 0 means draw flat and cannot hold a model")]
    OffSlot,
    #[error("model slot {slot} is assigned more than once")]
    DuplicateSlot { slot: u8 },
    #[error("model slot {slot} needs a name")]
    EmptyName { slot: u8 },
    #[error("model slot {slot} has an invalid stored file name")]
    InvalidFile { slot: u8 },
}

impl ModelLibrary {
    /// A library with every slot empty.
    pub const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Assigns each built-in model to its default slot when that slot is empty. An imported model
    /// already in the slot keeps its number, so a cue pointing at it is never redirected.
    /// Returns the built-in models that were added.
    pub fn add_builtins_to_free_slots(&mut self) -> Vec<BuiltinModel> {
        let mut added = Vec::new();
        for model in BuiltinModel::ALL {
            if self.resolve(model.default_slot()).is_none() {
                self.entries
                    .push(ModelEntry::builtin(model.default_slot(), model));
                added.push(model);
            }
        }
        self.entries.sort_by_key(|entry| entry.slot);
        added
    }

    pub fn resolve(&self, slot: u8) -> Option<&ModelEntry> {
        (slot != 0)
            .then(|| self.entries.iter().find(|entry| entry.slot == slot))
            .flatten()
    }

    /// Assigns or replaces a slot.
    pub fn assign(&mut self, entry: ModelEntry) -> Result<(), ModelLibraryError> {
        let entry = ModelEntry {
            name: entry.name.trim().to_owned(),
            ..entry
        };
        validate_entry(&entry)?;
        if let Some(existing) = self.entries.iter_mut().find(|e| e.slot == entry.slot) {
            *existing = entry;
        } else {
            self.entries.push(entry);
            self.entries.sort_by_key(|entry| entry.slot);
        }
        Ok(())
    }

    /// Renames an assigned slot. Returns false when the slot is empty.
    pub fn rename(&mut self, slot: u8, name: &str) -> Result<bool, ModelLibraryError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ModelLibraryError::EmptyName { slot });
        }
        Ok(
            match self.entries.iter_mut().find(|entry| entry.slot == slot) {
                Some(entry) => {
                    name.clone_into(&mut entry.name);
                    true
                }
                None => false,
            },
        )
    }

    /// Clears a slot, returning what it held.
    pub fn remove(&mut self, slot: u8) -> Option<ModelEntry> {
        let index = self.entries.iter().position(|entry| entry.slot == slot)?;
        Some(self.entries.remove(index))
    }

    pub fn validate(&self) -> Result<(), ModelLibraryError> {
        let mut occupied = [false; 256];
        for entry in &self.entries {
            validate_entry(entry)?;
            if occupied[usize::from(entry.slot)] {
                return Err(ModelLibraryError::DuplicateSlot { slot: entry.slot });
            }
            occupied[usize::from(entry.slot)] = true;
        }
        Ok(())
    }

    /// The file name a newly imported model for `slot` is stored under.
    pub fn stored_file_name(slot: u8) -> String {
        format!("model-{slot:03}.glb")
    }
}

fn validate_entry(entry: &ModelEntry) -> Result<(), ModelLibraryError> {
    if entry.slot == 0 {
        return Err(ModelLibraryError::OffSlot);
    }
    if entry.name.trim().is_empty() {
        return Err(ModelLibraryError::EmptyName { slot: entry.slot });
    }
    let file_is_valid = match entry.builtin {
        Some(_) => entry.file.is_empty(),
        None => is_plain_file_name(&entry.file),
    };
    if !file_is_valid {
        return Err(ModelLibraryError::InvalidFile { slot: entry.slot });
    }
    Ok(())
}

/// A stored name is a single, non-hidden file name: no separators, no parent references.
fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// What a layer's model selection resolved to this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStatus {
    /// Model zero: the layer draws flat, as it always did.
    Flat,
    /// The selected model resolved and the layer is drawn onto it.
    Mapped,
    /// The selected slot holds no model. The layer is mapped onto the default Plane.
    Missing,
    /// The slot is assigned but its stored file could not be loaded. The layer is mapped onto
    /// the default Plane.
    Unloadable,
}

impl ModelStatus {
    /// Resolves a mapping against the library and whatever the running process could load.
    pub fn of(
        mapping: ModelMapping,
        library: &ModelLibrary,
        unloadable: impl Fn(u8) -> bool,
    ) -> Self {
        if mapping.is_flat() {
            Self::Flat
        } else if library.resolve(mapping.model).is_none() {
            Self::Missing
        } else if unloadable(mapping.model) {
            Self::Unloadable
        } else {
            Self::Mapped
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Mapped => "mapped",
            Self::Missing => "missing",
            Self::Unloadable => "unloadable",
        }
    }
}

/// One mesh vertex in model space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// glTF texture coordinates: `(0, 0)` is the top-left of the layer image.
    pub uv: [f32; 2],
}

/// A triangle mesh, ready to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelGeometry {
    pub vertices: Vec<ModelVertex>,
    /// Triangle list indices into `vertices`.
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ModelGeometryError {
    #[error("the model contains no triangles")]
    Empty,
    #[error("the model has a triangle that refers to a vertex it does not contain")]
    IndexOutOfRange,
    #[error(
        "the model has no size: every vertex lies on the same point, or a coordinate is not a number"
    )]
    Degenerate,
    #[error("the model has more than {MAX_MODEL_VERTICES} vertices")]
    TooLarge,
}

impl ModelGeometry {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Checks the mesh and rescales it into the unit sphere around the origin.
    ///
    /// The centre is the centre of the axis-aligned bounds and the radius the farthest vertex
    /// from it, so every model — a 2 cm prop or a 40 m set piece — frames the same way at scale 1.
    pub fn normalized(mut self) -> Result<Self, ModelGeometryError> {
        if self.indices.len() < 3 || self.vertices.is_empty() {
            return Err(ModelGeometryError::Empty);
        }
        if self.vertices.len() > MAX_MODEL_VERTICES {
            return Err(ModelGeometryError::TooLarge);
        }
        self.indices.truncate(self.indices.len() / 3 * 3);
        let count = self.vertices.len();
        if self.indices.iter().any(|index| *index as usize >= count) {
            return Err(ModelGeometryError::IndexOutOfRange);
        }
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for vertex in &self.vertices {
            for axis in 0..3 {
                let value = vertex.position[axis];
                if !value.is_finite() {
                    return Err(ModelGeometryError::Degenerate);
                }
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value);
            }
        }
        let centre = [0, 1, 2].map(|axis| (low[axis] + high[axis]) * 0.5);
        let radius = self
            .vertices
            .iter()
            .map(|vertex| {
                (0..3)
                    .map(|axis| (vertex.position[axis] - centre[axis]).powi(2))
                    .sum::<f32>()
            })
            .fold(0.0_f32, f32::max)
            .sqrt();
        if !radius.is_finite() || radius <= f32::EPSILON {
            return Err(ModelGeometryError::Degenerate);
        }
        for vertex in &mut self.vertices {
            for (position, centre) in vertex.position.iter_mut().zip(centre) {
                *position = (*position - centre) / radius;
            }
            let length = vertex.normal.iter().map(|v| v * v).sum::<f32>().sqrt();
            vertex.normal = if length.is_finite() && length > f32::EPSILON {
                vertex.normal.map(|v| v / length)
            } else {
                [0.0, 0.0, 1.0]
            };
            if !vertex.uv.iter().all(|v| v.is_finite()) {
                vertex.uv = [0.0, 0.0];
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(slot: u8, name: &str) -> ModelEntry {
        ModelEntry {
            slot,
            name: name.to_owned(),
            file: ModelLibrary::stored_file_name(slot),
            builtin: None,
            vertices: 4,
            triangles: 2,
        }
    }

    #[test]
    fn slots_are_addressed_and_zero_is_flat() {
        let mut library = ModelLibrary::empty();
        assert!(library.resolve(0).is_none());
        library.assign(entry(12, " Cube ")).unwrap();
        library.assign(entry(3, "Sphere")).unwrap();
        assert_eq!(
            library
                .entries
                .iter()
                .map(|entry| entry.slot)
                .collect::<Vec<_>>(),
            vec![3, 12],
            "entries stay sorted by slot"
        );
        assert_eq!(library.resolve(12).unwrap().name, "Cube");
        assert_eq!(
            library.assign(entry(0, "Off")),
            Err(ModelLibraryError::OffSlot)
        );
    }

    #[test]
    fn assigning_an_occupied_slot_replaces_it_and_clearing_frees_it() {
        let mut library = ModelLibrary::empty();
        library.assign(entry(7, "Old")).unwrap();
        library.assign(entry(7, "New")).unwrap();
        assert_eq!(library.entries.len(), 1);
        assert_eq!(library.resolve(7).unwrap().name, "New");
        assert!(library.rename(7, "Renamed").unwrap());
        assert!(!library.rename(8, "Nothing").unwrap());
        assert_eq!(
            library.rename(7, "  "),
            Err(ModelLibraryError::EmptyName { slot: 7 })
        );
        assert_eq!(library.remove(7).unwrap().name, "Renamed");
        assert!(library.resolve(7).is_none());
        assert!(library.remove(7).is_none());
    }

    #[test]
    fn validation_refuses_duplicates_and_paths() {
        let library = ModelLibrary {
            entries: vec![entry(4, "A"), entry(4, "B")],
        };
        assert_eq!(
            library.validate(),
            Err(ModelLibraryError::DuplicateSlot { slot: 4 })
        );
        for file in ["../escape.glb", "a/b.glb", ".hidden.glb", ""] {
            let library = ModelLibrary {
                entries: vec![ModelEntry {
                    file: file.to_owned(),
                    ..entry(5, "Bad")
                }],
            };
            assert_eq!(
                library.validate(),
                Err(ModelLibraryError::InvalidFile { slot: 5 }),
                "{file}"
            );
        }
    }

    #[test]
    fn a_new_library_holds_every_built_in_model_with_the_plane_in_slot_one() {
        let library = ModelLibrary::default();
        library.validate().unwrap();
        assert_eq!(library.entries.len(), BuiltinModel::ALL.len());
        let plane = library.resolve(1).unwrap();
        assert_eq!(plane.builtin, Some(BuiltinModel::Plane));
        assert_eq!(plane.name, "Plane");
        assert!(plane.file.is_empty());
        assert_eq!((plane.vertices, plane.triangles), (4, 2));
        for model in BuiltinModel::ALL {
            let entry = library.resolve(model.default_slot()).unwrap();
            assert_eq!(entry.builtin, Some(model));
            assert_eq!(entry.name, model.label());
        }
        let text = serde_json::to_string(&library).unwrap();
        assert!(
            !text.contains("\"file\""),
            "a built-in slot stores no file: {text}"
        );
        assert_eq!(
            serde_json::from_str::<ModelLibrary>(&text).unwrap(),
            library
        );
    }

    #[test]
    fn built_ins_fill_only_free_default_slots() {
        let mut library = ModelLibrary::empty();
        library.assign(entry(1, "Imported")).unwrap();
        library.assign(entry(4, "Also imported")).unwrap();
        assert_eq!(
            library.add_builtins_to_free_slots(),
            vec![
                BuiltinModel::Cube,
                BuiltinModel::Sphere,
                BuiltinModel::Pyramid
            ]
        );
        assert_eq!(library.resolve(1).unwrap().name, "Imported");
        assert_eq!(library.resolve(4).unwrap().builtin, None);
        assert_eq!(
            library.resolve(5).unwrap().builtin,
            Some(BuiltinModel::Pyramid)
        );
        assert!(library.add_builtins_to_free_slots().is_empty());
        let slots: Vec<u8> = library.entries.iter().map(|e| e.slot).collect();
        assert_eq!(slots, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_built_in_slot_may_not_name_a_file_and_an_imported_one_must() {
        let mut builtin = ModelEntry::builtin(9, BuiltinModel::Cube);
        builtin.file = "model-009.glb".to_owned();
        assert_eq!(
            ModelLibrary::empty().assign(builtin),
            Err(ModelLibraryError::InvalidFile { slot: 9 })
        );
        assert_eq!(
            ModelLibrary::empty().assign(ModelEntry::imported(9, "Bare", "")),
            Err(ModelLibraryError::InvalidFile { slot: 9 })
        );
        let mut library = ModelLibrary::empty();
        library
            .assign(ModelEntry::imported(9, "Glb", "model-009.glb"))
            .unwrap();
        library
            .assign(ModelEntry::builtin(9, BuiltinModel::Sphere))
            .unwrap();
        assert_eq!(
            library.resolve(9).unwrap().builtin,
            Some(BuiltinModel::Sphere)
        );
    }

    #[test]
    fn the_library_round_trips_through_json_and_an_explicit_empty_one_stays_empty() {
        let mut library = ModelLibrary::empty();
        library.assign(entry(200, "Screen")).unwrap();
        let text = serde_json::to_string(&library).unwrap();
        assert_eq!(
            serde_json::from_str::<ModelLibrary>(&text).unwrap(),
            library
        );
        assert_eq!(
            serde_json::from_str::<ModelLibrary>("{}").unwrap(),
            ModelLibrary::empty()
        );
    }

    #[test]
    fn status_distinguishes_flat_missing_unloadable_and_mapped() {
        let mut library = ModelLibrary::empty();
        library.assign(entry(1, "Loads")).unwrap();
        library.assign(entry(2, "Broken")).unwrap();
        let mapping = |model| ModelMapping {
            model,
            ..Default::default()
        };
        let broken = |slot| slot == 2;
        assert_eq!(
            ModelStatus::of(mapping(0), &library, broken),
            ModelStatus::Flat
        );
        assert_eq!(
            ModelStatus::of(mapping(1), &library, broken),
            ModelStatus::Mapped
        );
        assert_eq!(
            ModelStatus::of(mapping(2), &library, broken),
            ModelStatus::Unloadable
        );
        assert_eq!(
            ModelStatus::of(mapping(3), &library, broken),
            ModelStatus::Missing
        );
    }

    fn vertex(x: f32, y: f32, z: f32) -> ModelVertex {
        ModelVertex {
            position: [x, y, z],
            normal: [0.0, 0.0, 2.0],
            uv: [0.0, 0.0],
        }
    }

    #[test]
    fn normalization_centres_and_fits_the_unit_sphere() {
        let geometry = ModelGeometry {
            vertices: vec![
                vertex(10.0, 10.0, 0.0),
                vertex(14.0, 10.0, 0.0),
                vertex(14.0, 13.0, 0.0),
            ],
            indices: vec![0, 1, 2],
        }
        .normalized()
        .unwrap();
        let farthest = geometry
            .vertices
            .iter()
            .map(|v| v.position.iter().map(|p| p * p).sum::<f32>().sqrt())
            .fold(0.0_f32, f32::max);
        assert!((farthest - 1.0).abs() < 1e-5, "{farthest}");
        // Bounds centre (12, 11.5) lands on the origin: (10,10) → (-2,-1.5)/2.5.
        assert!((geometry.vertices[0].position[0] + 0.8).abs() < 1e-5);
        assert!((geometry.vertices[0].position[1] + 0.6).abs() < 1e-5);
        assert_eq!(geometry.vertices[0].normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn broken_meshes_are_refused() {
        assert_eq!(
            ModelGeometry::default().normalized(),
            Err(ModelGeometryError::Empty)
        );
        let point = ModelGeometry {
            vertices: vec![vertex(1.0, 1.0, 1.0); 3],
            indices: vec![0, 1, 2],
        };
        assert_eq!(point.normalized(), Err(ModelGeometryError::Degenerate));
        let dangling = ModelGeometry {
            vertices: vec![vertex(0.0, 0.0, 0.0), vertex(1.0, 0.0, 0.0)],
            indices: vec![0, 1, 2],
        };
        assert_eq!(
            dangling.normalized(),
            Err(ModelGeometryError::IndexOutOfRange)
        );
    }
}
