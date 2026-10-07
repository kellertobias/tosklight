//! Physical instances of one optics destination (TL-548 C3).
//!
//! The engine installs a family's native writes only when every physical instance of the owner,
//! the root and each multipatch copy, carries its complete footprint (`project_family_native`).
//! Optics bindings are mode-level: the engine compiles each instance's forward model from the
//! mode alone (`physical_projection::Plan::compile`, `CompiledOpticsForward::compile(mode)`), and
//! copies duplicate the root's resolved channels. Each copy's own compiled optics model is
//! therefore the root's fitter, and fitting the same request against the same `current` raw
//! produces the same write. The adapter fits once and writes that raw explicitly on every
//! instance, so a copy is never silently left on its scalar value.
use super::*;

/// The root followed by every multipatch copy, in patch order.
pub(super) fn instance_destinations(fixture: &PatchedFixture) -> Box<[FixtureId]> {
    std::iter::once(fixture.fixture_id)
        .chain(fixture.multipatch.iter().map(|copy| FixtureId(copy.id)))
        .collect()
}

/// The owning control on every instance, root first; empty without one.
pub(super) fn instance_footprint(
    instances: &[FixtureId],
    control: Option<&OpticsFitControl>,
) -> Box<[NativeControlSlot]> {
    control
        .into_iter()
        .flat_map(|control| {
            instances.iter().map(|&destination| NativeControlSlot {
                destination,
                channel_index: control.channel_index,
                split: control.split,
            })
        })
        .collect()
}

/// The root's write repeated on every instance with only the destination changed.
pub(super) fn replicate(
    instances: &[FixtureId],
    root: NativeControlWrite,
) -> Vec<NativeControlWrite> {
    instances
        .iter()
        .map(|&destination| NativeControlWrite {
            slot: NativeControlSlot {
                destination,
                ..root.slot
            },
            ..root
        })
        .collect()
}
