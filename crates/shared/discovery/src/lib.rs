//! Finding the other ToskLight on the network.
//!
//! A rig planned in the Viz editor and a show running on a desk are the same rig, and moving one
//! to the other should not mean hunting for a file. Both applications advertise themselves here
//! and browse for each other, so each can offer to load what the other is holding.
//!
//! One service type carries both, with the role in the record: one browser finds everything, and
//! the two sides cannot drift onto different names. A side with nothing loaded still advertises —
//! it is discoverable, it simply has nothing to offer, and saying so is more useful than
//! disappearing.
//!
//! Discovery is a convenience and never a dependency: a network with no mDNS, or a firewall that
//! blocks it, costs the offer and nothing else. Everything here fails quietly for that reason.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

/// The service both applications advertise under.
pub const SERVICE_TYPE: &str = "_tosklight._tcp.local.";

/// Which ToskLight this is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// A lighting desk: the one running the show.
    Desk,
    /// The Viz editor: a planning document, with no desk behind it.
    Editor,
}

impl Role {
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Desk => "desk",
            Self::Editor => "editor",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "desk" => Some(Self::Desk),
            "editor" => Some(Self::Editor),
            _ => None,
        }
    }
}

/// One ToskLight seen on the network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Peer {
    pub role: Role,
    /// What this one calls itself: a desk's own name, or the editor's product name.
    pub name: String,
    /// The show or document it is holding. `None` means it has nothing loaded, which is a peer
    /// worth showing and not worth offering to load from.
    pub show: Option<String>,
    /// Where to reach its API, in the order worth trying: a machine answers on every interface
    /// it has, and only the peer's own network knows which of them a caller can actually reach.
    /// Never empty — a record with no usable address is not a peer.
    pub addresses: Vec<String>,
    /// The service instance name, which is what makes two peers with the same display name
    /// distinguishable.
    pub instance: String,
}

impl Peer {
    /// The address to show an operator, and the first one to try.
    pub fn address(&self) -> &str {
        self.addresses.first().map_or("", String::as_str)
    }

    /// `http://host:port` for each address, in the order worth trying. A caller that gives up
    /// after the first has given up on a rig whose desk simply answers on more than one
    /// interface.
    pub fn base_urls(&self) -> impl Iterator<Item = String> + '_ {
        self.addresses
            .iter()
            .map(|address| format!("http://{address}"))
    }
}

/// What this application publishes about itself.
#[derive(Clone, Debug)]
pub struct Advertisement {
    pub role: Role,
    pub name: String,
    pub show: Option<String>,
    pub port: u16,
}

/// A running advertisement. Dropping it withdraws the service.
pub struct Advertiser {
    daemon: ServiceDaemon,
    full_name: Mutex<String>,
    advertisement: Mutex<Advertisement>,
}

impl Advertiser {
    /// Start advertising, or answer why not.
    pub fn start(advertisement: Advertisement) -> Result<Self, String> {
        let daemon = ServiceDaemon::new().map_err(|error| error.to_string())?;
        let info = service_info(&advertisement)?;
        let full_name = info.get_fullname().to_owned();
        daemon
            .register(info)
            .map_err(|error| format!("advertising this {}: {error}", advertisement.role.wire()))?;
        Ok(Self {
            daemon,
            full_name: Mutex::new(full_name),
            advertisement: Mutex::new(advertisement),
        })
    }

    /// Re-publish with a different show, when the operator loads or closes one.
    ///
    /// The record is what a peer decides from, so it has to follow the application rather than
    /// describe the moment it started.
    pub fn set_show(&self, show: Option<String>) {
        let mut advertisement = self.advertisement.lock().expect("advertisement");
        if advertisement.show == show {
            return;
        }
        advertisement.show = show;
        let Ok(info) = service_info(&advertisement) else {
            return;
        };
        let next = info.get_fullname().to_owned();
        let mut full_name = self.full_name.lock().expect("advertised name");
        // Re-registering the same instance replaces its records, which is exactly what a changed
        // show is. Withdrawing first would make this side blink out of every peer's list.
        if self.daemon.register(info).is_ok() {
            *full_name = next;
        }
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        let name = self.full_name.lock().expect("advertised name").clone();
        // A goodbye is what stops a peer offering a desk that has just quit. It is best-effort:
        // the timeout covers everything this cannot.
        let _ = self.daemon.unregister(&name);
        let _ = self.daemon.shutdown();
    }
}

fn service_info(advertisement: &Advertisement) -> Result<ServiceInfo, String> {
    let mut properties = HashMap::from([
        ("role".to_owned(), advertisement.role.wire().to_owned()),
        ("name".to_owned(), advertisement.name.clone()),
    ]);
    if let Some(show) = &advertisement.show {
        properties.insert("show".to_owned(), show.clone());
    }
    // The instance name has to be stable for this application on this machine, or every change of
    // show would arrive at a browser as a different peer.
    let instance = instance_name(advertisement.role);
    ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &format!("{instance}.local."),
        (),
        advertisement.port,
        properties,
    )
    .map(|info| info.enable_addr_auto())
    .map_err(|error| error.to_string())
}

