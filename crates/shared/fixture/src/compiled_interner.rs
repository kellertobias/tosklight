//! TL-639: compiled per-profile models shared between instances with identical inputs.
//!
//! Patched fixtures of one profile revision share one `Arc<FixtureProfile>` snapshot, so the
//! snapshot's address (held alive by the entry) together with the mode identifies the profile
//! content exactly. Every other compile input (calibration, appearance, context) is compared by
//! value in `K`. Compiled models are immutable pure functions of those inputs, so an interned
//! result is the result a fresh compile would give.
use crate::FixtureProfile;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// A shared compile input compared by identity: equal only to the same allocation, which is
/// exact for immutable inputs and never merges two equal-content inputs by mistake.
#[derive(Clone, Debug)]
pub struct SharedByIdentity<T>(pub Arc<T>);

impl<T> PartialEq for SharedByIdentity<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

struct Entry<K, V> {
    /// Keeps the snapshot (and therefore its address) alive while the entry exists.
    profile: Arc<FixtureProfile>,
    inputs: K,
    model: V,
}

/// Interned compiled models by profile snapshot, mode and the remaining compile inputs `K`.
pub struct CompiledModelInterner<K, V> {
    entries: HashMap<(usize, Uuid), Vec<Entry<K, V>>>,
}

impl<K, V> Default for CompiledModelInterner<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<K: PartialEq, V: Clone> CompiledModelInterner<K, V> {
    /// The interned model for these exact inputs, compiling it on first use. `true` when the
    /// model was compiled now.
    pub fn get_or_compile(
        &mut self,
        profile: &Arc<FixtureProfile>,
        mode: Uuid,
        inputs: K,
        compile: impl FnOnce() -> V,
    ) -> (V, bool) {
        let bucket = self
            .entries
            .entry((Arc::as_ptr(profile) as usize, mode))
            .or_default();
        if let Some(entry) = bucket
            .iter()
            .find(|entry| Arc::ptr_eq(&entry.profile, profile) && entry.inputs == inputs)
        {
            return (entry.model.clone(), false);
        }
        let model = compile();
        bucket.push(Entry {
            profile: Arc::clone(profile),
            inputs,
            model: model.clone(),
        });
        (model, true)
    }

    /// Drop models of snapshots no fixture holds any more (only the interner keeps them).
    pub fn retain_live(&mut self) {
        self.entries.retain(|_, bucket| {
            bucket.retain(|entry| Arc::strong_count(&entry.profile) > 1);
            !bucket.is_empty()
        });
    }

    /// Distinct interned models.
    pub fn len(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests;
