//! Stable DMX frame types shared by render and delivery code.

pub const DMX_SLOTS: usize = 512;
pub type DmxFrame = [u8; DMX_SLOTS];