/// `tosklight-desk-<host>`: stable for this application on this machine, and readable in any
/// service browser an operator happens to have open.
fn instance_name(role: Role) -> String {
    let host = hostname();
    format!("tosklight-{}-{host}", role.wire())
}

/// This machine's short name, which is what both applications name themselves after when nothing
/// else has named them.
pub fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
        })
        .map(|value| value.trim().trim_end_matches(".local").to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "tosklight".to_owned())
        .replace(['.', ' '], "-")
}

/// A running browse. Dropping it stops looking.
pub struct Browser {
    daemon: ServiceDaemon,
    peers: Arc<Mutex<HashMap<String, Peer>>>,
}

impl Browser {
    /// Start looking for other ToskLights, or answer why not.
    pub fn start() -> Result<Self, String> {
        let daemon = ServiceDaemon::new().map_err(|error| error.to_string())?;
        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|error| format!("browsing for ToskLights: {error}"))?;
        let peers: Arc<Mutex<HashMap<String, Peer>>> = Arc::default();
        let sink = peers.clone();
        std::thread::Builder::new()
            .name("tosklight-discovery".into())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            if let Some(peer) = peer_from(&info) {
                                sink.lock()
                                    .expect("peers")
                                    .insert(peer.instance.clone(), peer);
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, full_name) => {
                            sink.lock().expect("peers").remove(&full_name);
                        }
                        _ => {}
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { daemon, peers })
    }

    /// Everything currently on the network, in a stable order so a menu does not reshuffle
    /// itself.
    ///
    /// A peer leaves this list when the responder says it has: a goodbye when an application
    /// exits cleanly, and record expiry when it cannot — a machine unplugged, a process killed, a
    /// network that dropped underneath it. Nothing is dropped merely for having been quiet,
    /// because a desk that has sat there unchanged all afternoon is exactly the desk an operator
    /// wants offered.
    pub fn peers(&self) -> Vec<Peer> {
        let peers = self.peers.lock().expect("peers");
        let mut found: Vec<Peer> = peers.values().cloned().collect();
        found.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.instance.cmp(&right.instance))
        });
        found
    }

    /// Everything of one role. What a caller almost always wants: an editor looks for desks.
    pub fn peers_with_role(&self, role: Role) -> Vec<Peer> {
        self.peers()
            .into_iter()
            .filter(|peer| peer.role == role)
            .collect()
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}

fn peer_from(resolved: &mdns_sd::ResolvedService) -> Option<Peer> {
    let property = |key: &str| {
        resolved
            .txt_properties
            .get_property_val_str(key)
            .map(str::to_owned)
    };
    let role = Role::from_wire(&property("role")?)?;
    let addresses: Vec<IpAddr> = resolved
        .addresses
        .iter()
        .map(|address| address.to_ip_addr())
        .collect();
    let addresses = preferred_addresses(&addresses, resolved.port);
    if addresses.is_empty() {
        return None;
    }
    Some(Peer {
        role,
        name: property("name").unwrap_or_else(|| resolved.fullname.clone()),
        show: property("show").filter(|show| !show.trim().is_empty()),
        addresses,
        instance: resolved.fullname.clone(),
    })
}

