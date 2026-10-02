//! Conservative heap accounting for bounded replay/gesture caches. Shared allocations are
//! counted per retained value so sharing can never conceal an oversized cache entry.
use super::*;
use crate::{AttributeValue, NativeColorIdentity};
use std::mem::size_of;

fn vector<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(size_of::<T>())
}
fn scalar(value: &ScalarIntent) -> usize {
    match value {
        ScalarIntent::Value(_) => 0,
        ScalarIntent::Spread(points) => vector(points),
    }
}
fn identity(value: &NativeColorIdentity) -> usize {
    value
        .profile_digest
        .capacity()
        .saturating_add(value.native_layout_signature.capacity())
}
fn shared<T>(_: &T) -> usize {
    size_of::<T>().saturating_add(2 * size_of::<usize>())
}
impl AttributeValue {
    pub fn retained_heap_bytes(&self) -> usize {
        match self {
            Self::Spread(values) => vector(values),
            Self::Discrete(value) => value.capacity(),
            Self::Position(value) => shared(value.as_ref()).saturating_add(match value.as_ref() {
                PositionIntent::Angles {
                    pan_degrees,
                    tilt_degrees,
                } => scalar(pan_degrees).saturating_add(scalar(tilt_degrees)),
                PositionIntent::Target { offset_metres, .. } => offset_metres
                    .iter()
                    .map(scalar)
                    .fold(0, usize::saturating_add),
            }),
            Self::Zoom(value) => {
                shared(value.as_ref()).saturating_add(scalar(&value.opening_degrees))
            }
            Self::ColorProgram(value) => {
                shared(value.as_ref()).saturating_add(match value.as_ref() {
                    ColorProgram::Semantic { intent } => vector(&intent.spreads)
                        .saturating_add(
                            intent
                                .spreads
                                .iter()
                                .map(|spread| vector(&spread.points))
                                .fold(0, usize::saturating_add),
                        )
                        .saturating_add(vector(&intent.wheel_constraints))
                        .saturating_add(
                            intent
                                .wheel_constraints
                                .iter()
                                .map(|wheel| identity(&wheel.source))
                                .fold(0, usize::saturating_add),
                        ),
                    ColorProgram::Direct { recipe, portable } => identity(&recipe.source)
                        .saturating_add(vector(&recipe.channels))
                        .saturating_add(vector(&recipe.spreads))
                        .saturating_add(
                            recipe
                                .spreads
                                .iter()
                                .map(|spread| vector(&spread.points))
                                .fold(0, usize::saturating_add),
                        )
                        .saturating_add(vector(&portable.limitations))
                        .saturating_add(
                            portable
                                .limitations
                                .iter()
                                .map(String::capacity)
                                .fold(0, usize::saturating_add),
                        ),
                })
            }
            Self::GroupFamily(value) => shared(value.as_ref())
                .saturating_add(value.template.retained_heap_bytes())
                .saturating_add(
                    value
                        .members
                        .len()
                        .saturating_mul(size_of::<uuid::Uuid>() + size_of::<AttributeValue>() + 64),
                )
                .saturating_add(
                    value
                        .members
                        .values()
                        .map(Self::retained_heap_bytes)
                        .fold(0, usize::saturating_add),
                ),
            _ => 0,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn group_and_shared_color_storage_cannot_hide_from_cache_limits() {
        let value = AttributeValue::ColorProgram(std::sync::Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                spreads: vec![ColorComponentSpread {
                    component: ColorComponent::Uv,
                    points: vec![0.0; 4096],
                }],
                ..Default::default()
            },
        }));
        assert!(value.retained_heap_bytes() >= 4096 * size_of::<f32>());
        let group = AttributeValue::GroupFamily(std::sync::Arc::new(GroupFamilyAssignment {
            owner: ProgrammingOwner::Color,
            template: value.clone(),
            members: (0..100)
                .map(|_| (uuid::Uuid::new_v4(), value.clone()))
                .collect(),
        }));
        assert!(group.retained_heap_bytes() > 101 * value.retained_heap_bytes());
    }
}
