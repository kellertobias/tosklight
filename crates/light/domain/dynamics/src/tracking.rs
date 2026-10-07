use crate::{DynamicAddressValue, DynamicSemanticValue};
use light_core::{
    AttributeKey, FixtureId, ProgrammerId,
    programming::{IntentError, ProgrammingComponent},
};
use uuid::Uuid;

mod merge;

/// Stable runtime controller for one authored Programmer Dynamic link. Live and Preload edits
/// share this logical motion/clock; their separate source lanes belong to the captured rows.
/// Session IDs are excluded because every connected surface controls the desk's one Programmer.
pub fn programmer_dynamic_controller_id(programmer_id: ProgrammerId, authored_link: Uuid) -> Uuid {
    const NAMESPACE: Uuid = Uuid::from_u128(0x5052_4f47_5f44_594e_414d_4943_5f56_3100);
    let mut key = [0_u8; 32];
    key[..16].copy_from_slice(programmer_id.0.as_bytes());
    key[16..].copy_from_slice(authored_link.as_bytes());
    Uuid::new_v5(&NAMESPACE, &key)
}

/// Resolve authored edits within one Programmer's logical Dynamic scope. The caller partitions
/// rows by Programmer ID; Live and Preload rows for that Programmer intentionally share this
/// relation. An Off sweeps every target/owner of its link, while independent On lanes survive.
/// A superseded intermediate edit still removes any earlier rows it had itself superseded.
pub fn merge_dynamic_address_values<'a>(
    rows: impl IntoIterator<Item = &'a DynamicAddressValue>,
) -> Vec<&'a DynamicAddressValue> {
    // A newer On can hide an Off without undoing that Off's cutoff for other lanes. Keep
    // every original edit as a precedence barrier until all rows have been considered.
    // Input storage order is not chronological (normal and Preload are separate vectors).
    merge::fold(rows.into_iter().collect())
}

/// The existing Programmer edit order relation, including legacy zero-order timestamp fallback.
pub fn dynamic_address_edit_is_later(new: &DynamicAddressValue, old: &DynamicAddressValue) -> bool {
    if new.programmer_order > 0 && old.programmer_order > 0 {
        new.programmer_order > old.programmer_order
    } else {
        (new.changed_at_millis, new.programmer_order)
            > (old.changed_at_millis, old.programmer_order)
    }
}

fn dynamic_conflicts(a: &DynamicAddressValue, b: &DynamicAddressValue) -> bool {
    a.value.replaces_address(
        a.fixture_id,
        &a.attribute,
        b.value.track_key(),
        b.fixture_id,
        &b.attribute,
    ) || b.value.replaces_address(
        b.fixture_id,
        &b.attribute,
        a.value.track_key(),
        a.fixture_id,
        &a.attribute,
    )
}

/// Identity within one fixture/owner. Several component lanes can share an owner.
/// An instance-wide Off has no lane; ordinary Static/FixAt/Release has no instance.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DynamicTrackKey {
    pub instance_link: Option<Uuid>,
    pub lane_id: Option<Uuid>,
    pub component: Option<ProgrammingComponent>,
}

impl DynamicTrackKey {
    /// Off belongs to the complete instance, across its fixtures and owners.
    /// On rows retain exact fixture/owner/lane identity within that instance.
    pub fn replaces_address(
        self,
        fixture: FixtureId,
        owner: &AttributeKey,
        stored: Self,
        stored_fixture: FixtureId,
        stored_owner: &AttributeKey,
    ) -> bool {
        if self.instance_link.is_some()
            && self.instance_link == stored.instance_link
            && (self.lane_id.is_none() || stored.lane_id.is_none())
        {
            return true;
        }
        fixture == stored_fixture && owner == stored_owner && self == stored
    }
}

impl DynamicSemanticValue {
    pub fn track_key(&self) -> DynamicTrackKey {
        let (instance_link, lane_id) = match self {
            Self::DynamicOn {
                instance_link,
                lane_id,
                ..
            } => (Some(*instance_link), Some(*lane_id)),
            Self::DynamicOff { instance_link, .. } => (Some(*instance_link), None),
            Self::Static { .. }
            | Self::FixAt { .. }
            | Self::ProgrammingFixAt { .. }
            | Self::ProgrammingRelease { .. }
            | Self::Release => (None, None),
        };
        DynamicTrackKey {
            instance_link,
            lane_id,
            component: match self {
                Self::ProgrammingFixAt { mask, .. } => mask.address.component,
                Self::ProgrammingRelease { component } => *component,
                _ => None,
            },
        }
    }

