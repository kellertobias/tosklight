use crate::contribution::ApplicableSequenceMaster;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use rustc_hash::FxHashMap;

/// Per-fixture view of one render's resolved values.
///
/// A frame resolved into slots is read straight through its numbering: the patch already grouped
/// each fixture's slots when it compiled, so there is nothing to index. A projection assembled
/// from maps someone else handed over — visualization overrides, notably — is scanned into
/// per-fixture lists once instead of once per head.
pub(crate) enum ProfileValueIndex<'a> {
    Dense {
        frame: &'a crate::contribution::ResolvedFrame,
        /// Where each channel of each head reads from, worked out when the patch compiled.
        channels: &'a crate::ChannelSlotIndex,
    },
    Scanned {
        values: FxHashMap<FixtureId, Vec<(&'a AttributeKey, &'a AttributeValue)>>,
        sequence_masters: FxHashMap<FixtureId, Vec<(&'a AttributeKey, ApplicableSequenceMaster)>>,
    },
}

impl<'a> ProfileValueIndex<'a> {
    pub(crate) fn new(
        values: &'a crate::FrameValues,
        sequence_masters: &'a FxHashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
        channels: &'a crate::ChannelSlotIndex,
    ) -> Self {
        match values.frame() {
            Some(frame) => Self::Dense { frame, channels },
            None => Self::Scanned {
                values: index_values(values.values()),
                sequence_masters: index_sequence_masters(sequence_masters),
            },
        }
    }

    /// One of a channel's attributes, read from where the patch said it lives.
    ///
    /// Falls back to the attribute's name only when the patch could not number it and the frame
    /// actually holds something unnumbered; otherwise an absent address means absent.
    pub(crate) fn value_at(
        &self,
        head: HeadRead<'a>,
        channel_index: usize,
        which: light_fixture::ChannelAttribute,
        attribute: &AttributeKey,
    ) -> Option<&'a AttributeValue> {
        if let (Self::Dense { frame, .. }, Some(addresses)) = (self, head.addresses) {
            if let Some(slot) = addresses.slot(channel_index, which) {
                return frame.value(slot);
            }
            if !head.overflowed {
                return None;
            }
        }
        self.value(head.owner, attribute)
    }

    /// The sequence master scaling one of a channel's attributes, read the same way.
    pub(crate) fn sequence_master_at(
        &self,
        head: HeadRead<'a>,
        channel_index: usize,
        which: light_fixture::ChannelAttribute,
        attribute: &AttributeKey,
    ) -> Option<ApplicableSequenceMaster> {
        if let (Self::Dense { frame, .. }, Some(addresses)) = (self, head.addresses) {
            if let Some(slot) = addresses.slot(channel_index, which) {
                return frame.sequence_master(slot);
            }
            if !head.overflowed {
                return None;
            }
        }
        self.sequence_master(head.owner, attribute)
    }

    /// Everything one head's channels need to know before reading, found once per head.
    ///
    /// Whether the head has to consider unnumbered values is decided here rather than per
    /// channel: a frame that overflowed for one fixture used to send every other fixture's
    /// unnumbered reads through a hash of the attribute name, for a value that was never there.
    pub(crate) fn head_read(&self, owner: FixtureId) -> HeadRead<'a> {
        let (addresses, overflowed) = match self {
            Self::Dense { frame, channels } => (
                channels.head(owner),
                frame.has_overflow() && !frame.overflow(owner).is_empty(),
            ),
            Self::Scanned { .. } => (None, true),
        };
        HeadRead {
            owner,
            addresses,
            overflowed,
        }
    }

    pub(crate) fn values(&self, fixture_id: FixtureId) -> crate::HeadValues {
        let mut values = crate::HeadValues::default();
        match self {
            Self::Dense { frame, .. } => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if let Some(value) = frame.value(*slot) {
                        values.insert(frame.slots().attribute_key(*slot).clone(), value.clone());
                    }
                }
                for (attribute, winner) in frame.overflow(fixture_id) {
                    values.insert(attribute.clone(), winner.value.clone());
                }
            }
            Self::Scanned {
                values: scanned, ..
            } => {
                for (attribute, value) in scanned.get(&fixture_id).into_iter().flatten() {
                    values.insert((*attribute).clone(), (*value).clone());
                }
            }
        }
        values
    }

    pub(crate) fn value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&'a AttributeValue> {
        match self {
            Self::Dense { frame, .. } => match frame.slots().slot(fixture_id, attribute) {
                Some(slot) => frame.value(slot),
                None => frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| candidate == attribute)
                    .map(|(_, winner)| &winner.value),
            },
            Self::Scanned { values, .. } => values
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, value)| (*candidate == attribute).then_some(*value)),
        }
    }

    /// The colour and level of one head, found from its row rather than by name.
    ///
    /// Projection asks every head for both before it resolves a channel, so answering them by
    /// comparing names against the fixture's whole attribute list is work every head pays and no
    /// head needs.
    pub(crate) fn common(&self, fixture_id: FixtureId) -> HeadCommon<'a> {
        let Self::Dense { frame, .. } = self else {
            return HeadCommon {
                intensity: self.value_named(fixture_id, "intensity"),
                color: self.value_named(fixture_id, "color"),
                intensity_master: self.sequence_master_named(fixture_id, "intensity"),
            };
        };
        let common = frame.slots().common(fixture_id);
        HeadCommon {
            intensity: common.intensity.and_then(|slot| frame.value(slot)),
            color: common.color.and_then(|slot| frame.value(slot)),
            intensity_master: common
                .intensity
                .and_then(|slot| frame.sequence_master(slot)),
        }
    }

    pub(crate) fn value_named(
        &self,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> Option<&'a AttributeValue> {
        match self {
            Self::Dense { frame, .. } => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if &*frame.slots().attribute_key(*slot).0 == attribute {
                        return frame.value(*slot);
                    }
                }
                frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| &*candidate.0 == attribute)
                    .map(|(_, winner)| &winner.value)
            }
            Self::Scanned { values, .. } => values
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, value)| (&*candidate.0 == attribute).then_some(*value)),
        }
    }

    pub(crate) fn sequence_masters(&self, fixture_id: FixtureId) -> crate::HeadSequenceMasters {
        let mut masters = crate::HeadSequenceMasters::default();
        match self {
            Self::Dense { frame, .. } => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if let Some(master) = frame.sequence_master(*slot) {
                        masters.insert(frame.slots().attribute_key(*slot).clone(), master);
                    }
                }
                for (attribute, winner) in frame.overflow(fixture_id) {
                    if let Some(master) = winner.sequence_master {
                        masters.insert(attribute.clone(), master);
                    }
                }
            }
            Self::Scanned {
                sequence_masters, ..
            } => {
                for (attribute, master) in sequence_masters.get(&fixture_id).into_iter().flatten() {
                    masters.insert((*attribute).clone(), *master);
                }
            }
        }
        masters
    }

    pub(crate) fn sequence_master(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<ApplicableSequenceMaster> {
        match self {
            Self::Dense { frame, .. } => match frame.slots().slot(fixture_id, attribute) {
                Some(slot) => frame.sequence_master(slot),
                None => frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| candidate == attribute)
                    .and_then(|(_, winner)| winner.sequence_master),
            },
            Self::Scanned {
                sequence_masters, ..
            } => sequence_masters
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, master)| (*candidate == attribute).then_some(*master)),
        }
    }

    pub(crate) fn sequence_master_named(
        &self,
        fixture_id: FixtureId,
        attribute: &str,
    ) -> Option<ApplicableSequenceMaster> {
        match self {
            Self::Dense { frame, .. } => {
                for slot in frame.slots().fixture_slots(fixture_id) {
                    if &*frame.slots().attribute_key(*slot).0 == attribute {
                        return frame.sequence_master(*slot);
                    }
                }
                frame
                    .overflow(fixture_id)
                    .iter()
                    .find(|(candidate, _)| &*candidate.0 == attribute)
                    .and_then(|(_, winner)| winner.sequence_master)
            }
            Self::Scanned {
                sequence_masters, ..
            } => sequence_masters
                .get(&fixture_id)?
                .iter()
                .find_map(|(candidate, master)| (&*candidate.0 == attribute).then_some(*master)),
        }
    }
}

