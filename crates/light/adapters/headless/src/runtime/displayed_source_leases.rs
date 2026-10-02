//! TL-594 C1: bounded, per-session retention of the exact accepted sources a surface was SENT.
//!
//! - A lease is issued only for a delivery (an HTTP readout or a WebSocket lane message) and is
//!   opaque: one process-wide monotonic counter, never a hub sequence. A session can therefore
//!   resolve only a lease it was given; another session's lease resolves to nothing.
//! - Per session and lane, a ring keeps the newest [`LEASE_RING`] delivered sources, each for at
//!   most [`LEASE_MAX_AGE`] since it was last delivered. Older or expired entries are dropped
//!   (and their `Arc`s released).
//! - A source is leased once per session and lane: delivering the same accepted source again
//!   (another reader, the WebSocket lane, a probe) returns the lease it already has and refreshes
//!   its age instead of taking a new ring slot. A second reader therefore never pushes the
//!   desk's lease out of the ring or lets it age out while that source is still being shown.
//! - One pinned slot per session and lane holds the source adopted by the active gesture, so a
//!   long drag never loses it to ring turnover. A new pin replaces it; it is released on
//!   FinishGesture, after [`PIN_MAX_IDLE`] without use, on lane eviction and on session close.
//!
//! Nothing here runs on the output scheduler: issue/resolve happen in request and WebSocket
//! tasks. The table never allocates per lookup and has no unbounded map per session.

use light_core::SessionId;
use light_wire::v2::visualization::VisualizationLane;
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

/// Deliveries retained per session and lane (about 0.8 s at the 10 Hz stream cap).
pub(in crate::runtime) const LEASE_RING: usize = 8;
/// Age since delivery after which a ring entry can no longer be resolved.
pub(in crate::runtime) const LEASE_MAX_AGE: Duration = Duration::from_secs(2);
/// A pinned gesture source is released after this long without being used.
pub(in crate::runtime) const PIN_MAX_IDLE: Duration = Duration::from_secs(30);

struct Delivered<S> {
    lease: u64,
    delivered: Instant,
    source: S,
}

struct Pinned<S> {
    gesture: String,
    lease: u64,
    touched: Instant,
    source: S,
}

struct LaneLeases<S> {
    ring: VecDeque<Delivered<S>>,
    pinned: Option<Pinned<S>>,
}

impl<S> Default for LaneLeases<S> {
    fn default() -> Self {
        Self {
            ring: VecDeque::with_capacity(LEASE_RING),
            pinned: None,
        }
    }
}

struct SessionLeases<S> {
    normal: LaneLeases<S>,
    preload: LaneLeases<S>,
}

impl<S> Default for SessionLeases<S> {
    fn default() -> Self {
        Self {
            normal: LaneLeases::default(),
            preload: LaneLeases::default(),
        }
    }
}

impl<S> SessionLeases<S> {
    fn lane(&mut self, lane: VisualizationLane) -> &mut LaneLeases<S> {
        match lane {
            VisualizationLane::Normal => &mut self.normal,
            VisualizationLane::Preload => &mut self.preload,
        }
    }
}

/// A retained source the table recognises when the same accepted source is delivered again.
pub(in crate::runtime) trait LeasedSource: Clone {
    /// Whether `delivered` is another delivery of this very accepted source. When it is, fold
    /// whatever it adds (for example owners another reader asked for) into `self`.
    fn absorb(&mut self, delivered: &Self) -> bool;
}

/// Generic over the retained source so Live frames and Pending captures share one policy.
pub(in crate::runtime) struct DisplayedSourceLeases<S> {
    next: AtomicU64,
    ring: usize,
    max_age: Duration,
    pin_idle: Duration,
    sessions: parking_lot::Mutex<HashMap<SessionId, SessionLeases<S>>>,
}

impl<S: LeasedSource> Default for DisplayedSourceLeases<S> {
    fn default() -> Self {
        Self::with_bounds(LEASE_RING, LEASE_MAX_AGE, PIN_MAX_IDLE)
    }
}

impl<S: LeasedSource> DisplayedSourceLeases<S> {
    pub(in crate::runtime) fn with_bounds(
        ring: usize,
        max_age: Duration,
        pin_idle: Duration,
    ) -> Self {
        Self {
            next: AtomicU64::new(0),
            ring: ring.max(1),
            max_age,
            pin_idle,
            sessions: parking_lot::Mutex::default(),
        }
    }

