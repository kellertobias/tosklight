//! The patch layers an MVR's fixtures land on.
//!
//! An MVR groups its objects into named layers, and an import keeps that grouping. A layer the show
//! already has receives the fixtures — matched by name, because a file's layer identities are its
//! own — and any other layer is created under the name the file gives it. The show's default layer
//! takes whatever the file puts on its own default layer or on no layer at all. Only a layer that
//! actually receives a fixture is created, so a file's scenery-only layers leave no empty layers
//! behind.

use light_mvr::MvrDocument;
use std::collections::HashMap;

/// The layer every patch has, whether or not the show stores an object for it.
pub const DEFAULT_PATCH_LAYER: &str = "default";

struct PatchLayer {
    id: String,
    name: String,
}

/// Where each MVR layer lands in the show, and which patch layers the import has to create.
pub struct MvrLayerPlan {
    declared: HashMap<String, String>,
    layers: Vec<PatchLayer>,
    targets: HashMap<String, String>,
    created: Vec<(String, serde_json::Value)>,
    next_order: i64,
}

impl MvrLayerPlan {
    /// `existing` are the show's stored `patch_layer` objects as `(object id, body)`.
    pub fn new(
        document: &MvrDocument,
        existing: impl IntoIterator<Item = (String, serde_json::Value)>,
    ) -> Self {
        let mut layers = Vec::new();
        let mut next_order = 0;
        for (id, body) in existing {
            let text = |key: &str| {
                body.get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned()
            };
            let stored_id = text("id");
            let order = body
                .get("order")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            next_order = next_order.max(order + 1);
            layers.push(PatchLayer {
                id: if stored_id.is_empty() { id } else { stored_id },
                name: text("name"),
            });
        }
        Self {
            declared: document
                .layers
                .iter()
                .map(|layer| (layer.id.trim().to_owned(), layer.name.trim().to_owned()))
                .collect(),
            layers,
            targets: HashMap::new(),
            created: Vec::new(),
            next_order,
        }
    }

    /// The patch layer a fixture on the MVR layer `layer` lands on, creating it on first use.
    pub fn layer_for(&mut self, layer: Option<&str>) -> String {
        let source = layer
            .map(str::trim)
            .filter(|layer| !layer.is_empty())
            .unwrap_or(DEFAULT_PATCH_LAYER)
            .to_owned();
        if let Some(target) = self.targets.get(&source) {
            return target.clone();
        }
        // A layer the file does not declare is named by its identity, as older files wrote it.
        let name = self
            .declared
            .get(&source)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| source.clone());
        let target = if let Some(existing) = self
            .layers
            .iter()
            .find(|layer| layer.name.eq_ignore_ascii_case(&name))
        {
            existing.id.clone()
        } else if source == DEFAULT_PATCH_LAYER || name.eq_ignore_ascii_case("default") {
            DEFAULT_PATCH_LAYER.to_owned()
        } else if self.layers.iter().any(|layer| layer.id == source) {
            source.clone()
        } else {
            self.create(&source, &name)
        };
        self.targets.insert(source, target.clone());
        target
    }

    /// The layers this import creates, as `(id, body)` `patch_layer` objects in the order they
    /// were first used.
    pub fn created(&self) -> &[(String, serde_json::Value)] {
        &self.created
    }

    fn create(&mut self, id: &str, name: &str) -> String {
        let body = serde_json::json!({
            "id": id,
            "name": name,
            "order": self.next_order,
            "locked": false,
            "visible2d": true,
            "visible3d": true,
        });
        self.next_order += 1;
        self.layers.push(PatchLayer {
            id: id.to_owned(),
            name: name.to_owned(),
        });
        self.created.push((id.to_owned(), body));
        id.to_owned()
    }
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;
