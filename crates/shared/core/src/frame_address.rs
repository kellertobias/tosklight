//! Where a fixture's attribute lives in the engine's numbered frame.
//!
//! The engine resolves every frame into slots numbered when the patch compiled. A producer that
//! emits the same fixture-and-attribute pairs on every tick — a running Dynamic, most of all —
//! used to have each of them looked up by name again on every tick. Carrying the number instead
//! makes that lookup an array index, and the generation tag keeps a number from an old patch
//! from being read against a new one.

use crate::programming::ProgrammingComponent;
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

/// A component edit addresses its whole-family slot. It cannot contribute a separate output
/// winner. Recompile this address whenever its owner's patch generation changes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ComponentFrameAddress {
    pub owner: FrameAddress,
    pub component: ProgrammingComponent,
}
impl ComponentFrameAddress {
    pub fn resolve(
        resolver: &dyn FrameAddressResolver,
        fixture: FixtureId,
        component: ProgrammingComponent,
    ) -> Option<Self> {
        let owner = resolver.frame_address(fixture, &component.owner().key())?;
        (owner.generation == resolver.generation()).then_some(Self { owner, component })
    }
    pub const fn is_current(self, generation: u64) -> bool {
        self.owner.generation == generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Resolver {
        generation: u64,
    }
    impl FrameAddressResolver for Resolver {
        fn generation(&self) -> u64 {
            self.generation
        }
        fn frame_address(
            &self,
            _fixture: FixtureId,
            attribute: &AttributeKey,
        ) -> Option<FrameAddress> {
            (attribute.0.as_ref() == "position").then_some(FrameAddress {
                generation: self.generation,
                slot: 42,
            })
        }
    }
    #[test]
    fn pan_and_target_address_one_owner_and_stale_generation_is_rejected() {
        let resolver = Resolver { generation: 7 };
        let fixture = FixtureId::new();
        let pan =
            ComponentFrameAddress::resolve(&resolver, fixture, ProgrammingComponent::Pan).unwrap();
        let target =
            ComponentFrameAddress::resolve(&resolver, fixture, ProgrammingComponent::TargetX)
                .unwrap();
        assert_eq!(pan.owner, target.owner);
        assert!(pan.is_current(7));
        assert!(!pan.is_current(8));
    }
}
