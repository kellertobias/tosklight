//! Point poses compiled and sampled with the same generation as a rendered output frame.

use std::sync::Arc;

use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::PatchedFixture;

use crate::{FrameValues, Pooled, Reusable, Slot, SlotTable, ValuePool};

/// Shared offset encoding for the desk's 3D Point personality and tracking inputs.
pub const POINT_AXIS_METRES: f32 = 100.0;
const ROTATION_RANGE_DEGREES: f32 = 180.0;
const AXIS_NAMES: [&str; 6] = [
    "point.position.x",
    "point.position.y",
    "point.position.z",
    "point.rotation.x",
    "point.rotation.y",
    "point.rotation.z",
];

/// A Point's resolved pose in desk axes, measured in metres and degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPointPose {
    pub fixture_id: FixtureId,
    pub origin_metres: [f32; 3],
    pub offset_metres: [f32; 3],
    pub rotation_degrees: [f32; 3],
}

impl Reusable for Vec<ResolvedPointPose> {
    fn reset(&mut self) {
        self.clear();
    }
}

struct PointAxis {
    name: AttributeKey,
    owner: FixtureId,
    slot: Option<Slot>,
    frozen_owner: Option<AttributeValue>,
    frozen_root: Option<AttributeValue>,
}

struct PointEntry {
    fixture_id: FixtureId,
    origin_metres: [f32; 3],
    axes: [PointAxis; 6],
}

/// The small Point-only projection of a patch. It is shared across generations when fixtures are
/// unchanged; neither a frame nor a reader has to scan the full patch to find Points.
pub(crate) struct PointProjectionIndex {
    slot_generation: u64,
    entries: Vec<PointEntry>,
    pool: Arc<ValuePool<Vec<ResolvedPointPose>>>,
    #[cfg(test)]
    resolutions: std::sync::atomic::AtomicUsize,
}

impl PointProjectionIndex {
    pub(crate) fn compile(fixtures: &[PatchedFixture], slots: &SlotTable) -> Self {
        // Preserve apply_fixture_freezes order, including legacy or foreign target addresses.
        // Full/family metadata does not filter its stored values. Later fixtures win duplicates.
        let mut frozen = rustc_hash::FxHashMap::default();
        for fixture in fixtures {
            for (owner, target) in &fixture.freeze.targets {
                for (attribute, value) in &target.values {
                    if AXIS_NAMES.contains(&attribute.0.as_ref()) {
                        frozen.insert((*owner, attribute.clone()), value);
                    }
                }
            }
        }
        let mut entries: Vec<_> = fixtures
            .iter()
            .filter(|fixture| {
                fixture.definition.heads.iter().any(|head| {
                    head.parameters
                        .iter()
                        .any(|parameter| parameter.attribute.0.as_ref() == AXIS_NAMES[0])
                })
            })
            .map(|fixture| PointEntry {
                fixture_id: fixture.fixture_id,
                origin_metres: [
                    fixture.location.x as f32 / 1000.0,
                    fixture.location.y as f32 / 1000.0,
                    fixture.location.z as f32 / 1000.0,
                ],
                axes: std::array::from_fn(|index| {
                    let name = AttributeKey(AXIS_NAMES[index].into());
                    let owner = axis_owner(fixture, &name).unwrap_or(fixture.fixture_id);
                    let slot = slots.slot(owner, &name);
                    PointAxis {
                        frozen_owner: frozen.get(&(owner, name.clone())).copied().cloned(),
                        frozen_root: (owner != fixture.fixture_id)
                            .then(|| {
                                frozen
                                    .get(&(fixture.fixture_id, name.clone()))
                                    .copied()
                                    .cloned()
                            })
                            .flatten(),
                        name,
                        owner,
                        slot,
                    }
                }),
            })
            .collect();
        entries.sort_by_key(|entry| entry.fixture_id.0);
        Self {
            slot_generation: slots.generation(),
            entries,
            pool: Arc::new(ValuePool::default()),
            #[cfg(test)]
            resolutions: Default::default(),
        }
    }

    /// Tracking stores world positions; each immutable generation supplies its actual origin.
    pub(crate) fn origin_for(&self, fixture_id: FixtureId) -> Option<[f32; 3]> {
        let index = self
            .entries
            .binary_search_by_key(&fixture_id.0, |entry| entry.fixture_id.0)
            .ok()?;
        Some(self.entries[index].origin_metres)
    }

