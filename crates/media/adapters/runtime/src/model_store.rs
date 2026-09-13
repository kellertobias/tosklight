//! The 3D model library as the running process uses it.
//!
//! One instance per process, shared by every output and by the API. It imports uploads, loads
//! each assigned model once, keeps the parsed mesh for as long as its slot points at the same
//! file, and reports a model that is selected but cannot be drawn — once, never every frame. A
//! layer whose model is missing or unloadable draws flat; the API's `modelStatus` says why.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use media_domain::{ModelGeometry, ModelLibrary, OutputState};
use media_http::{ImportedModel, ModelRejection};
use media_library::models::{ModelStore, import_glb};
use media_render::ModelGeometries;

/// What the library resolved to: drawable meshes, and the slots that could not be loaded.
#[derive(Debug, Default)]
pub(crate) struct ResolvedModels {
    pub geometries: ModelGeometries,
    pub failures: Vec<(u8, String)>,
}

#[derive(Clone)]
pub struct Models {
    store: ModelStore,
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// The library `resolved` was built from.
    library: Option<ModelLibrary>,
    resolved: Arc<ResolvedModels>,
    /// Parsed meshes by stored file name, including the reason a file failed to load.
    loaded: HashMap<String, Result<Arc<ModelGeometry>, String>>,
    /// Slots already reported as selected but empty.
    reported_missing: HashSet<u8>,
}

