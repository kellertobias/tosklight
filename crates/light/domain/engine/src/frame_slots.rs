//! The fixed shape of a frame: one dense slot for every value the patched show can produce.
//!
//! A show's shape changes when an operator patches it, not while it runs. Numbering every
//! (fixture, attribute) pair once when the patch compiles lets a render address its values by
//! integer for the rest of that generation's life, instead of hashing a `FixtureId` and an
//! attribute name a few tens of thousands of times per frame.
//!
//! An unpatched fixture is numbered like any other. It stays selectable, programmable, and
//! storable in groups and cues; only its DMX output is suppressed.

use light_core::{AttributeId, AttributeKey, AttributeTable, FixtureId};
use light_fixture::PatchedFixture;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Hands out a fresh tag every time a patch is compiled, so no two tables in one process life
/// ever number their slots under the same generation.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// The tag for a table about to be compiled.
pub(crate) fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// A value's place in a frame, valid only against the [`SlotTable`] generation that issued it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Slot(u32);

impl Slot {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    /// Address a slot by its position. Only frame storage shaped by the same table should need
    /// this; every other caller asks the table for a slot rather than inventing one.
    pub(crate) fn from_index(index: usize) -> Self {
        Self(index as u32)
    }
}

/// Marks a (fixture, attribute) column the show cannot produce.
const UNNUMBERED: u32 = u32::MAX;

/// Attributes every profile head can hold regardless of the channels its mode declares.
const SYNTHESISED_HEAD_ATTRIBUTES: &[&str] = &["intensity", "color"];

/// The two attributes every head is asked for before its channels are resolved.
///
/// Projection wants a head's colour and its level whatever the head is, so those two are found
/// when the patch compiles rather than by walking the fixture's attributes and comparing names
/// three times over for every head of every frame.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CommonSlots {
    pub(crate) intensity: Option<Slot>,
    pub(crate) color: Option<Slot>,
}

/// Every value the patched show can produce, numbered once.
///
/// Numbering is per generation. A repatch compiles a new table with a new generation tag, so a
/// consumer still holding the previous frame can tell that its slot numbers address a table that
/// no longer exists rather than silently reading another fixture's value.
pub(crate) struct SlotTable {
    generation: u64,
    attributes: AttributeTable,
    /// Where each fixture's column run starts, as a multiple of `stride`.
    rows: FxHashMap<FixtureId, u32>,
    /// `rows[fixture] * stride + attribute.ordinal()` addresses a slot, or [`UNNUMBERED`].
    columns: Vec<u32>,
    stride: usize,
    /// What each slot names, for the boundary that still speaks in names.
    pairs: Vec<(FixtureId, AttributeId)>,
    /// Each fixture's own slots, contiguous, so a head can be read without scanning the show.
    fixture_slots: FxHashMap<FixtureId, Vec<Slot>>,
    /// Where each fixture keeps the two attributes projection asks every head for by name.
    /// Answered from the row rather than by comparing names against the fixture's whole slot list.
    common: FxHashMap<FixtureId, CommonSlots>,
}

/// Hands out [`light_core::FrameAddress`]es for one generation's numbering.
///
/// Held by producers that emit the same pairs every tick, so the engine can read their samples by
/// number rather than by name.
#[derive(Clone)]
pub struct FrameAddresser {
    slots: std::sync::Arc<SlotTable>,
}

impl FrameAddresser {
    pub(crate) fn new(slots: std::sync::Arc<SlotTable>) -> Self {
        Self { slots }
    }
}

impl light_core::FrameAddressResolver for FrameAddresser {
    fn generation(&self) -> u64 {
        self.slots.generation()
    }

    fn frame_address(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<light_core::FrameAddress> {
        self.slots
            .slot(fixture_id, attribute)
            .map(|slot| light_core::FrameAddress {
                generation: self.slots.generation(),
                slot: slot.index() as u32,
            })
    }
}

/// Which attributes each fixture declares, collected before any column is addressed.
#[derive(Default)]
struct Declarations {
    owners: Vec<FixtureId>,
    owner_rows: FxHashMap<FixtureId, usize>,
    declared: Vec<Vec<AttributeId>>,
}

impl Declarations {
    /// The row an owner has, or the next one if it has none yet. Every fixture owns a row even
    /// when its profile declares nothing, so an unpatched or empty fixture is still addressable
    /// rather than absent.
    fn row(&mut self, owner: FixtureId) -> usize {
        *self.owner_rows.entry(owner).or_insert_with(|| {
            self.owners.push(owner);
            self.declared.push(Vec::new());
            self.owners.len() - 1
        })
    }

