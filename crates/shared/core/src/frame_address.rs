//! Where a fixture's attribute lives in the engine's numbered frame.
//!
//! The engine resolves every frame into slots numbered when the patch compiled. A producer that
//! emits the same fixture-and-attribute pairs on every tick — a running Dynamic, most of all —
//! used to have each of them looked up by name again on every tick. Carrying the number instead
//! makes that lookup an array index, and the generation tag keeps a number from an old patch
//! from being read against a new one.

use crate::{AttributeKey, FixtureId};

/// One slot of one patch generation's frame.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FrameAddress {
    /// The patch generation whose numbering `slot` belongs to.
    pub generation: u64,
    pub slot: u32,
}

/// Answers where a pair lives, for producers that want to remember it.
pub trait FrameAddressResolver {
    /// The generation every address this resolver hands out belongs to.
    fn generation(&self) -> u64;

    /// The address of a pair, or nothing when the patch never numbered it.
    fn frame_address(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<FrameAddress>;
}
