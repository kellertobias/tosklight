use super::*;
use std::sync::Arc;

const NORMAL: VisualizationLane = VisualizationLane::Normal;
const PRELOAD: VisualizationLane = VisualizationLane::Preload;

/// Test sources: equal values stand for one accepted source delivered again.
impl LeasedSource for Arc<u64> {
    fn absorb(&mut self, delivered: &Self) -> bool {
        **self == **delivered
    }
}

fn table() -> DisplayedSourceLeases<Arc<u64>> {
    DisplayedSourceLeases::default()
}

#[test]
fn the_ring_keeps_only_the_newest_deliveries_per_session_lane() {
    let leases = table();
    let session = SessionId::new();
    let now = Instant::now();
    let issued = (0..20)
        .map(|frame| leases.issue(session, NORMAL, Arc::new(frame), now))
        .collect::<Vec<_>>();
    assert_eq!(leases.retained(session, NORMAL), LEASE_RING);
    for (frame, lease) in issued.iter().enumerate() {
        let resolved = leases.resolve(session, NORMAL, *lease, now);
        if frame < 20 - LEASE_RING {
            assert_eq!(resolved, None, "frame {frame} left the ring");
        } else {
            assert_eq!(resolved.as_deref(), Some(&(frame as u64)));
        }
    }
}

#[test]
fn the_ring_bound_holds_over_ten_thousand_deliveries_and_releases_old_sources() {
    let leases = table();
    let session = SessionId::new();
    let now = Instant::now();
    let first = Arc::new(0_u64);
    leases.issue(session, NORMAL, Arc::clone(&first), now);
    for frame in 1..10_000 {
        leases.issue(session, NORMAL, Arc::new(frame), now);
    }
    assert_eq!(leases.retained(session, NORMAL), LEASE_RING);
    assert_eq!(leases.retained(session, PRELOAD), 0);
    assert_eq!(
        Arc::strong_count(&first),
        1,
        "an evicted source is not retained"
    );
}

#[test]
fn entries_expire_after_the_age_cap_since_delivery() {
    let leases = table();
    let session = SessionId::new();
    let start = Instant::now();
    let lease = leases.issue(session, NORMAL, Arc::new(1), start);
    assert!(
        leases
            .resolve(session, NORMAL, lease, start + LEASE_MAX_AGE)
            .is_some()
    );
    assert_eq!(
        leases.resolve(
            session,
            NORMAL,
            lease,
            start + LEASE_MAX_AGE + Duration::from_millis(1)
        ),
        None
    );
    assert_eq!(leases.retained(session, NORMAL), 0);
}

#[test]
fn leases_are_opaque_session_and_lane_scoped() {
    let leases = table();
    let (owner, other) = (SessionId::new(), SessionId::new());
    let now = Instant::now();
    let lease = leases.issue(owner, NORMAL, Arc::new(7), now);
    let other_lease = leases.issue(other, NORMAL, Arc::new(8), now);
    assert_ne!(
        lease, other_lease,
        "leases are never reused across sessions"
    );
    assert_eq!(leases.resolve(other, NORMAL, lease, now), None);
    assert_eq!(leases.resolve(owner, PRELOAD, lease, now), None);
    assert_eq!(leases.resolve(owner, NORMAL, lease + 1000, now), None);
    assert_eq!(
        leases.resolve(owner, NORMAL, lease, now).as_deref(),
        Some(&7)
    );
}

#[test]
fn a_pinned_gesture_source_survives_ring_turnover_and_age_until_released() {
    let leases = table();
    let session = SessionId::new();
    let start = Instant::now();
    let lease = leases.issue(session, NORMAL, Arc::new(42), start);
    assert_eq!(
        leases
            .pin(session, NORMAL, lease, "gesture-a", start)
            .as_deref(),
        Some(&42)
    );
    let later = start + LEASE_MAX_AGE * 4;
    for frame in 0..(LEASE_RING as u64 * 3) {
        leases.issue(session, NORMAL, Arc::new(frame), later);
    }
    assert_eq!(leases.retained(session, NORMAL), LEASE_RING + 1);
    assert_eq!(
        leases.resolve(session, NORMAL, lease, later).as_deref(),
        Some(&42)
    );
    leases.unpin(session, NORMAL, "another-gesture");
    assert!(leases.resolve(session, NORMAL, lease, later).is_some());
    leases.unpin(session, NORMAL, "gesture-a");
    assert_eq!(leases.resolve(session, NORMAL, lease, later), None);
}