    /// Record one delivery of `source` to `session` on `lane` and return its opaque lease. A
    /// redelivery of a retained source keeps that source's lease and becomes its newest delivery.
    pub(in crate::runtime) fn issue(
        &self,
        session: SessionId,
        lane: VisualizationLane,
        source: S,
        now: Instant,
    ) -> u64 {
        let mut sessions = self.sessions.lock();
        let leases = sessions.entry(session).or_default().lane(lane);
        self.expire(leases, now);
        if let Some(index) = leases
            .ring
            .iter_mut()
            .position(|entry| entry.source.absorb(&source))
            && let Some(mut entry) = leases.ring.remove(index)
        {
            entry.delivered = now;
            let lease = entry.lease;
            leases.ring.push_back(entry);
            return lease;
        }
        let lease = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        while leases.ring.len() >= self.ring {
            leases.ring.pop_front();
        }
        leases.ring.push_back(Delivered {
            lease,
            delivered: now,
            source,
        });
        lease
    }

    /// The source of a lease this session was given on this lane, while it is still retained.
    /// Unknown, foreign-session, other-lane, evicted and expired leases resolve to `None`.
    pub(in crate::runtime) fn resolve(
        &self,
        session: SessionId,
        lane: VisualizationLane,
        lease: u64,
        now: Instant,
    ) -> Option<S> {
        let mut sessions = self.sessions.lock();
        let leases = sessions.get_mut(&session)?.lane(lane);
        self.expire(leases, now);
        if let Some(pinned) = leases.pinned.as_mut().filter(|pin| pin.lease == lease) {
            pinned.touched = now;
            return Some(pinned.source.clone());
        }
        leases
            .ring
            .iter()
            .find(|entry| entry.lease == lease)
            .map(|entry| entry.source.clone())
    }

    /// Resolve `lease` and pin it as the source of `gesture`, replacing any earlier pin of this
    /// session and lane. Returns `None` (and pins nothing) when the lease is not resolvable.
    pub(in crate::runtime) fn pin(
        &self,
        session: SessionId,
        lane: VisualizationLane,
        lease: u64,
        gesture: &str,
        now: Instant,
    ) -> Option<S> {
        let mut sessions = self.sessions.lock();
        let leases = sessions.get_mut(&session)?.lane(lane);
        self.expire(leases, now);
        let source = match leases.pinned.as_ref().filter(|pin| pin.lease == lease) {
            Some(pinned) => pinned.source.clone(),
            None => leases
                .ring
                .iter()
                .find(|entry| entry.lease == lease)?
                .source
                .clone(),
        };
        leases.pinned = Some(Pinned {
            gesture: gesture.to_owned(),
            lease,
            touched: now,
            source: source.clone(),
        });
        Some(source)
    }

    /// Release the pin of `gesture` (FinishGesture). Another gesture's pin is kept.
    pub(in crate::runtime) fn unpin(
        &self,
        session: SessionId,
        lane: VisualizationLane,
        gesture: &str,
    ) {
        let mut sessions = self.sessions.lock();
        if let Some(leases) = sessions.get_mut(&session).map(|leases| leases.lane(lane))
            && leases
                .pinned
                .as_ref()
                .is_some_and(|pin| pin.gesture == gesture)
        {
            leases.pinned = None;
        }
    }

    /// Drop every lease of one session lane (show/scope change, resubscription).
    pub(in crate::runtime) fn evict_lane(&self, session: SessionId, lane: VisualizationLane) {
        if let Some(leases) = self.sessions.lock().get_mut(&session) {
            *leases.lane(lane) = LaneLeases::default();
        }
    }

    /// Session teardown releases every retained source of the session.
    pub(in crate::runtime) fn close_session(&self, session: SessionId) {
        self.sessions.lock().remove(&session);
    }

    fn expire(&self, leases: &mut LaneLeases<S>, now: Instant) {
        while leases
            .ring
            .front()
            .is_some_and(|entry| now.saturating_duration_since(entry.delivered) > self.max_age)
        {
            leases.ring.pop_front();
        }
        if leases
            .pinned
            .as_ref()
            .is_some_and(|pin| now.saturating_duration_since(pin.touched) > self.pin_idle)
        {
            leases.pinned = None;
        }
    }

    /// Retained entries (ring + pinned) of one session lane. Test and diagnostics only.
    #[cfg(test)]
    pub(in crate::runtime) fn retained(
        &self,
        session: SessionId,
        lane: VisualizationLane,
    ) -> usize {
        self.sessions.lock().get_mut(&session).map_or(0, |leases| {
            let lane = leases.lane(lane);
            lane.ring.len() + usize::from(lane.pinned.is_some())
        })
    }

    #[cfg(test)]
    pub(in crate::runtime) fn sessions(&self) -> usize {
        self.sessions.lock().len()
    }
}

#[cfg(test)]
#[path = "displayed_source_leases/tests.rs"]
mod tests;
