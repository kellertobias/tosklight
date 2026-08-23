//! The attributes a loaded show can name, numbered once so the engine never works with strings.
//!
//! A desk resolves an attribute by name at the moments an operator changes what the show contains
//! — loading it, patching a fixture, importing a profile — and by number for every frame after
//! that. The name remains what is written to disk, sent over the wire, and shown to an operator;
//! the number never leaves the process.

use rustc_hash::FxHashMap;

use super::configuration::{custom_descriptor, resolved_descriptor};
use super::{ATTRIBUTE_REGISTRY, AttributeDescriptor, AttributeKey, ResolvedAttributeDescriptor};

/// An attribute's place in the table of the loaded show.
///
/// Cheap to copy, compare, and hash, which is the point: a render compares numbers where it used
/// to hash and compare strings. Only meaningful against the table that issued it, and deliberately
/// neither serialisable nor persistable — a reloaded show numbers its attributes afresh.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttributeId(u32);

impl AttributeId {
    /// The position this id addresses. Only the owning table should need this.
    fn index(self) -> usize {
        self.0 as usize
    }

    /// This id's place in its table's numbering, for callers that address a parallel array by
    /// attribute rather than asking the table for an entry. Meaningless against any other table.
    pub fn ordinal(self) -> usize {
        self.0 as usize
    }
}

/// One attribute, with the answers a render would otherwise recompute from its name every frame.
#[derive(Clone, Debug)]
pub struct AttributeEntry {
    key: AttributeKey,
    built_in: Option<&'static AttributeDescriptor>,
    is_intensity: bool,
    is_position: bool,
}

impl AttributeEntry {
    /// The name this attribute is known by outside the process.
    pub fn key(&self) -> &AttributeKey {
        &self.key
    }

    /// Whether this attribute drives a fixture's brightness, decided once rather than by matching
    /// the end of its name on every frame.
    pub fn is_intensity(&self) -> bool {
        self.is_intensity
    }

    /// Whether this attribute aims a fixture, decided once for the same reason.
    pub fn is_position(&self) -> bool {
        self.is_position
    }

    /// This attribute's metadata. A built-in answers from the static registry; anything else
    /// describes itself from its own name, as it always has.
    pub fn descriptor(&self) -> ResolvedAttributeDescriptor<'_> {
        match self.built_in {
            Some(descriptor) => resolved_descriptor(descriptor),
            None => custom_descriptor(&self.key),
        }
    }
}

/// Every attribute the loaded show can name.
///
/// The table only ever grows. Patching a fixture whose profile mentions an attribute the show has
/// not seen appends it; nothing already numbered is renumbered, so an id handed out early in a
/// show's life stays valid for the rest of it, through any amount of repatching.
#[derive(Clone, Debug, Default)]
pub struct AttributeTable {
    entries: Vec<AttributeEntry>,
    /// A frame asks this map once per contribution and once per channel read, and the keys are
    /// attribute names the desk itself made. The default hasher answered those in SipHash, which
    /// the profile showed above every other leaf in a render.
    ids: FxHashMap<AttributeKey, AttributeId>,
}

impl AttributeTable {
    /// An empty table, as a freshly opened show starts with.
    pub fn new() -> Self {
        Self::default()
    }

    /// A table that already knows every built-in attribute, so the common names are numbered
    /// before a show mentions them.
    pub fn with_built_ins() -> Self {
        let mut table = Self::new();
        for descriptor in ATTRIBUTE_REGISTRY {
            table.intern(&AttributeKey(descriptor.id.into()));
        }
        table
    }

    /// The number for this name, appending it if the show has not named it before.
    ///
    /// This is where a name is hashed. Call it while loading, patching, or importing — never while
    /// rendering.
    pub fn intern(&mut self, key: &AttributeKey) -> AttributeId {
        if let Some(id) = self.ids.get(key) {
            return *id;
        }
        let id = AttributeId(self.entries.len() as u32);
        self.entries.push(AttributeEntry {
            key: key.clone(),
            built_in: ATTRIBUTE_REGISTRY
                .iter()
                .find(|descriptor| descriptor.id == &*key.0),
            is_intensity: key.is_intensity(),
            is_position: key.is_position(),
        });
        self.ids.insert(key.clone(), id);
        id
    }

    /// The number this name already has, or nothing if the show has never named it. Unlike
    /// [`Self::intern`] this leaves the table alone, so it is safe to ask about a name that
    /// arrived from outside.
    pub fn id(&self, key: &AttributeKey) -> Option<AttributeId> {
        self.ids.get(key).copied()
    }

    /// What is known about a numbered attribute.
    pub fn entry(&self, id: AttributeId) -> &AttributeEntry {
        &self.entries[id.index()]
    }

    /// The name to hand back to a file, a client, or an operator.
    pub fn key(&self, id: AttributeId) -> &AttributeKey {
        self.entry(id).key()
    }

    /// How many attributes the show has named.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every attribute in numbering order.
    pub fn entries(&self) -> impl Iterator<Item = (AttributeId, &AttributeEntry)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (AttributeId(index as u32), entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> AttributeKey {
        AttributeKey(name.into())
    }

    #[test]
    fn a_name_keeps_the_number_it_was_first_given() {
        let mut table = AttributeTable::new();
        let first = table.intern(&key("intensity"));
        table.intern(&key("pan"));
        assert_eq!(table.intern(&key("intensity")), first);
        assert_eq!(table.key(first), &key("intensity"));
    }

    #[test]
    fn every_fixture_shares_one_number_for_the_same_attribute() {
        let mut table = AttributeTable::with_built_ins();
        let dimmer_of_one_lamp = table.intern(&key("intensity"));
        let dimmer_of_another = table.intern(&key("intensity"));
        assert_eq!(dimmer_of_one_lamp, dimmer_of_another);
    }

    #[test]
    fn a_custom_attribute_is_numbered_like_any_other() {
        let mut table = AttributeTable::with_built_ins();
        let known = table.len();
        let custom = table.intern(&key("myFogSwirl"));
        assert_eq!(table.len(), known + 1);
        assert_eq!(table.key(custom), &key("myFogSwirl"));
        assert!(!table.entry(custom).descriptor().built_in);
    }

    #[test]
    fn patching_later_never_renumbers_what_is_already_stored() {
        let mut table = AttributeTable::with_built_ins();
        let stored = table.intern(&key("intensity"));
        for extra in 0..64 {
            table.intern(&key(&format!("late.attribute.{extra}")));
        }
        assert_eq!(table.id(&key("intensity")), Some(stored));
        assert_eq!(table.key(stored), &key("intensity"));
    }

    #[test]
    fn a_name_the_show_never_mentioned_has_no_number() {
        let table = AttributeTable::with_built_ins();
        assert_eq!(table.id(&key("neverPatched")), None);
    }

    #[test]
    fn the_shape_of_a_name_is_decided_once() {
        let mut table = AttributeTable::new();
        let head_dimmer = table.intern(&key("head2.intensity"));
        let tilt = table.intern(&key("position.tilt"));
        let colour = table.intern(&key("color"));
        assert!(table.entry(head_dimmer).is_intensity());
        assert!(table.entry(tilt).is_position());
        assert!(!table.entry(colour).is_intensity());
        assert!(!table.entry(colour).is_position());
    }

    #[test]
    fn a_built_in_describes_itself_from_the_registry() {
        let mut table = AttributeTable::new();
        let intensity = table.intern(&key("intensity"));
        let descriptor = table.entry(intensity).descriptor();
        assert!(descriptor.built_in);
        assert_eq!(descriptor.id, "intensity");
    }
}
