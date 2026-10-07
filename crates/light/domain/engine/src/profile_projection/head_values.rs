//! TL-639 round 2: one head's values while it is projected, read through the frame instead of
//! copied out of it.
//!
//! A head used to start from an owned map of every value its fixture resolved, and a fitted
//! native channel built one more map of its sequence masters, for every head of every frame. The
//! views below answer each lookup exactly as such a map would (an unnumbered value of the same
//! name wins over a numbered one, the later of two unnumbered values wins), and keep only what a
//! head writes itself: overlays, safe values, a requested colour's channels.
use crate::ProfileValueIndex;
use crate::profile_value_index::HeadRead;
use light_core::{AttributeKey, AttributeValue, FixtureId};

/// A head's own writes, by name; `None` removes the frame's value of that name.
///
/// A head writes at most a few names (a native channel two), so they are kept inline and found by
/// comparison, without hashing or allocating; more spill into a list.
#[derive(Default)]
struct LocalValues {
    inline: [Option<(AttributeKey, Option<AttributeValue>)>; 3],
    spill: Vec<(AttributeKey, Option<AttributeValue>)>,
}

impl LocalValues {
    fn entries(&self) -> impl Iterator<Item = &(AttributeKey, Option<AttributeValue>)> {
        self.inline.iter().flatten().chain(self.spill.iter())
    }

    fn is_empty(&self) -> bool {
        self.inline[0].is_none() && self.spill.is_empty()
    }

    fn get(&self, attribute: &AttributeKey) -> Option<&Option<AttributeValue>> {
        if self.is_empty() {
            return None;
        }
        self.entries()
            .find(|(key, _)| key == attribute)
            .map(|(_, value)| value)
    }

    fn contains_key(&self, attribute: &AttributeKey) -> bool {
        self.get(attribute).is_some()
    }

    fn insert(&mut self, attribute: AttributeKey, value: Option<AttributeValue>) {
        for (key, current) in self
            .inline
            .iter_mut()
            .flatten()
            .chain(self.spill.iter_mut())
        {
            if *key == attribute {
                *current = value;
                return;
            }
        }
        match self.inline.iter_mut().find(|entry| entry.is_none()) {
            Some(free) => *free = Some((attribute, value)),
            None => self.spill.push((attribute, value)),
        }
    }

    /// Removes one name. Inline entries stay packed from the front, so `is_empty` reads one
    /// slot and `insert` fills the first free one.
    fn remove(&mut self, attribute: &AttributeKey) {
        let found = self
            .inline
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|(key, _)| key == attribute));
        let Some(position) = found else {
            self.spill.retain(|(key, _)| key != attribute);
            return;
        };
        let last = self.inline.len() - 1;
        for index in position..last {
            self.inline[index] = self.inline[index + 1].take();
        }
        self.inline[last] = (!self.spill.is_empty()).then(|| self.spill.remove(0));
    }
}

/// The values of one head: its own writes over the frame's row of its fixture.
pub(crate) struct HeadValueView<'v, 'a> {
    base: Option<(&'v ProfileValueIndex<'a>, HeadRead<'a>)>,
    local: LocalValues,
}

impl<'v, 'a> HeadValueView<'v, 'a> {
    /// Every value `owner` resolved this frame.
    pub(crate) fn over(values: &'v ProfileValueIndex<'a>, owner: FixtureId) -> Self {
        Self {
            base: Some((values, values.head_read(owner))),
            local: LocalValues::default(),
        }
    }

    /// Only what the head writes itself.
    pub(crate) fn local() -> Self {
        Self {
            base: None,
            local: LocalValues::default(),
        }
    }

    pub(crate) fn get(&self, attribute: &AttributeKey) -> Option<&AttributeValue> {
        match self.local.get(attribute) {
            Some(local) => local.as_ref(),
            None => self
                .base
                .and_then(|(values, read)| values.head_value(read.owner, attribute)),
        }
    }

    pub(crate) fn contains_key(&self, attribute: &AttributeKey) -> bool {
        self.get(attribute).is_some()
    }

    pub(crate) fn insert(&mut self, attribute: AttributeKey, value: AttributeValue) {
        self.local.insert(attribute, Some(value));
    }

    pub(crate) fn remove(&mut self, attribute: &AttributeKey) {
        if self.base.is_some() {
            self.local.insert(attribute.clone(), None);
        } else {
            self.local.remove(attribute);
        }
    }

    /// Every name with a value, each once, in no particular order.
    pub(crate) fn keys(&self) -> Vec<AttributeKey> {
        let mut keys = Vec::new();
        if let Some((values, read)) = self.base {
            values.for_each_head_attribute(read.owner, |attribute| {
                if !self.local.contains_key(attribute) && !keys.contains(attribute) {
                    keys.push(attribute.clone());
                }
            });
        }
        keys.extend(
            self.local
                .entries()
                .filter(|(_, value)| value.is_some())
                .map(|(attribute, _)| attribute.clone()),
        );
        keys
    }
}

/// Where the safety helpers read and write a head's values.
pub(crate) trait HeadValueStore {
    fn value(&self, attribute: &AttributeKey) -> Option<&AttributeValue>;
    fn store(&mut self, attribute: AttributeKey, value: AttributeValue);
}

impl HeadValueStore for crate::HeadValues {
    fn value(&self, attribute: &AttributeKey) -> Option<&AttributeValue> {
        self.get(attribute)
    }

    fn store(&mut self, attribute: AttributeKey, value: AttributeValue) {
        self.insert(attribute, value);
    }
}

impl HeadValueStore for HeadValueView<'_, '_> {
    fn value(&self, attribute: &AttributeKey) -> Option<&AttributeValue> {
        self.get(attribute)
    }

    fn store(&mut self, attribute: AttributeKey, value: AttributeValue) {
        self.insert(attribute, value);
    }
}