    fn declare(&mut self, owner: FixtureId, attribute: AttributeId) {
        let row = self.row(owner);
        if !self.declared[row].contains(&attribute) {
            self.declared[row].push(attribute);
        }
    }

    fn declare_fixture(&mut self, fixture: &PatchedFixture, attributes: &mut AttributeTable) {
        // Every fixture owns a row even when its profile declares nothing, so an unpatched or
        // empty fixture is still addressable rather than absent.
        self.row(fixture.fixture_id);
        if let Some(mode) = crate::fixture::profile_mode(fixture) {
            for (head_index, head) in mode.heads.iter().enumerate() {
                let owner = crate::fixture::profile_head_owner(fixture, head_index, head);
                // Every head can carry a colour and a level whether or not its mode names a
                // channel for them: projection composes both from the channels it does have,
                // and a Group colour or a virtual dimmer supplies them directly.
                for synthesised in SYNTHESISED_HEAD_ATTRIBUTES {
                    let id = attributes.intern(&AttributeKey((*synthesised).into()));
                    self.declare(owner, id);
                }
                for channel in mode
                    .channels
                    .iter()
                    .filter(|channel| channel.head_id == head.id)
                {
                    let id = attributes.intern(&channel.attribute);
                    self.declare(owner, id);
                    // The manufacturer's own name for the channel is read for every channel
                    // whose canonical name differs from it, so it is numbered too; otherwise
                    // that read is a miss that has to be answered by name.
                    if channel.fixture_attribute != channel.attribute {
                        let id = attributes.intern(&channel.fixture_attribute);
                        self.declare(owner, id);
                    }
                    for function in &channel.functions {
                        let id = attributes.intern(&function.attribute);
                        self.declare(owner, id);
                    }
                }
            }
        }
        for (head_index, head) in fixture.definition.heads.iter().enumerate() {
            let owner = if head.shared {
                fixture.fixture_id
            } else {
                fixture
                    .logical_heads
                    .iter()
                    .find(|logical| logical.head_index == head.index)
                    .map(|logical| logical.fixture_id)
                    .unwrap_or(fixture.fixture_id)
            };
            let _ = head_index;
            for parameter in &head.parameters {
                let id = attributes.intern(&parameter.attribute);
                self.declare(owner, id);
            }
        }
        // A fixture's safe values name attributes it must be able to hold, whether or not a
        // channel in the current mode declares them.
        for attribute in fixture.definition.safe_values.keys() {
            let id = attributes.intern(attribute);
            self.declare(fixture.fixture_id, id);
        }
        // A Freeze holds values that were resolvable when it was taken, so its targets keep
        // their numbering even if the underlying profile no longer offers them.
        for (fixture_id, target) in &fixture.freeze.targets {
            for attribute in target.values.keys() {
                let id = attributes.intern(attribute);
                self.declare(*fixture_id, id);
            }
        }
    }
}

impl SlotTable {
    /// Number every pair the fixtures of this generation can produce.
    pub(crate) fn compile(generation: u64, fixtures: &[PatchedFixture]) -> Self {
        let mut attributes = AttributeTable::with_built_ins();
        // Collected before any column is addressed, because the stride is the attribute count and
        // that is only final once every profile has been read.
        let mut declarations = Declarations::default();
        for fixture in fixtures {
            declarations.declare_fixture(fixture, &mut attributes);
        }
        let Declarations {
            owners, declared, ..
        } = declarations;

        let stride = attributes.len();
        let mut columns = vec![UNNUMBERED; owners.len().saturating_mul(stride)];
        let mut pairs = Vec::new();
        let mut fixture_slots: FxHashMap<FixtureId, Vec<Slot>> = FxHashMap::default();
        for (row, attribute_ids) in declared.iter().enumerate() {
            let owner = owners[row];
            let owned = fixture_slots.entry(owner).or_default();
            for attribute in attribute_ids {
                let column = row * stride + attribute.ordinal();
                if columns[column] != UNNUMBERED {
                    continue;
                }
                let slot = Slot(pairs.len() as u32);
                columns[column] = slot.0;
                pairs.push((owner, *attribute));
                owned.push(slot);
            }
        }
        let intensity = attributes.id(&AttributeKey("intensity".into()));
        let color = attributes.id(&AttributeKey("color".into()));
        let common = owners
            .iter()
            .enumerate()
            .map(|(row, owner)| {
                let at = |attribute: Option<AttributeId>| {
                    let attribute = attribute?;
                    match columns.get(row * stride + attribute.ordinal()).copied() {
                        Some(UNNUMBERED) | None => None,
                        Some(slot) => Some(Slot(slot)),
                    }
                };
                (
                    *owner,
                    CommonSlots {
                        intensity: at(intensity),
                        color: at(color),
                    },
                )
            })
            .collect();
        Self {
            generation,
            attributes,
            common,
            rows: owners
                .iter()
                .enumerate()
                .map(|(row, owner)| (*owner, row as u32))
                .collect(),
            columns,
            stride,
            pairs,
            fixture_slots,
        }
    }