    /// Apply a tracked Point position through its compiled axis owners without allocating names.
    pub(crate) fn override_position(
        &self,
        fixture_id: FixtureId,
        normalized: [f32; 3],
        resolved: &mut crate::ResolvedAttributes,
    ) {
        let Ok(index) = self
            .entries
            .binary_search_by_key(&fixture_id.0, |entry| entry.fixture_id.0)
        else {
            return;
        };
        for (axis, value) in self.entries[index].axes[..3].iter().zip(normalized) {
            resolved.override_value(
                axis.owner,
                &axis.name,
                AttributeValue::Normalized(value),
                None,
            );
        }
    }

    /// Root-addressed tracking bindings target the Point personality's actual value owner.
    pub(crate) fn owner_for(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<FixtureId> {
        let index = self
            .entries
            .binary_search_by_key(&fixture_id.0, |entry| entry.fixture_id.0)
            .ok()?;
        self.entries[index]
            .axes
            .iter()
            .find(|axis| &axis.name == attribute)
            .map(|axis| axis.owner)
    }

    pub(crate) fn resolve(&self, values: &FrameValues) -> Pooled<Vec<ResolvedPointPose>> {
        let frame = values
            .frame()
            .filter(|frame| frame.slots().generation() == self.slot_generation);
        self.resolve_reading(
            |owner, name, slot| match frame {
                Some(frame) => slot
                    .and_then(|slot| frame.value(slot))
                    .or_else(|| slot.is_none().then(|| values.value(owner, name)).flatten()),
                None => values.value(owner, name),
            },
            false,
        )
    }

    pub(crate) fn resolve_saved(&self, values: &FrameValues) -> Pooled<Vec<ResolvedPointPose>> {
        self.resolve_reading(|owner, name, _| values.value(owner, name), true)
    }

    /// Read a prepared token without taking its dense storage or changing static Current.
    /// Compiled final Point freezes shadow values exactly as the final render's override does.
    pub(crate) fn resolve_pre_freeze(
        &self,
        frame: &crate::contribution::ResolvedFrame,
    ) -> Pooled<Vec<ResolvedPointPose>> {
        let same_generation = frame.slots().generation() == self.slot_generation;
        self.resolve_reading(
            |owner, name, slot| {
                if same_generation && let Some(slot) = slot {
                    frame.value(slot)
                } else {
                    frame.winner(owner, name).map(|winner| &winner.value)
                }
            },
            true,
        )
    }

    #[cfg(test)]
    pub(crate) fn resolutions(&self) -> usize {
        self.resolutions.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn resolve_reading<'a>(
        &'a self,
        mut read: impl FnMut(FixtureId, &AttributeKey, Option<Slot>) -> Option<&'a AttributeValue>,
        apply_freezes: bool,
    ) -> Pooled<Vec<ResolvedPointPose>> {
        #[cfg(test)]
        self.resolutions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut poses = self.pool.take();
        poses.reserve(self.entries.len());
        for entry in &self.entries {
            let axes = std::array::from_fn::<_, 6, _>(|index| {
                let axis = &entry.axes[index];
                let owned = apply_freezes
                    .then_some(axis.frozen_owner.as_ref())
                    .flatten()
                    .or_else(|| read(axis.owner, &axis.name, axis.slot));
                // Older root-addressed Programmer/Cue values may be unnumbered when a profile
                // moved its axes to a logical head. They still describe this root Point.
                let value = owned.or_else(|| {
                    (axis.owner != entry.fixture_id)
                        .then(|| {
                            apply_freezes
                                .then_some(axis.frozen_root.as_ref())
                                .flatten()
                                .or_else(|| read(entry.fixture_id, &axis.name, None))
                        })
                        .flatten()
                });
                value.and_then(AttributeValue::normalized)
            });
            let convert = |value: Option<f32>, extent: f32| {
                value.map_or(0.0, |value| -extent + value * (2.0 * extent))
            };
            poses.push(ResolvedPointPose {
                fixture_id: entry.fixture_id,
                origin_metres: entry.origin_metres,
                offset_metres: std::array::from_fn(|index| convert(axes[index], POINT_AXIS_METRES)),
                rotation_degrees: std::array::from_fn(|index| {
                    convert(axes[index + 3], ROTATION_RANGE_DEGREES)
                }),
            });
        }
        poses
    }
}

fn axis_owner(fixture: &PatchedFixture, attribute: &AttributeKey) -> Option<FixtureId> {
    let mode = crate::fixture::profile_mode(fixture)?;
    mode.heads
        .iter()
        .enumerate()
        .find_map(|(head_index, head)| {
            mode.channels
                .iter()
                .any(|channel| {
                    channel.head_id == head.id
                        && (channel.attribute == *attribute
                            || channel.fixture_attribute == *attribute
                            || channel
                                .functions
                                .iter()
                                .any(|function| function.attribute == *attribute))
                })
                .then(|| crate::profile_head_owner(fixture, head_index, head))
        })
}
