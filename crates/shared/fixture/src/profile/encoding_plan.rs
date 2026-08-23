use super::{FixtureMode, ProfileError};
use std::collections::HashMap;
use uuid::Uuid;

const MAX_CHANNEL_COMPONENTS: usize = 4;

/// Immutable physical-slot layout for one validated fixture mode.
///
/// Compiling this once keeps primary-slot derivation and channel lookup out of the render loop.
/// Raw values remain resolved by the engine at frame time; this plan only writes their bytes to
/// the mode's already-validated component slots.
#[derive(Clone, Debug)]
pub struct FixtureModeEncodingPlan {
    /// One entry per channel of the mode, in the mode's own order.
    ///
    /// A resolved channel arrives knowing which channel of the mode it is, so it is found by that
    /// position rather than by hashing its identity — twice, as validation and writing each did.
    ordered: Box<[CompiledChannelEncoding]>,
    channels: HashMap<Uuid, CompiledChannelEncoding>,
    split_footprints: HashMap<u16, u16>,
}

#[derive(Clone, Copy, Debug)]
struct CompiledChannelEncoding {
    split: u16,
    component_count: usize,
    slots: [u16; MAX_CHANNEL_COMPONENTS],
}

impl FixtureMode {
    pub fn compile_encoding_plan(&self) -> Result<FixtureModeEncodingPlan, ProfileError> {
        let primary_slots = self.primary_slots()?;
        let mut channels = HashMap::with_capacity(self.channels.len());
        for channel in &self.channels {
            let mut slots = [0; MAX_CHANNEL_COMPONENTS];
            slots[0] = primary_slots.get(&channel.id).copied().ok_or_else(|| {
                ProfileError::Invalid("compiled profile channel is missing a primary slot".into())
            })?;
            for (target, slot) in slots[1..].iter_mut().zip(&channel.secondary_slots) {
                *target = *slot;
            }
            channels.insert(
                channel.id,
                CompiledChannelEncoding {
                    split: channel.split,
                    component_count: channel.resolution.bytes(),
                    slots,
                },
            );
        }
        let ordered = self
            .channels
            .iter()
            .map(|channel| channels[&channel.id])
            .collect();
        Ok(FixtureModeEncodingPlan {
            ordered,
            channels,
            split_footprints: self
                .splits
                .iter()
                .map(|split| (split.number, split.footprint))
                .collect(),
        })
    }
}

impl FixtureModeEncodingPlan {
    // @tour fixture-semantics:40 Encode one checked DMX split
    // Every channel and address is validated before the first byte is written. The compiled plan
    // then writes coarse and fine components MSB-first into their physical slots.

    /// Encode every resolved channel belonging to `split` as one checked batch.
    ///
    /// Validation happens before the first byte is written, so an unknown channel or invalid base
    /// address cannot leave a partially updated frame.
    pub fn encode_split(
        &self,
        frame: &mut [u8; 512],
        base: u16,
        split: u16,
        values: &[(Uuid, u32)],
    ) -> Result<(), ProfileError> {
        let start = self.validate_batch(frame.len(), base, split, values)?;
        for &(channel_id, raw) in values {
            let encoding = self.channels[&channel_id];
            if encoding.split == split {
                encoding.write(frame, start, raw);
            }
        }
        Ok(())
    }

    /// Encode resolved channels that already know their place in the mode.
    ///
    /// The same checked batch as [`Self::encode_split`], without hashing a channel's identity to
    /// find where its bytes go.
    pub fn encode_split_by_index(
        &self,
        frame: &mut [u8; 512],
        base: u16,
        split: u16,
        values: &[(u32, u32)],
    ) -> Result<(), ProfileError> {
        let start = self.validate_indexed_batch(frame.len(), base, split, values)?;
        for &(index, raw) in values {
            let encoding = self.ordered[index as usize];
            if encoding.split == split {
                encoding.write(frame, start, raw);
            }
        }
        Ok(())
    }

    fn validate_indexed_batch(
        &self,
        frame_len: usize,
        base: u16,
        split: u16,
        values: &[(u32, u32)],
    ) -> Result<usize, ProfileError> {
        let mut start = None;
        for &(index, _) in values {
            let encoding = self.ordered.get(index as usize).ok_or_else(|| {
                ProfileError::Invalid("resolved channel is not part of this mode".into())
            })?;
            if encoding.split != split {
                continue;
            }
            let frame_start = *start.get_or_insert_with(|| usize::from(base.saturating_sub(1)));
            if base == 0 || !encoding.fits(frame_start, frame_len) {
                return Err(ProfileError::Invalid(
                    "encoded channel exceeds its universe".into(),
                ));
            }
        }
        Ok(start.unwrap_or(0))
    }

    pub fn split_footprint(&self, split: u16) -> Option<u16> {
        self.split_footprints.get(&split).copied()
    }

    fn validate_batch(
        &self,
        frame_len: usize,
        base: u16,
        split: u16,
        values: &[(Uuid, u32)],
    ) -> Result<usize, ProfileError> {
        let mut start = None;
        for &(channel_id, _) in values {
            let encoding = self.channels.get(&channel_id).ok_or_else(|| {
                ProfileError::Invalid("resolved profile channel is missing".into())
            })?;
            if encoding.split != split {
                continue;
            }
            let frame_start = *start.get_or_insert_with(|| usize::from(base.saturating_sub(1)));
            if base == 0 || !encoding.fits(frame_start, frame_len) {
                return Err(ProfileError::Invalid(
                    "encoded channel exceeds its universe".into(),
                ));
            }
        }
        Ok(start.unwrap_or(0))
    }
}

impl CompiledChannelEncoding {
    fn fits(self, start: usize, frame_len: usize) -> bool {
        self.slots[..self.component_count]
            .iter()
            .all(|slot| start + usize::from(slot.saturating_sub(1)) < frame_len)
    }

    fn write(self, frame: &mut [u8; 512], start: usize, raw: u32) {
        for (index, slot) in self.slots[..self.component_count].iter().enumerate() {
            let shift = 8 * (self.component_count - index - 1);
            frame[start + usize::from(slot - 1)] = ((raw >> shift) & 0xff) as u8;
        }
    }
}