impl Models {
    pub(crate) fn new(library_root: &Path) -> Self {
        Self {
            store: ModelStore::new(library_root),
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The drawable models for this library. Cheap when the library has not changed: every
    /// output compares the returned `Arc` to decide whether to upload anything.
    pub(crate) fn resolve(&self, library: &ModelLibrary) -> Arc<ResolvedModels> {
        let mut inner = self.lock();
        if inner.library.as_ref() == Some(library) {
            return Arc::clone(&inner.resolved);
        }
        let mut resolved = ResolvedModels::default();
        for entry in &library.entries {
            let result = inner.loaded.entry(entry.file.clone()).or_insert_with(|| {
                let loaded = self.store.load(&entry.file).map(Arc::new);
                if let Err(detail) = &loaded {
                    tracing::warn!(
                        slot = entry.slot,
                        name = %entry.name,
                        %detail,
                        "a 3D model cannot be loaded; layers selecting it draw flat"
                    );
                }
                loaded
            });
            match result {
                Ok(geometry) => {
                    resolved.geometries.insert(entry.slot, Arc::clone(geometry));
                }
                Err(detail) => resolved.failures.push((entry.slot, detail.clone())),
            }
        }
        let files: HashSet<&str> = library
            .entries
            .iter()
            .map(|entry| entry.file.as_str())
            .collect();
        inner.loaded.retain(|file, _| files.contains(file.as_str()));
        inner.library = Some(library.clone());
        inner.resolved = Arc::new(resolved);
        Arc::clone(&inner.resolved)
    }

    /// Validates and stores an upload. Nothing is written when the file is refused.
    pub(crate) fn import(&self, slot: u8, bytes: &[u8]) -> Result<ImportedModel, ModelRejection> {
        let geometry = import_glb(bytes).map_err(|error| {
            tracing::info!(slot, %error, "a 3D model upload was refused");
            ModelRejection {
                code: error.code().to_owned(),
                message: error.to_string(),
            }
        })?;
        let file = ModelLibrary::stored_file_name(slot);
        self.store.store(&file, bytes).map_err(|error| {
            tracing::error!(slot, %error, "a 3D model could not be stored");
            ModelRejection {
                code: "model-not-stored".to_owned(),
                message: format!("the model could not be saved to the library: {error}"),
            }
        })?;
        let imported = ImportedModel {
            file: file.clone(),
            vertices: u32::try_from(geometry.vertices.len()).unwrap_or(u32::MAX),
            triangles: u32::try_from(geometry.triangle_count()).unwrap_or(u32::MAX),
        };
        let mut inner = self.lock();
        inner.loaded.insert(file, Ok(Arc::new(geometry)));
        // The same file name now holds a different mesh: resolve again even if the library is
        // otherwise identical.
        inner.library = None;
        tracing::info!(
            slot,
            vertices = imported.vertices,
            triangles = imported.triangles,
            "3D model imported"
        );
        Ok(imported)
    }

    pub(crate) fn remove(&self, file: &str) -> Result<(), String> {
        self.store.remove(file).map_err(|error| error.to_string())?;
        let mut inner = self.lock();
        inner.loaded.remove(file);
        inner.library = None;
        Ok(())
    }

    /// Logs, once per slot, a layer that selects a model slot holding nothing.
    pub(crate) fn note_selections(&self, library: &ModelLibrary, output: &OutputState) {
        if output.layers.iter().all(|layer| layer.model.is_flat()) {
            return;
        }
        let mut inner = self.lock();
        for (index, layer) in output.layers.iter().enumerate() {
            let slot = layer.model.model;
            if slot == 0 {
                continue;
            }
            if library.resolve(slot).is_some() {
                inner.reported_missing.remove(&slot);
            } else if inner.reported_missing.insert(slot) {
                tracing::warn!(
                    output = %output.id,
                    layer = index + 1,
                    slot,
                    "a layer selects 3D model {slot}, but that slot holds no model; the layer draws flat"
                );
            }
        }
    }
}

/// Keeps one output's GPU copy of the models in step with the resolved library.
#[derive(Default)]
pub(crate) struct InstalledModels {
    current: Option<Arc<ResolvedModels>>,
}

impl InstalledModels {
    pub(crate) fn sync(
        &mut self,
        resolved: &Arc<ResolvedModels>,
        output: media_domain::OutputId,
        install: impl FnOnce(&ModelGeometries) -> Vec<(u8, String)>,
    ) {
        if self
            .current
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, resolved))
        {
            return;
        }
        for (slot, detail) in install(&resolved.geometries) {
            tracing::error!(
                %output,
                slot,
                %detail,
                "a 3D model cannot be drawn on this output; layers selecting it draw flat"
            );
        }
        self.current = Some(Arc::clone(resolved));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::ModelEntry;

    fn root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir()
            .join("media-runtime-models")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn library_with(imported: &ImportedModel, slot: u8) -> ModelLibrary {
        let mut library = ModelLibrary::default();
        library
            .assign(ModelEntry {
                slot,
                name: "Quad".to_owned(),
                file: imported.file.clone(),
                vertices: imported.vertices,
                triangles: imported.triangles,
            })
            .unwrap();
        library
    }

    #[test]
    fn an_imported_model_resolves_once_and_stays_shared() {
        let root = root("resolve");
        let models = Models::new(&root);
        let imported = models
            .import(4, &media_library::models::sample_textured_quad())
            .unwrap();
        assert_eq!((imported.vertices, imported.triangles), (4, 2));
        let library = library_with(&imported, 4);

        let first = models.resolve(&library);
        assert!(first.geometries.contains_key(&4));
        assert!(first.failures.is_empty());
        assert!(Arc::ptr_eq(&first, &models.resolve(&library)));

        // A fresh process loads the stored file from disk.
        let restarted = Models::new(&root).resolve(&library);
        assert_eq!(restarted.geometries[&4].triangle_count(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_refused_upload_stores_nothing() {
        let root = root("refuse");
        let models = Models::new(&root);
        let rejection = models.import(2, b"not a model").unwrap_err();
        assert_eq!(rejection.code, "model-not-glb");
        assert!(!root.join(".models").join("model-002.glb").exists());
    }

    #[test]
    fn an_unloadable_file_is_reported_not_drawn() {
        let root = root("unloadable");
        let models = Models::new(&root);
        let imported = models
            .import(9, &media_library::models::sample_textured_quad())
            .unwrap();
        let library = library_with(&imported, 9);
        std::fs::write(root.join(".models").join(&imported.file), b"damaged").unwrap();

        let resolved = Models::new(&root).resolve(&library);
        assert!(resolved.geometries.is_empty());
        assert_eq!(resolved.failures.len(), 1);
        assert_eq!(resolved.failures[0].0, 9);

        models.remove(&imported.file).unwrap();
        assert!(
            models.resolve(&library).failures[0]
                .1
                .contains("cannot read")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_output_uploads_only_when_the_resolved_library_changes() {
        let resolved = Arc::new(ResolvedModels::default());
        let mut installed = InstalledModels::default();
        let mut uploads = 0;
        for _ in 0..3 {
            installed.sync(&resolved, media_domain::OutputId::new(), |_| {
                uploads += 1;
                Vec::new()
            });
        }
        assert_eq!(uploads, 1);
    }
}
