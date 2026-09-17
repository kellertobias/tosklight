//! What the network output has done and heard: per-route send activity, and the Art-Net and sACN
//! peers the desk hears on its listeners.
//!
//! The DMX screen's Nodes tab reads this to say, per endpoint, whether light is leaving the desk
//! and whether anything else on the network is talking to it or competing with it.

use crate::Protocol;
use light_core::Universe;
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap},
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

/// A peer not heard for this long is forgotten. sACN sources announce every ten seconds, so this
/// survives two missed announcements.
pub const PEER_TIMEOUT: Duration = Duration::from_secs(25);

/// Send activity of one route destination.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RouteActivity {
    pub protocol: Protocol,
    pub universe: Universe,
    pub destination: SocketAddr,
    /// Milliseconds since the last packet left for this destination.
    pub last_sent_millis_ago: Option<u64>,
    /// The most recent send failure and how long ago it happened.
    pub last_error: Option<String>,
    pub last_error_millis_ago: Option<u64>,
    /// Failed sends since the output started.
    pub errors: u64,
}

/// A controller that sent the desk an ArtPoll.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObservedArtPoller {
    pub address: SocketAddr,
    pub polls: u64,
    pub last_seen_millis_ago: u64,
}

/// Another device broadcasting ArtDmx on the desk's network.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObservedArtNetSender {
    pub address: IpAddr,
    pub universes: Vec<Universe>,
    pub last_seen_millis_ago: u64,
}

/// Another sACN source heard on the desk's network.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObservedSacnSource {
    /// The source's CID as 32 hexadecimal digits.
    pub cid: String,
    pub name: String,
    pub address: IpAddr,
    pub universes: Vec<Universe>,
    pub last_seen_millis_ago: u64,
}

/// Everything the Nodes tab needs from the network output.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct NetworkActivity {
    pub routes: Vec<RouteActivity>,
    /// Where ArtPolls are answered.
    pub art_poll_listeners: Vec<SocketAddr>,
    /// Where sACN universe discovery is heard, if a listener could be opened.
    pub sacn_discovery_listener: Option<SocketAddr>,
    pub art_pollers: Vec<ObservedArtPoller>,
    pub art_net_senders: Vec<ObservedArtNetSender>,
    pub sacn_sources: Vec<ObservedSacnSource>,
}

#[derive(Default)]
struct RouteRecord {
    last_sent: Option<Instant>,
    last_error: Option<(String, Instant)>,
    errors: u64,
}

struct PollerRecord {
    polls: u64,
    last_seen: Instant,
}

struct SenderRecord {
    universes: HashMap<Universe, Instant>,
    last_seen: Instant,
}

struct SourceRecord {
    name: String,
    address: IpAddr,
    announced: BTreeSet<Universe>,
    /// Pages of the announcement being assembled, replaced when page zero arrives.
    pending: BTreeSet<Universe>,
    data: HashMap<Universe, Instant>,
    last_seen: Instant,
}

/// The mutable registry behind [`NetworkActivity`].
#[derive(Default)]
pub(crate) struct PeerRegistry {
    routes: HashMap<(Protocol, Universe, SocketAddr), RouteRecord>,
    pollers: HashMap<SocketAddr, PollerRecord>,
    senders: HashMap<IpAddr, SenderRecord>,
    sources: HashMap<[u8; 16], SourceRecord>,
}

fn millis(since: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(since).as_millis() as u64
}

impl PeerRegistry {
    pub(crate) fn record_sent(&mut self, key: (Protocol, Universe, SocketAddr), at: Instant) {
        self.routes.entry(key).or_default().last_sent = Some(at);
    }

    pub(crate) fn record_send_error(
        &mut self,
        key: (Protocol, Universe, SocketAddr),
        message: String,
        at: Instant,
    ) {
        let record = self.routes.entry(key).or_default();
        record.errors += 1;
        record.last_error = Some((message, at));
    }

    /// `(key, errors)` of every destination that failed at least once.
    pub(crate) fn route_errors(&self) -> Vec<((Protocol, Universe, SocketAddr), u64)> {
        self.routes
            .iter()
            .filter(|(_, record)| record.errors > 0)
            .map(|(key, record)| (*key, record.errors))
            .collect()
    }

    pub(crate) fn record_art_poll(&mut self, from: SocketAddr, at: Instant) {
        let record = self.pollers.entry(from).or_insert(PollerRecord {
            polls: 0,
            last_seen: at,
        });
        record.polls += 1;
        record.last_seen = at;
    }

    pub(crate) fn record_art_dmx(&mut self, from: IpAddr, universe: Universe, at: Instant) {
        let record = self.senders.entry(from).or_insert_with(|| SenderRecord {
            universes: HashMap::new(),
            last_seen: at,
        });
        record.universes.insert(universe, at);
        record.last_seen = at;
    }

