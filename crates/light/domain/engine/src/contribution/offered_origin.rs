//! A traced winner's origin, built only when its offer wins (TL-639 round 7).
//!
//! Tracing resolvers record the origin of every winning contribution. Built eagerly, each
//! offer allocated one origin per frame, and the slot freed last frame's: on a desk whose
//! sources hold still that is one allocation and one free per traced slot and frame for the
//! same answer. An offer now carries what its origin is made of; a winning one keeps the
//! origin its slot already holds when that describes the same source, edit stamp and
//! transition, and builds one otherwise. Origins are compared by content only, never by
//! allocation, so a kept origin is the origin a fresh build would have made.
use crate::contribution_batch::{ContributionOrigin, ContributionSourceId};
use light_core::TimedValue;
use std::borrow::Cow;
use std::sync::Arc;

pub(crate) enum OfferedOrigin<'v> {
    None,
    /// An origin the contribution already carries (kept winners offer the same one every
    /// frame, so a slot already holding it keeps it without touching its count).
    Shared(&'v Arc<ContributionOrigin>),
    /// Built (or kept) only when the offer wins.
    Built {
        source: Cow<'v, ContributionSourceId>,
        value: &'v TimedValue,
        transition_ordinal: Option<u64>,
    },
}

impl OfferedOrigin<'_> {
    /// The origin a winner holds, given the one its slot held before (`previous`).
    pub(crate) fn resolve(
        self,
        previous: Option<Arc<ContributionOrigin>>,
    ) -> Option<Arc<ContributionOrigin>> {
        match self {
            Self::None => None,
            Self::Shared(origin) => match previous {
                Some(previous) if Arc::ptr_eq(&previous, origin) => Some(previous),
                _ => Some(Arc::clone(origin)),
            },
            Self::Built {
                source,
                value,
                transition_ordinal,
            } => match previous {
                Some(previous) if previous.describes(&source, value, transition_ordinal) => {
                    Some(previous)
                }
                _ => Some(ContributionOrigin::with_transition_ordinal(
                    source.into_owned(),
                    value,
                    transition_ordinal,
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode};

    fn value(changed_at: i64, programmer_order: u64) -> TimedValue {
        TimedValue {
            fixture_id: FixtureId(uuid::Uuid::from_u128(2)),
            attribute: AttributeKey("pan".into()),
            value: AttributeValue::Normalized(0.5),
            priority: 0,
            changed_at: Utc.timestamp_millis_opt(changed_at).unwrap(),
            programmer_order,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }
    }

    fn built<'v>(
        source: &'v ContributionSourceId,
        value: &'v TimedValue,
        ordinal: Option<u64>,
    ) -> OfferedOrigin<'v> {
        OfferedOrigin::Built {
            source: Cow::Borrowed(source),
            value,
            transition_ordinal: ordinal,
        }
    }

    #[test]
    fn a_held_origin_is_kept_only_while_it_describes_the_offer() {
        let programmer = light_core::ProgrammerId(uuid::Uuid::from_u128(1));
        let source = ContributionSourceId::programmer_transient(programmer, "a");
        let other = ContributionSourceId::programmer_transient(programmer, "b");
        let held = value(10, 3);
        let previous = built(&source, &held, Some(4)).resolve(None).unwrap();
        // The same source, stamp and transition: the slot's allocation stays.
        let kept = built(&source, &value(10, 3), Some(4))
            .resolve(Some(previous.clone()))
            .unwrap();
        assert!(Arc::ptr_eq(&kept, &previous));
        let mut other_fixture = held.clone();
        other_fixture.fixture_id = FixtureId::new();
        let moved = built(&source, &other_fixture, Some(4))
            .resolve(Some(previous.clone()))
            .unwrap();
        assert!(
            !Arc::ptr_eq(&moved, &previous),
            "an origin cannot describe another authored fixture"
        );
        assert_eq!(moved.authored_fixture_id(), other_fixture.fixture_id);

        // Any difference builds the offer's own origin, equal to an eager build.
        for (source, changed, order, ordinal) in [
            (&other, 10, 3, Some(4)),
            (&source, 11, 3, Some(4)),
            (&source, 10, 4, Some(4)),
            (&source, 10, 3, None),
        ] {
            let offered = value(changed, order);
            let fresh = built(source, &offered, ordinal)
                .resolve(Some(previous.clone()))
                .unwrap();
            assert!(!Arc::ptr_eq(&fresh, &previous));
            let eager =
                ContributionOrigin::with_transition_ordinal(source.clone(), &offered, ordinal);
            assert!(fresh.describes(eager.source(), &offered, eager.transition_ordinal()));
            assert_eq!(fresh.stamp().changed_at, eager.stamp().changed_at);
            assert_eq!(
                fresh.stamp().programmer_order,
                eager.stamp().programmer_order
            );
        }
        assert!(OfferedOrigin::None.resolve(Some(previous)).is_none());
    }
}