    /// Every slot one fixture owns. Compiled with the patch, so reading a head costs a lookup
    /// rather than a scan of the show.
    /// The slots for the attributes every head is asked for by name.
    pub(crate) fn common(&self, fixture_id: FixtureId) -> CommonSlots {
        self.common.get(&fixture_id).copied().unwrap_or_default()
    }

    pub(crate) fn fixture_slots(&self, fixture_id: FixtureId) -> &[Slot] {
        self.fixture_slots
            .get(&fixture_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// The name a slot's attribute is known by.
    pub(crate) fn attribute_key(&self, slot: Slot) -> &AttributeKey {
        self.attributes.key(self.pairs[slot.index()].1)
    }

    /// The generation that numbered these slots. A frame carries this tag so a reader holding a
    /// frame across a repatch can tell that its numbers no longer address this table.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// How many values a frame of this shape holds.
    pub(crate) fn len(&self) -> usize {
        self.pairs.len()
    }

    /// The number for this name, or nothing when the show never named it.
    pub(crate) fn attribute_id(&self, attribute: &AttributeKey) -> Option<AttributeId> {
        self.attributes.id(attribute)
    }

    /// The slot for a pair the show can produce, addressed by number.
    pub(crate) fn slot_of(&self, fixture_id: FixtureId, attribute: AttributeId) -> Option<Slot> {
        let row = *self.rows.get(&fixture_id)? as usize;
        let column = row * self.stride + attribute.ordinal();
        match self.columns.get(column).copied() {
            Some(UNNUMBERED) | None => None,
            Some(slot) => Some(Slot(slot)),
        }
    }

    /// The slot for a pair still addressed by name. Hashes the name, so this belongs on the paths
    /// that store a value, not inside the frame loop.
    pub(crate) fn slot(&self, fixture_id: FixtureId, attribute: &AttributeKey) -> Option<Slot> {
        self.slot_of(fixture_id, self.attribute_id(attribute)?)
    }

    /// What a slot names, for the boundary that hands values back by name.
    pub(crate) fn pair(&self, slot: Slot) -> (FixtureId, &AttributeKey) {
        let (fixture_id, attribute) = self.pairs[slot.index()];
        (fixture_id, self.attributes.key(attribute))
    }
}

/// A schema-v1 fixture whose one shared head declares the named attributes, for tests in this
/// crate that need a show with a known shape.
#[cfg(test)]
pub(crate) fn legacy_test_fixture(fixture_id: FixtureId, attributes: &[&str]) -> PatchedFixture {
    use light_fixture::{
        ByteOrder, ChannelComponent, FixtureDefinition, LogicalHead, Parameter, PatchedHead,
        SignalLossPolicy,
    };
    use std::collections::BTreeMap;

    let parameter = |attribute: &&str| Parameter {
        attribute: AttributeKey((*attribute).into()),
        components: vec![ChannelComponent {
            offset: 0,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: light_fixture::ParameterMetadata::default(),
        capabilities: vec![],
    };
    PatchedFixture {
        fixture_id,
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Cell".into(),
        layer_id: "default".into(),
        note: None,
        position_master: None,
        definition: FixtureDefinition {
            schema_version: 1,
            id: FixtureId::new(),
            revision: 1,
            manufacturer: "Test".into(),
            device_type: "other".into(),
            name: "Cell".into(),
            model: "Cell".into(),
            mode: "test".into(),
            footprint: attributes.len() as u16,
            heads: vec![LogicalHead {
                index: 0,
                name: "Cell".into(),
                shared: true,
                parameters: attributes.iter().map(parameter).collect(),
            }],
            color_calibration: None,
            physical: Default::default(),
            model_asset: None,
            icon_asset: None,
            hazardous: false,
            direct_control_protocols: Vec::new(),
            signal_loss_policy: SignalLossPolicy::HoldLast,
            safe_values: BTreeMap::new(),
            profile_id: None,
            mode_id: None,
            profile_snapshot: None,
        },
        universe: Some(1),
        address: Some(1),
        split_patches: Vec::new(),
        direct_control: None,
        internal_bindings: Default::default(),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: Vec::<PatchedHead>::new(),
        multipatch: Vec::new(),
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
        freeze: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline_fixture(attributes: &[&str]) -> PatchedFixture {
        legacy_test_fixture(FixtureId::new(), attributes)
    }

    #[test]
    fn every_declared_pair_gets_its_own_slot() {
        let fixture = inline_fixture(&["intensity", "pan", "tilt"]);
        let table = SlotTable::compile(1, std::slice::from_ref(&fixture));
        let slots = ["intensity", "pan", "tilt"].map(|name| {
            table
                .slot(fixture.fixture_id, &AttributeKey(name.into()))
                .unwrap()
        });
        assert_eq!(table.len(), 3);
        assert_eq!(
            slots.iter().collect::<std::collections::HashSet<_>>().len(),
            3
        );
    }

    #[test]
    fn a_pair_the_show_cannot_produce_has_no_slot() {
        let fixture = inline_fixture(&["intensity"]);
        let table = SlotTable::compile(1, std::slice::from_ref(&fixture));
        assert!(
            table
                .slot(fixture.fixture_id, &AttributeKey("zoom".into()))
                .is_none()
        );
        assert!(
            table
                .slot(FixtureId::new(), &AttributeKey("intensity".into()))
                .is_none()
        );
    }

    #[test]
    fn an_unpatched_fixture_is_numbered_like_any_other() {
        let mut fixture = inline_fixture(&["intensity"]);
        fixture.universe = None;
        fixture.address = None;
        let table = SlotTable::compile(1, std::slice::from_ref(&fixture));
        assert!(
            table
                .slot(fixture.fixture_id, &AttributeKey("intensity".into()))
                .is_some()
        );
    }

    #[test]
    fn a_slot_names_the_pair_it_was_numbered_for() {
        let fixture = inline_fixture(&["intensity", "pan"]);
        let table = SlotTable::compile(7, std::slice::from_ref(&fixture));
        let pan = table
            .slot(fixture.fixture_id, &AttributeKey("pan".into()))
            .unwrap();
        assert_eq!(
            table.pair(pan),
            (fixture.fixture_id, &AttributeKey("pan".into()))
        );
        assert_eq!(table.generation(), 7);
    }

    #[test]
    fn two_fixtures_sharing_a_name_do_not_share_a_slot() {
        let first = inline_fixture(&["intensity"]);
        let second = inline_fixture(&["intensity"]);
        let table = SlotTable::compile(1, &[first.clone(), second.clone()]);
        let intensity = AttributeKey("intensity".into());
        assert_ne!(
            table.slot(first.fixture_id, &intensity),
            table.slot(second.fixture_id, &intensity)
        );
    }

    #[test]
    fn a_fixture_owns_exactly_the_slots_it_declared() {
        let first = inline_fixture(&["intensity", "pan"]);
        let second = inline_fixture(&["intensity"]);
        let table = SlotTable::compile(1, &[first.clone(), second.clone()]);
        assert_eq!(table.fixture_slots(first.fixture_id).len(), 2);
        assert_eq!(table.fixture_slots(second.fixture_id).len(), 1);
        assert!(table.fixture_slots(FixtureId::new()).is_empty());
    }
}