    /// Whole-owner Release sweeps component holds, but ordinary whole values do not. Keeping
    /// those rules separate lets a component hold reappear after a temporary whole takeover.
    pub fn replaces_address(
        &self,
        fixture: FixtureId,
        owner: &AttributeKey,
        stored: DynamicTrackKey,
        stored_fixture: FixtureId,
        stored_owner: &AttributeKey,
    ) -> bool {
        if matches!(self, Self::Release) {
            return fixture == stored_fixture
                && owner == stored_owner
                && stored.instance_link.is_none();
        }
        self.track_key()
            .replaces_address(fixture, owner, stored, stored_fixture, stored_owner)
    }

    pub fn is_programming_release(&self) -> bool {
        matches!(self, Self::Release | Self::ProgrammingRelease { .. })
    }

    pub fn validate_programming_at(&self, owner: &AttributeKey) -> Result<(), IntentError> {
        match self {
            Self::ProgrammingFixAt { mask, .. } => {
                mask.validate()?;
                if mask.address.owner().key() != *owner {
                    return Err(IntentError(
                        "FixAT mask belongs to a different owner".into(),
                    ));
                }
            }
            Self::ProgrammingRelease { component } => {
                if component.is_none() {
                    use light_core::programming::ProgrammingOwner;
                    if ![
                        ProgrammingOwner::Color,
                        ProgrammingOwner::Position,
                        ProgrammingOwner::Focus,
                        ProgrammingOwner::Zoom,
                    ]
                    .iter()
                    .any(|candidate| candidate.key() == *owner)
                    {
                        return Err(IntentError(
                            "typed family Release has an invalid owner".into(),
                        ));
                    }
                    return Ok(());
                }
                let component = component.expect("checked component");
                let continuous = match component {
                    ProgrammingComponent::NativeColor(binding) => {
                        !binding.channel_id.is_nil() && !binding.function_id.is_nil()
                    }
                    _ => component.descriptor().dynamics,
                };
                if component.owner().key() != *owner || !continuous {
                    return Err(IntentError(
                        "component Release has an invalid owner or component".into(),
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivationBoundary, ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot,
        DynamicInstanceOverrides, DynamicPhaseSpreadMode, DynamicReference, DynamicRunMode,
        DynamicSpeed, DynamicTargetBinding, DynamicValueTiming, PhaseDistribution, PhaseOrdering,
        Rational,
    };
    use std::sync::Arc;

    pub(super) fn reference() -> DynamicReference {
        let definition = DynamicDefinition {
            id: Uuid::from_u128(0xd1),
            pool_number: 1,
            revision: 1,
            name: "tracking".into(),
            color: None,
            icon: None,
            target_binding: DynamicTargetBinding::Targetless,
            lanes: vec![],
            random_groups: vec![],
            phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
            spatial_mapping: Default::default(),
            phase: PhaseDistribution {
                ordering: PhaseOrdering::Selection,
                offset_degrees: 0.0,
                span_degrees: 360.0,
                block_size: 1,
                repeats: 1,
                wings: false,
                anchors_degrees: vec![],
            },
            speed: DynamicSpeed::Fixed {
                duration_millis: 1_000,
            },
            overall_speed_multiplier: Rational::ONE,
            run_mode: DynamicRunMode::Loop,
            default_activation: ActivationPolicy::StartNow,
            activation_boundary: ActivationBoundary::Beat,
        };
        DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: 1,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(definition),
            },
        }
    }

    pub(super) fn on(
        fixture_id: FixtureId,
        instance_link: Uuid,
        lane_id: Uuid,
        order: u64,
        reference: &DynamicReference,
    ) -> DynamicAddressValue {
        DynamicAddressValue {
            fixture_id,
            attribute: AttributeKey("position".into()),
            value: DynamicSemanticValue::DynamicOn {
                instance_link,
                dynamic: reference.clone(),
                lane_id,
                overrides: DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: DynamicValueTiming::default(),
            },
            programmer_order: order,
            changed_at_millis: order,
        }
    }

    pub(super) fn off(
        fixture_id: FixtureId,
        instance_link: Uuid,
        order: u64,
    ) -> DynamicAddressValue {
        DynamicAddressValue {
            fixture_id,
            attribute: AttributeKey("color".into()),
            value: DynamicSemanticValue::DynamicOff {
                instance_link,
                timing: DynamicValueTiming::default(),
            },
            programmer_order: order,
            changed_at_millis: order,
        }
    }

    #[test]
    fn programmer_controller_id_scopes_authored_link_to_programmer_not_source_lane() {
        let first = ProgrammerId(Uuid::from_u128(1));
        let second = ProgrammerId(Uuid::from_u128(2));
        let link = Uuid::from_u128(3);
        let id = programmer_dynamic_controller_id(first, link);
        assert_eq!(id, programmer_dynamic_controller_id(first, link));
        assert_ne!(id, programmer_dynamic_controller_id(second, link));
        assert_ne!(
            id,
            programmer_dynamic_controller_id(first, Uuid::from_u128(4))
        );
        assert_ne!(id, link);
    }

    #[test]
    fn borrowed_fold_preserves_mixed_targets_and_siblings_until_global_off() {
        let reference = reference();
        let link = Uuid::from_u128(10);
        let other_link = Uuid::from_u128(11);
        let first = FixtureId::new();
        let second = FixtureId::new();
        let pan = Uuid::from_u128(1);
        let tilt = Uuid::from_u128(2);
        let rows = [
            on(first, link, pan, 1, &reference),
            on(first, link, tilt, 2, &reference),
            on(second, link, pan, 3, &reference),
            on(first, other_link, pan, 4, &reference),
        ];
        let all = merge_dynamic_address_values(rows.iter());
        assert_eq!(all.len(), 4);
        let global_off = off(second, link, 5);
        let with_off = merge_dynamic_address_values(rows.iter().chain([&global_off]));
        assert_eq!(with_off, vec![&rows[3], &global_off]);
        let restarted = on(first, link, tilt, 6, &reference);
        let after_restart =
            merge_dynamic_address_values(rows.iter().chain([&global_off, &restarted]));
        assert_eq!(after_restart, vec![&rows[3], &restarted]);
    }

    #[test]
    fn superseded_intermediate_off_still_clears_earlier_target() {
        let reference = reference();
        let link = Uuid::from_u128(12);
        let first = FixtureId::new();
        let second = FixtureId::new();
        let earlier = on(first, link, Uuid::from_u128(1), 1, &reference);
        let newer_sibling = on(second, link, Uuid::from_u128(2), 5, &reference);
        let middle_off = off(first, link, 3);
        let folded = merge_dynamic_address_values([&earlier, &newer_sibling, &middle_off]);
        assert_eq!(folded, vec![&newer_sibling]);
        assert!(dynamic_address_edit_is_later(&newer_sibling, &middle_off));
    }

    #[test]
    fn off_cutoff_survives_newer_on_in_every_storage_order() {
        let reference = reference();
        let link = Uuid::from_u128(12);
        let earlier = on(FixtureId::new(), link, Uuid::from_u128(1), 10, &reference);
        let middle_off = off(earlier.fixture_id, link, 20);
        let newer_sibling = on(FixtureId::new(), link, Uuid::from_u128(2), 30, &reference);
        let rows = [&earlier, &middle_off, &newer_sibling];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            assert_eq!(
                merge_dynamic_address_values(order.map(|index| rows[index])),
                vec![&newer_sibling],
                "a new lane must not resurrect another lane cut off by Off: {order:?}"
            );
        }
    }

    #[test]
    fn equal_stamp_duplicates_keep_the_first_row_once() {
        let first = on(
            FixtureId::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            1,
            &reference(),
        );
        let duplicate = first.clone();
        let folded = merge_dynamic_address_values([&first, &duplicate]);
        assert_eq!(folded.len(), 1);
        assert!(std::ptr::eq(folded[0], &first));
    }
}