fn index_values(
    values: &crate::ResolvedValues,
) -> FxHashMap<FixtureId, Vec<(&AttributeKey, &AttributeValue)>> {
    let mut indexed = FxHashMap::<FixtureId, Vec<_>>::default();
    for ((fixture_id, attribute), value) in values {
        indexed
            .entry(*fixture_id)
            .or_default()
            .push((attribute, value));
    }
    indexed
}

fn index_sequence_masters(
    masters: &FxHashMap<(FixtureId, AttributeKey), ApplicableSequenceMaster>,
) -> FxHashMap<FixtureId, Vec<(&AttributeKey, ApplicableSequenceMaster)>> {
    let mut indexed = FxHashMap::<FixtureId, Vec<_>>::default();
    for ((fixture_id, attribute), master) in masters {
        indexed
            .entry(*fixture_id)
            .or_default()
            .push((attribute, *master));
    }
    indexed
}

/// Where one head reads from, and whether it has to look past the numbered frame at all.
#[derive(Clone, Copy)]
pub(crate) struct HeadRead<'a> {
    pub(crate) owner: FixtureId,
    pub(crate) addresses: Option<crate::HeadChannelSlots<'a>>,
    /// True only when this head's fixture has values the patch could not number.
    pub(crate) overflowed: bool,
}

/// What projection asks of every head before it looks at a single channel.
#[derive(Clone, Copy)]
pub(crate) struct HeadCommon<'a> {
    pub(crate) intensity: Option<&'a AttributeValue>,
    pub(crate) color: Option<&'a AttributeValue>,
    pub(crate) intensity_master: Option<ApplicableSequenceMaster>,
}