/// A peer's addresses, in the order worth trying, and without the ones that cannot work.
///
/// A machine answers on every interface it has, and the set arrives unordered — so picking
/// whichever came first produces a different answer each time, and often an unreachable one. The
/// order here is by how likely an address is to work: a routable IPv4 first because that is the
/// lighting network, loopback next because it is how two of these run on one machine, and a
/// routable IPv6 after that. A link-local IPv6 is dropped entirely: it needs an interface scope
/// that no URL carries, so it would only ever fail when pressed.
fn preferred_addresses(addresses: &[IpAddr], port: u16) -> Vec<String> {
    let mut candidates: Vec<(u8, String)> = addresses
        .iter()
        .filter_map(|address| {
            let rank = match address {
                IpAddr::V4(value) if value.is_loopback() => 1,
                IpAddr::V4(_) => 0,
                IpAddr::V6(value) if value.is_loopback() => 3,
                // fe80::/10, which cannot be reached without naming the interface it is on.
                IpAddr::V6(value) if (value.segments()[0] & 0xffc0) == 0xfe80 => return None,
                IpAddr::V6(_) => 2,
            };
            Some((rank, address.to_string()))
        })
        .collect();
    // Sorted rather than min-by so the same rig always produces the same order, which is what
    // makes a peer's entry stable between two reads of the list.
    candidates.sort();
    candidates.dedup();
    candidates
        .into_iter()
        .map(|(_, address)| {
            if address.contains(':') {
                format!("[{address}]:{port}")
            } else {
                format!("{address}:{port}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_survives_the_trip_through_a_service_record() {
        for role in [Role::Desk, Role::Editor] {
            assert_eq!(Role::from_wire(role.wire()), Some(role));
        }
        assert_eq!(Role::from_wire("something else"), None);
    }

    /// The instance name is what a browser keys a peer on, so it must not move when the show does.
    #[test]
    fn the_instance_name_is_stable_for_this_application_on_this_machine() {
        assert_eq!(instance_name(Role::Desk), instance_name(Role::Desk));
        assert_ne!(instance_name(Role::Desk), instance_name(Role::Editor));
        assert!(instance_name(Role::Editor).starts_with("tosklight-editor-"));
        // Anything a hostname can contain that a DNS-SD instance name should not.
        assert!(!instance_name(Role::Desk).contains(' '));
    }

    #[test]
    fn a_peer_says_where_to_reach_it() {
        let peer = Peer {
            role: Role::Desk,
            name: "FOH desk".into(),
            show: Some("Summer Tour".into()),
            addresses: vec!["10.0.0.9:5000".into(), "127.0.0.1:5000".into()],
            instance: "tosklight-desk-foh._tosklight._tcp.local.".into(),
        };
        assert_eq!(peer.address(), "10.0.0.9:5000");
        assert_eq!(
            peer.base_urls().collect::<Vec<_>>(),
            vec!["http://10.0.0.9:5000", "http://127.0.0.1:5000"]
        );
    }

    #[test]
    fn the_addresses_a_peer_is_reached_at_are_the_ones_that_can_be() {
        let addresses = |values: &[&str]| {
            preferred_addresses(
                &values
                    .iter()
                    .map(|value| value.parse().unwrap())
                    .collect::<Vec<_>>(),
                5000,
            )
        };
        // The lighting network first, then the rest — a desk answering on a hotspot bridge as
        // well as the house network must not become unreachable because the wrong one sorted
        // first.
        assert_eq!(
            addresses(&["fe80::1", "fd72:e8d0::7a", "192.168.42.184", "127.0.0.1"]),
            vec![
                "192.168.42.184:5000".to_owned(),
                "127.0.0.1:5000".to_owned(),
                "[fd72:e8d0::7a]:5000".to_owned(),
            ]
        );
        // Two of these on one machine still find each other.
        assert_eq!(
            addresses(&["fe80::1", "127.0.0.1"]),
            vec!["127.0.0.1:5000".to_owned()]
        );
        // An IPv6-only rig works, and is bracketed so the URL parses.
        assert_eq!(
            addresses(&["fe80::1c47", "fd72:e8d0::7a"]),
            vec!["[fd72:e8d0::7a]:5000".to_owned()]
        );
        // Link-local only is no address at all: it would parse and never connect.
        assert!(addresses(&["fe80::1c47", "fe80::1"]).is_empty());
        assert!(addresses(&[]).is_empty());
    }

    /// A record that is not a ToskLight, or is one this build cannot understand, is ignored rather
    /// than offered as a peer nothing can be loaded from.
    #[test]
    fn an_advertisement_carries_the_role_and_the_show() {
        let advertisement = Advertisement {
            role: Role::Editor,
            name: "ToskLight Viz Editor".into(),
            show: Some("Summer Tour rig".into()),
            port: 5310,
        };
        let info = service_info(&advertisement).expect("a service record");
        assert_eq!(info.get_property_val_str("role"), Some("editor"));
        assert_eq!(info.get_property_val_str("show"), Some("Summer Tour rig"));
        assert_eq!(info.get_port(), 5310);

        let empty = Advertisement {
            show: None,
            ..advertisement
        };
        let info = service_info(&empty).expect("a service record");
        assert_eq!(info.get_property_val_str("role"), Some("editor"));
        assert_eq!(
            info.get_property_val_str("show"),
            None,
            "a side with nothing loaded advertises, and says it has nothing"
        );
    }
}

#[cfg(test)]
mod network_tests {
    use super::*;
    use std::time::Duration;

    /// Proves the two halves actually meet: an advertisement made here is found by a browse
    /// here, with its role and show intact.
    ///
    /// Ignored by default because it needs a working multicast responder, which a build machine
    /// or a locked-down container will not have — and a discovery failure there is exactly the
    /// quiet no-op the rest of this module is built around. Run it on a real machine with
    /// `cargo test -p light-discovery -- --ignored`.
    #[test]
    #[ignore = "needs multicast on the local network"]
    fn an_advertised_desk_is_found_by_a_browse() {
        let advertiser = Advertiser::start(Advertisement {
            role: Role::Desk,
            name: "discovery test desk".to_owned(),
            show: Some("Summer Tour".to_owned()),
            port: 5000,
        })
        .expect("advertising");
        let browser = Browser::start().expect("browsing");
        let mut found = None;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(200));
            found = browser
                .peers_with_role(Role::Desk)
                .into_iter()
                .find(|peer| peer.name == "discovery test desk");
            if found.is_some() {
                break;
            }
        }
        let peer = found.expect("the advertised desk within ten seconds");
        assert_eq!(peer.show.as_deref(), Some("Summer Tour"));
        assert!(peer.address().ends_with(":5000"), "{}", peer.address());
        drop(advertiser);
    }
}