    pub(crate) fn record_sacn_data(
        &mut self,
        cid: [u8; 16],
        name: String,
        from: IpAddr,
        universe: Universe,
        at: Instant,
    ) {
        let record = self.source(cid, name, from, at);
        record.data.insert(universe, at);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_sacn_discovery(
        &mut self,
        cid: [u8; 16],
        name: String,
        from: IpAddr,
        page: u8,
        last_page: u8,
        universes: &[Universe],
        at: Instant,
    ) {
        let record = self.source(cid, name, from, at);
        if page == 0 {
            record.pending.clear();
        }
        record.pending.extend(universes.iter().copied());
        if page >= last_page {
            record.announced = std::mem::take(&mut record.pending);
        }
    }

    fn source(
        &mut self,
        cid: [u8; 16],
        name: String,
        from: IpAddr,
        at: Instant,
    ) -> &mut SourceRecord {
        let record = self.sources.entry(cid).or_insert_with(|| SourceRecord {
            name: String::new(),
            address: from,
            announced: BTreeSet::new(),
            pending: BTreeSet::new(),
            data: HashMap::new(),
            last_seen: at,
        });
        record.name = name;
        record.address = from;
        record.last_seen = at;
        record
    }

    /// Forgets peers not heard within [`PEER_TIMEOUT`] and reports the rest.
    pub(crate) fn snapshot(&mut self, now: Instant) -> NetworkActivity {
        let fresh = |at: &Instant| now.saturating_duration_since(*at) < PEER_TIMEOUT;
        self.pollers.retain(|_, record| fresh(&record.last_seen));
        self.senders.retain(|_, record| {
            record.universes.retain(|_, at| fresh(at));
            fresh(&record.last_seen)
        });
        self.sources.retain(|_, record| {
            record.data.retain(|_, at| fresh(at));
            fresh(&record.last_seen)
        });
        NetworkActivity {
            routes: self.route_activity(now),
            art_pollers: self.pollers_at(now),
            art_net_senders: self.senders_at(now),
            sacn_sources: self.sources_at(now),
            ..NetworkActivity::default()
        }
    }

    fn route_activity(&self, now: Instant) -> Vec<RouteActivity> {
        let mut routes: Vec<_> = self
            .routes
            .iter()
            .map(
                |(&(protocol, universe, destination), record)| RouteActivity {
                    protocol,
                    universe,
                    destination,
                    last_sent_millis_ago: record.last_sent.map(|at| millis(at, now)),
                    last_error: record.last_error.as_ref().map(|(error, _)| error.clone()),
                    last_error_millis_ago: record
                        .last_error
                        .as_ref()
                        .map(|(_, at)| millis(*at, now)),
                    errors: record.errors,
                },
            )
            .collect();
        routes.sort_by_key(|route| (route.protocol as u8, route.universe, route.destination));
        routes
    }

    fn pollers_at(&self, now: Instant) -> Vec<ObservedArtPoller> {
        let mut pollers: Vec<_> = self
            .pollers
            .iter()
            .map(|(address, record)| ObservedArtPoller {
                address: *address,
                polls: record.polls,
                last_seen_millis_ago: millis(record.last_seen, now),
            })
            .collect();
        pollers.sort_by_key(|poller| poller.address);
        pollers
    }

    fn senders_at(&self, now: Instant) -> Vec<ObservedArtNetSender> {
        let mut senders: Vec<_> = self
            .senders
            .iter()
            .map(|(address, record)| {
                let mut universes: Vec<_> = record.universes.keys().copied().collect();
                universes.sort_unstable();
                ObservedArtNetSender {
                    address: *address,
                    universes,
                    last_seen_millis_ago: millis(record.last_seen, now),
                }
            })
            .collect();
        senders.sort_by_key(|sender| sender.address);
        senders
    }

    fn sources_at(&self, now: Instant) -> Vec<ObservedSacnSource> {
        let mut sources: Vec<_> = self
            .sources
            .iter()
            .map(|(cid, record)| {
                let mut universes = record.announced.clone();
                universes.extend(record.data.keys().copied());
                ObservedSacnSource {
                    cid: cid.iter().map(|byte| format!("{byte:02x}")).collect(),
                    name: record.name.clone(),
                    address: record.address,
                    universes: universes.into_iter().collect(),
                    last_seen_millis_ago: millis(record.last_seen, now),
                }
            })
            .collect();
        sources.sort_by(|left, right| (left.address, &left.cid).cmp(&(right.address, &right.cid)));
        sources
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn a_multi_page_announcement_replaces_the_previous_one_when_complete() {
        let mut registry = PeerRegistry::default();
        let now = Instant::now();
        let from = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));
        registry.record_sacn_discovery([1; 16], "A".into(), from, 0, 0, &[1, 2], now);
        registry.record_sacn_discovery([1; 16], "A".into(), from, 0, 1, &[5], now);
        assert_eq!(registry.snapshot(now).sacn_sources[0].universes, [1, 2]);
        registry.record_sacn_discovery([1; 16], "A".into(), from, 1, 1, &[6], now);
        assert_eq!(registry.snapshot(now).sacn_sources[0].universes, [5, 6]);
    }

    #[test]
    fn silent_peers_are_forgotten() {
        let mut registry = PeerRegistry::default();
        let then = Instant::now();
        let from = SocketAddr::from((Ipv4Addr::new(10, 0, 0, 2), 6454));
        registry.record_art_poll(from, then);
        registry.record_art_dmx(from.ip(), 4, then);
        let activity = registry.snapshot(then + Duration::from_secs(1));
        assert_eq!(activity.art_pollers.len(), 1);
        assert_eq!(activity.art_net_senders[0].universes, [4]);
        let later = registry.snapshot(then + PEER_TIMEOUT);
        assert!(later.art_pollers.is_empty());
        assert!(later.art_net_senders.is_empty());
    }
}
