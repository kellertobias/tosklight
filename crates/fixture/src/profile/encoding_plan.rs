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
    channels: HashMap<Uuid, CompiledChannelEncoding>,
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
        Ok(FixtureModeEncodingPlan { channels })
    }
}

impl FixtureModeEncodingPlan {
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
