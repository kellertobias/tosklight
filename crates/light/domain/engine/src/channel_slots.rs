//! Where every channel's attributes live, worked out when the patch compiles.
//!
//! Resolving one channel asks for a handful of attributes by name — its own, the manufacturer name
//! behind it, the one that decides which function is in control, and each function's. Which of
//! them it asks for never changes, and neither does where the answer lives, for as long as the
//! patch stands. Only the values change.
//!
//! So the addresses are found once here rather than by hashing a name for every channel of every
//! fixture of every frame.

use light_core::FixtureId;
use light_fixture::{ChannelAttribute, PatchedFixture};
use rustc_hash::FxHashMap;

use crate::{Slot, SlotTable};

/// Where one channel reads each of its attributes from, for one head.
#[derive(Clone, Debug, Default)]
struct ChannelSlots {
    canonical: Option<Slot>,
    fixture: Option<Slot>,
    control: Option<Slot>,
    functions: Box<[Option<Slot>]>,
}

impl ChannelSlots {
    fn slot(&self, attribute: ChannelAttribute) -> Option<Slot> {
        match attribute {
            ChannelAttribute::Canonical => self.canonical,
            ChannelAttribute::Fixture => self.fixture,
            ChannelAttribute::Control => self.control,
            ChannelAttribute::Function(index) => self.functions.get(index).copied().flatten(),
        }
    }
}

/// Every head's channel addresses for one generation.
#[derive(Default)]
pub(crate) struct ChannelSlotIndex {
    heads: FxHashMap<FixtureId, Box<[ChannelSlots]>>,
}

impl ChannelSlotIndex {
    /// Work out where each channel of each head reads from.
    pub(crate) fn compile(fixtures: &[PatchedFixture], slots: &SlotTable) -> Self {
        let mut heads: FxHashMap<FixtureId, Box<[ChannelSlots]>> = FxHashMap::default();
        for fixture in fixtures {
            let Some(mode) = crate::fixture::profile_mode(fixture) else {
                continue;
            };
            for (head_index, head) in mode.heads.iter().enumerate() {
                let owner = crate::fixture::profile_head_owner(fixture, head_index, head);
                let mut channels = vec![ChannelSlots::default(); mode.channels.len()];
                for (channel_index, channel) in mode.channels.iter().enumerate() {
                    if channel.head_id != head.id {
                        continue;
                    }
                    channels[channel_index] = ChannelSlots {
                        canonical: slots.slot(owner, &channel.attribute),
                        fixture: slots.slot(owner, &channel.fixture_attribute),
                        // A channel's control attribute is synthesised from its id and is never
                        // declared by a profile, so it is never numbered. A control action that
                        // sets one reaches the frame through the unnumbered values instead.
                        control: None,
                        functions: channel
                            .functions
                            .iter()
                            .map(|function| slots.slot(owner, &function.attribute))
                            .collect(),
                    };
                }
                heads.insert(owner, channels.into_boxed_slice());
            }
        }
        Self { heads }
    }

    /// One head's channel addresses, found once for all of its channels.
    pub(crate) fn head(&self, owner: FixtureId) -> Option<HeadChannelSlots<'_>> {
        Some(HeadChannelSlots {
            channels: self.heads.get(&owner)?,
        })
    }
}

/// One head's addresses, indexed by channel.
#[derive(Clone, Copy)]
pub(crate) struct HeadChannelSlots<'a> {
    channels: &'a [ChannelSlots],
}

impl HeadChannelSlots<'_> {
    /// Where this channel reads one of its attributes from.
    pub(crate) fn slot(&self, channel_index: usize, attribute: ChannelAttribute) -> Option<Slot> {
        self.channels.get(channel_index)?.slot(attribute)
    }
}