#[test]
fn a_pin_expires_when_idle_and_a_new_pin_replaces_the_old_one() {
    let leases = table();
    let session = SessionId::new();
    let start = Instant::now();
    let first = leases.issue(session, NORMAL, Arc::new(1), start);
    let second = leases.issue(session, NORMAL, Arc::new(2), start);
    leases.pin(session, NORMAL, first, "a", start).unwrap();
    leases.pin(session, NORMAL, second, "b", start).unwrap();
    let after_ring = start + LEASE_MAX_AGE * 2;
    assert_eq!(leases.resolve(session, NORMAL, first, after_ring), None);
    assert!(
        leases
            .resolve(session, NORMAL, second, after_ring)
            .is_some()
    );
    let idle = after_ring + PIN_MAX_IDLE + Duration::from_millis(1);
    assert_eq!(leases.resolve(session, NORMAL, second, idle), None);
    assert_eq!(leases.retained(session, NORMAL), 0);
    assert_eq!(
        leases.pin(session, NORMAL, 9_999, "c", idle),
        None,
        "an unknown lease pins nothing"
    );
}

#[test]
fn lane_eviction_and_session_close_release_everything() {
    let leases = table();
    let session = SessionId::new();
    let now = Instant::now();
    let live = leases.issue(session, NORMAL, Arc::new(1), now);
    let pending = leases.issue(session, PRELOAD, Arc::new(2), now);
    leases.pin(session, NORMAL, live, "g", now).unwrap();
    leases.evict_lane(session, NORMAL);
    assert_eq!(leases.resolve(session, NORMAL, live, now), None);
    assert!(leases.resolve(session, PRELOAD, pending, now).is_some());
    leases.close_session(session);
    assert_eq!(leases.resolve(session, PRELOAD, pending, now), None);
    assert_eq!(leases.sessions(), 0);
}

#[test]
fn the_table_is_shareable_across_request_and_socket_tasks() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DisplayedSourceLeases<Arc<u64>>>();
}

#[test]
fn a_redelivered_source_keeps_its_lease_and_ring_slot_and_renews_its_age() {
    let leases = table();
    let (desk, other) = (SessionId::new(), SessionId::new());
    let start = Instant::now();
    let shown = leases.issue(desk, NORMAL, Arc::new(1), start);
    let half = start + LEASE_MAX_AGE / 2;
    for _ in 0..(LEASE_RING * 4) {
        assert_eq!(
            leases.issue(desk, NORMAL, Arc::new(1), half),
            shown,
            "a second reader of the shown source shares its lease"
        );
    }
    assert_eq!(leases.retained(desk, NORMAL), 1, "one source, one slot");
    assert_ne!(
        leases.issue(other, NORMAL, Arc::new(1), half),
        shown,
        "another session never receives this session's lease"
    );
    assert_ne!(leases.issue(desk, PRELOAD, Arc::new(1), half), shown);
    let renewed = start + LEASE_MAX_AGE + Duration::from_millis(1);
    assert_eq!(
        leases.resolve(desk, NORMAL, shown, renewed).as_deref(),
        Some(&1),
        "the redelivery renewed the age"
    );
    for frame in 2..=(LEASE_RING as u64) {
        leases.issue(desk, NORMAL, Arc::new(frame), renewed);
    }
    assert!(
        leases.resolve(desk, NORMAL, shown, renewed).is_some(),
        "newer sources by other readers leave it in the ring until it is full"
    );
    leases.issue(desk, NORMAL, Arc::new(1_000), renewed);
    assert_eq!(leases.resolve(desk, NORMAL, shown, renewed), None);
    assert_eq!(leases.retained(desk, NORMAL), LEASE_RING);
    let expired = renewed + LEASE_MAX_AGE * 2;
    let again = leases.issue(desk, NORMAL, Arc::new(1_000), expired);
    assert_eq!(
        leases.retained(desk, NORMAL),
        1,
        "expired sources are released"
    );
    assert_eq!(
        leases.resolve(desk, NORMAL, again, expired).as_deref(),
        Some(&1_000)
    );
}

#[test]
fn a_redelivery_never_moves_or_replaces_the_pinned_gesture_source() {
    let leases = table();
    let session = SessionId::new();
    let now = Instant::now();
    let pinned = leases.issue(session, NORMAL, Arc::new(5), now);
    leases.pin(session, NORMAL, pinned, "gesture", now).unwrap();
    let other = leases.issue(session, NORMAL, Arc::new(6), now);
    assert_eq!(leases.issue(session, NORMAL, Arc::new(5), now), pinned);
    assert_eq!(leases.retained(session, NORMAL), 3);
    leases.unpin(session, NORMAL, "gesture");
    assert_eq!(
        leases.resolve(session, NORMAL, pinned, now).as_deref(),
        Some(&5),
        "unpinning leaves the delivered ring entry"
    );
    assert!(leases.resolve(session, NORMAL, other, now).is_some());
}
