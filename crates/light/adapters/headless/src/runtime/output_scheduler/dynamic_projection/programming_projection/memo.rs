//! Layered frame caches of the captured programming sources (TL-639 round 5).
//!
//! The frame's own sources keep one map per cache. A parallel worker's fork reads the frame's
//! maps as they stood when the section began (`frozen`), the entries its earlier groups added
//! (`chunk`) and the entries of the group it is composing (`group`). A group the frame must rerun
//! drops its `group` layer; a finished group moves it into `chunk`; the frame takes `chunk` back
//! in group order. Lookups never depend on the layer an entry sits in: a fork's group sees
//! exactly the entries the single-threaded loop would have seen for its targets.

use rustc_hash::FxHashMap;
use std::hash::Hash;

pub(super) struct Memo<'f, K, V> {
    group: FxHashMap<K, V>,
    chunk: FxHashMap<K, V>,
    frozen: Option<&'f FxHashMap<K, V>>,
}

impl<K, V> Default for Memo<'_, K, V> {
    fn default() -> Self {
        Self {
            group: FxHashMap::default(),
            chunk: FxHashMap::default(),
            frozen: None,
        }
    }
}

impl<'f, K: Eq + Hash + Clone, V: Clone> Memo<'f, K, V> {
    /// A fork reading `frozen` (the frame's map at the start of the section).
    pub fn fork(frozen: &'f FxHashMap<K, V>) -> Self {
        Self {
            frozen: Some(frozen),
            ..Self::default()
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.group
            .get(key)
            .or_else(|| self.chunk.get(key))
            .or_else(|| self.frozen.and_then(|frozen| frozen.get(key)))
    }

    /// The entry for `key`, inserting `make()` into the group layer when no layer has one.
    pub fn get_or_insert_with(&mut self, key: K, make: impl FnOnce() -> V) -> &V {
        if self.get(&key).is_none() {
            self.group.insert(key.clone(), make());
        }
        self.get(&key).expect("present or just inserted")
    }

    /// A writable entry for `key` in the group layer, copied up from a lower layer first.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if !self.group.contains_key(key) {
            let lower = self
                .chunk
                .get(key)
                .or_else(|| self.frozen.and_then(|frozen| frozen.get(key)))?
                .clone();
            self.group.insert(key.clone(), lower);
        }
        self.group.get_mut(key)
    }

    /// [`Self::get_mut`], starting from `V::default()` when no layer has `key`.
    pub fn entry_or_default(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        if self.get_mut(&key).is_none() {
            self.group.insert(key.clone(), V::default());
        }
        self.group.get_mut(&key).expect("present or just inserted")
    }

    /// The finished group's entries join the chunk.
    pub fn commit_group(&mut self) {
        if !self.group.is_empty() {
            self.chunk.extend(self.group.drain());
        }
    }

    /// The group is rerun by the frame: forget what it added.
    pub fn abort_group(&mut self) {
        self.group.clear();
    }

    /// The frame's own map, for the section's duration (the frame's sources keep theirs in the
    /// group layer).
    pub fn take_own(&mut self) -> FxHashMap<K, V> {
        std::mem::take(&mut self.group)
    }

    pub fn restore_own(&mut self, own: FxHashMap<K, V>) {
        self.group = own;
    }

    /// A fork's finished entries, to apply over the frame's map.
    pub fn into_chunk(mut self) -> FxHashMap<K, V> {
        self.commit_group();
        self.chunk
    }

    /// Apply a fork's entries (each replaces the frame's entry for its key: a fork copied any
    /// entry it changed).
    pub fn apply(&mut self, chunk: FxHashMap<K, V>) {
        self.group.extend(chunk);
    }

    /// The frame's own map (its group layer), for whole-map reads after a section.
    pub fn own(&self) -> &FxHashMap<K, V> {
        &self.group
    }
}

/// An ordered log with the same layers: the frame's entries, then each group's in order.
pub(super) struct Log<'f, T> {
    group: Vec<T>,
    chunk: Vec<T>,
    frozen: Option<&'f [T]>,
}

impl<T> Default for Log<'_, T> {
    fn default() -> Self {
        Self {
            group: Vec::new(),
            chunk: Vec::new(),
            frozen: None,
        }
    }
}

impl<'f, T: Clone> Log<'f, T> {
    pub fn fork(frozen: &'f [T]) -> Self {
        Self {
            frozen: Some(frozen),
            ..Self::default()
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.frozen
            .into_iter()
            .flatten()
            .chain(&self.chunk)
            .chain(&self.group)
    }

    pub fn push(&mut self, item: T) {
        self.group.push(item);
    }

    pub fn commit_group(&mut self) {
        self.chunk.append(&mut self.group);
    }

    pub fn abort_group(&mut self) {
        self.group.clear();
    }

    pub fn take_own(&mut self) -> Vec<T> {
        std::mem::take(&mut self.group)
    }

    pub fn restore_own(&mut self, own: Vec<T>) {
        self.group = own;
    }

    /// Entries of the finished groups of a fork.
    pub fn chunk_len(&self) -> usize {
        self.chunk.len()
    }

    pub fn into_chunk(mut self) -> Vec<T> {
        self.commit_group();
        self.chunk
    }

    pub fn apply(&mut self, chunk: Vec<T>) {
        self.group.extend(chunk);
    }

    pub fn own(&self) -> &[T] {
        &self.group
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fork_reads_through_and_drops_an_aborted_group() {
        let frozen = FxHashMap::from_iter([(1, "a"), (2, "b")]);
        let mut fork = Memo::fork(&frozen);
        assert_eq!(fork.get(&1), Some(&"a"));
        *fork.get_mut(&2).unwrap() = "B";
        fork.get_or_insert_with(3, || "c");
        fork.commit_group();
        fork.get_or_insert_with(4, || "d");
        *fork.get_mut(&1).unwrap() = "A";
        fork.abort_group();
        assert_eq!(fork.get(&4), None);
        assert_eq!(fork.get(&1), Some(&"a"));
        let chunk = fork.into_chunk();
        let mut own = Memo::default();
        own.restore_own(frozen.clone());
        own.apply(chunk);
        assert_eq!(
            own.own(),
            &FxHashMap::from_iter([(1, "a"), (2, "B"), (3, "c")])
        );
    }

    #[test]
    fn a_fork_log_keeps_order_and_drops_an_aborted_group() {
        let frozen = vec![1, 2];
        let mut fork = Log::fork(&frozen);
        fork.push(3);
        fork.commit_group();
        fork.push(4);
        fork.abort_group();
        fork.push(5);
        assert_eq!(fork.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 5]);
        let mut own = Log::default();
        own.restore_own(frozen.clone());
        own.apply(fork.into_chunk());
        assert_eq!(own.own(), &[1, 2, 3, 5]);
    }
}
