//! `GET /api/v2/output/network-endpoints`: every Art-Net and sACN endpoint the desk sends to or
//! hears from, with a status the operator can act on. The DMX screen's Nodes tab reads it.

use super::*;
use light_output::{DeliveryMode, NetworkActivity, OutputRoute, Protocol, RouteActivity};
use light_wire::v2::events::{OutputDeliveryMode, OutputProtocol};
use light_wire::v2::network_endpoints::{
    NetworkEndpoint, NetworkEndpointDirection as Direction, NetworkEndpointOrigin as Origin,
    NetworkEndpointStatus as Status, NetworkEndpointsSnapshot,
};
use std::net::{IpAddr, SocketAddr};

/// Data last seen within this window is flowing now; the output's own loss rule for sACN.
const ACTIVE_WITHIN_MILLIS: u64 = 2_500;
/// sACN sources announce their universes this often.
const SACN_ANNOUNCEMENT_MILLIS: u64 = 10_000;

pub(super) async fn network_endpoints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NetworkEndpointsSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let configuration = state.installation.configuration();
    let (_, timecode) = state.output.timecode_status();
    let timecode = timecode.map(|timecode| {
        let age = (chrono::Utc::now() - timecode.received_at)
            .num_milliseconds()
            .max(0) as u64;
        (timecode.source, age)
    });
    Ok(Json(project(&EndpointInputs {
        routes: &state.output.snapshot().routes,
        activity: state.output.network_activity(),
        output_bind_ip: configuration.output_bind_ip,
        art_timecode_bind: configuration.art_timecode_bind,
        timecode,
    })))
}

pub(super) struct EndpointInputs<'a> {
    pub(super) routes: &'a [OutputRoute],
    pub(super) activity: Option<NetworkActivity>,
    pub(super) output_bind_ip: IpAddr,
    pub(super) art_timecode_bind: Option<SocketAddr>,
    /// The last timecode's source (`artnet:<ip>:<stream>` for Art-Net) and its age.
    pub(super) timecode: Option<(String, u64)>,
}

pub(super) fn project(inputs: &EndpointInputs<'_>) -> NetworkEndpointsSnapshot {
    let available = inputs.activity.is_some();
    let activity = inputs.activity.clone().unwrap_or_default();
    let mut endpoints = send_endpoints(inputs.routes, &activity, available);
    endpoints.extend(listener_endpoints(inputs, &activity));
    endpoints.extend(peer_endpoints(inputs.routes, &activity));
    NetworkEndpointsSnapshot {
        output_bind_ip: inputs.output_bind_ip.to_string(),
        network_output_available: available,
        endpoints,
    }
}

fn wire_protocol(protocol: Protocol) -> OutputProtocol {
    match protocol {
        Protocol::ArtNet => OutputProtocol::ArtNet,
        Protocol::Sacn => OutputProtocol::Sacn,
    }
}

fn protocol_label(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::ArtNet => "Art-Net",
        Protocol::Sacn => "sACN",
    }
}

fn protocol_key(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::ArtNet => "artnet",
        Protocol::Sacn => "sacn",
    }
}

fn wire_delivery(mode: DeliveryMode) -> OutputDeliveryMode {
    match mode {
        DeliveryMode::Broadcast => OutputDeliveryMode::Broadcast,
        DeliveryMode::Multicast => OutputDeliveryMode::Multicast,
        DeliveryMode::Unicast => OutputDeliveryMode::Unicast,
    }
}

fn endpoint(
    id: String,
    protocol: Protocol,
    direction: Direction,
    origin: Origin,
    role: &str,
    address: String,
) -> NetworkEndpoint {
    NetworkEndpoint {
        id,
        protocol: wire_protocol(protocol),
        direction,
        origin,
        role: role.into(),
        endpoint: address,
        name: None,
        software: None,
        delivery_mode: None,
        logical_universe: None,
        universes: Vec::new(),
        status: Status::Idle,
        detail: String::new(),
        errors: 0,
        last_activity_millis_ago: None,
    }
}

/// Other sources heard sending `universe` on `protocol`, by name or address.
fn competitors(activity: &NetworkActivity, protocol: Protocol, universe: u16) -> Vec<String> {
    match protocol {
        Protocol::ArtNet => activity
            .art_net_senders
            .iter()
            .filter(|sender| sender.universes.contains(&universe))
            .map(|sender| sender.address.to_string())
            .collect(),
        Protocol::Sacn => activity
            .sacn_sources
            .iter()
            .filter(|source| source.universes.contains(&universe))
            .map(|source| match source.name.trim() {
                "" => source.address.to_string(),
                name => format!("{name} ({})", source.address),
            })
            .collect(),
    }
}

fn send_endpoints(
    routes: &[OutputRoute],
    activity: &NetworkActivity,
    available: bool,
) -> Vec<NetworkEndpoint> {
    let mut endpoints: Vec<NetworkEndpoint> = routes
        .iter()
        .filter(|route| route.target.is_network())
        .map(|route| route_endpoint(route, activity, available))
        .collect();
    for protocol in [Protocol::ArtNet, Protocol::Sacn] {
        if let Some(announcement) = announcement_endpoint(routes, activity, protocol, available) {
            endpoints.push(announcement);
        }
    }
    endpoints
}

fn route_endpoint(
    route: &OutputRoute,
    activity: &NetworkActivity,
    available: bool,
) -> NetworkEndpoint {
    let destination = route.resolved_destination();
    let address = destination
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|_| "—".into());
    let mode = route.resolved_delivery_mode();
    let mut item = endpoint(
        format!(
            "send:{}:{}:{}:{address}",
            protocol_key(route.protocol),
            route.logical_universe,
            route.destination_universe
        ),
        route.protocol,
        Direction::Send,
        Origin::Configured,
        "DMX output",
        address,
    );
    item.delivery_mode = Some(wire_delivery(mode));
    item.logical_universe = Some(route.logical_universe);
    item.universes = vec![route.destination_universe];
    let record = destination.as_ref().ok().and_then(|destination| {
        activity.routes.iter().find(|record| {
            record.protocol == route.protocol
                && record.universe == route.destination_universe
                && record.destination == *destination
        })
    });
    item.errors = record.map_or(0, |record| record.errors);
    item.last_activity_millis_ago = record.and_then(|record| record.last_sent_millis_ago);
    (item.status, item.detail) = route_status(route, record, activity, available);
    item
}

fn route_status(
    route: &OutputRoute,
    record: Option<&RouteActivity>,
    activity: &NetworkActivity,
    available: bool,
) -> (Status, String) {
    let label = protocol_label(route.protocol);
    if let Err(error) = route.validate() {
        return (
            Status::Error,
            format!("{error}. Correct the route in Desk Setup > Outputs > Routes."),
        );
    }
    if !route.enabled {
        return (
            Status::Disabled,
            "Route is off. Enable it in Desk Setup > Outputs > Routes to send this universe."
                .into(),
        );
    }
    if !available {
        return (
            Status::Unavailable,
            "Network output is not running. Check the Output bind address in Desk Setup > Outputs."
                .into(),
        );
    }
    let sent = record.and_then(|record| record.last_sent_millis_ago);
    if let Some(record) = record
        && let (Some(error), Some(error_age)) = (&record.last_error, record.last_error_millis_ago)
        && sent.is_none_or(|sent| error_age <= sent)
    {
        return (
            Status::Error,
            format!(
                "Sending fails: {error}. Check the destination address and that the Output bind address can reach it."
            ),
        );
    }
    let others = competitors(activity, route.protocol, route.destination_universe);
    if !others.is_empty() {
        return (
            Status::Conflict,
            format!(
                "{label} universe {} is also sent by {}. Two sources on one universe fight over the rig; move one of them to another universe.",
                route.destination_universe,
                others.join(", ")
            ),
        );
    }
    match sent {
        Some(age) if age <= ACTIVE_WITHIN_MILLIS => (
            Status::Active,
            format!(
                "Sending logical universe {} as {label} universe {}.",
                route.logical_universe, route.destination_universe
            ),
        ),
        Some(age) => (
            Status::Idle,
            format!(
                "No frame sent for {} s. Check that DMX output is running.",
                age / 1_000
            ),
        ),
        None => (
            Status::Idle,
            "No frame sent yet. Output starts when a show is loaded.".into(),
        ),
    }
}

/// The desk announcing what it sends: ArtPollReply for Art-Net, universe discovery for sACN.
fn announcement_endpoint(
    routes: &[OutputRoute],
    activity: &NetworkActivity,
    protocol: Protocol,
    available: bool,
) -> Option<NetworkEndpoint> {
    let mut universes = sent_universes(routes, protocol);
    universes.sort_unstable();
    universes.dedup();
    let (role, address, detail) = match protocol {
        Protocol::ArtNet => {
            if activity.art_poll_listeners.is_empty() {
                return None;
            }
            (
                "ArtPoll reply",
                "Controllers that poll".to_string(),
                format!(
                    "Answers ArtPolls with {} Art-Net universe(s) as input ports.",
                    universes.len()
                ),
            )
        }
        Protocol::Sacn => {
            if universes.is_empty() {
                return None;
            }
            (
                "Universe discovery",
                light_output::sacn_multicast_destination(light_output::SACN_DISCOVERY_UNIVERSE)
                    .to_string(),
                format!("Announces {} sACN universe(s) every 10 s.", universes.len()),
            )
        }
    };
    let mut item = endpoint(
        format!("send:{}:announce", protocol_key(protocol)),
        protocol,
        Direction::Send,
        Origin::Configured,
        role,
        address,
    );
    item.universes = universes;
    (item.status, item.detail) = if available {
        (Status::Active, detail)
    } else {
        (
            Status::Unavailable,
            "Network output is not running, so the desk is not announced.".into(),
        )
    };
    Some(item)
}

fn listener_endpoints(
    inputs: &EndpointInputs<'_>,
    activity: &NetworkActivity,
) -> Vec<NetworkEndpoint> {
    let mut endpoints = Vec::new();
    if let Some(bind) = inputs.art_timecode_bind {
        endpoints.push(timecode_endpoint(bind, inputs.timecode.as_ref()));
    }
    let available = inputs.activity.is_some();
    if activity.art_poll_listeners.is_empty() {
        let mut item = endpoint(
            "receive:artnet:poll".into(),
            Protocol::ArtNet,
            Direction::Receive,
            Origin::Configured,
            "ArtPoll listener",
            format!("{}:{}", inputs.output_bind_ip, light_output::ARTNET_PORT),
        );
        item.status = Status::Unavailable;
        item.detail = if available {
            "No broadcast network on the output interface, so controllers cannot find the desk. Set the Output bind address in Desk Setup > Outputs to an interface with an IPv4 broadcast address.".into()
        } else {
            "Network output is not running.".into()
        };
        endpoints.push(item);
    }
    for listener in &activity.art_poll_listeners {
        let mut item = endpoint(
            format!("receive:artnet:poll:{listener}"),
            Protocol::ArtNet,
            Direction::Receive,
            Origin::Configured,
            "ArtPoll listener",
            listener.to_string(),
        );
        item.status = Status::Listening;
        item.detail = "Answers controllers looking for Art-Net devices.".into();
        endpoints.push(item);
    }
    let group = light_output::sacn_multicast_destination(light_output::SACN_DISCOVERY_UNIVERSE);
    let mut item = endpoint(
        "receive:sacn:discovery".into(),
        Protocol::Sacn,
        Direction::Receive,
        Origin::Configured,
        "Universe discovery listener",
        activity
            .sacn_discovery_listener
            .map_or_else(|| group.to_string(), |listener| listener.to_string()),
    );
    (item.status, item.detail) = match activity.sacn_discovery_listener {
        Some(_) => (
            Status::Listening,
            "Hears other sACN sources announcing their universes.".into(),
        ),
        None => (
            Status::Unavailable,
            "Could not join the sACN discovery group, so other sources and universe conflicts are not detected. Check that the Output bind address is on a network that allows multicast.".into(),
        ),
    };
    endpoints.push(item);
    endpoints
}

fn timecode_endpoint(bind: SocketAddr, timecode: Option<&(String, u64)>) -> NetworkEndpoint {
    let mut item = endpoint(
        "receive:artnet:timecode".into(),
        Protocol::ArtNet,
        Direction::Receive,
        Origin::Configured,
        "Timecode input",
        bind.to_string(),
    );
    let art_net = timecode.and_then(|(source, age)| {
        let mut parts = source.strip_prefix("artnet:")?.rsplitn(2, ':');
        let _stream = parts.next();
        Some((parts.next().unwrap_or_default().to_string(), *age))
    });
    item.last_activity_millis_ago = art_net.as_ref().map(|(_, age)| *age);
    (item.status, item.detail) = match art_net {
        Some((sender, age)) if age <= ACTIVE_WITHIN_MILLIS => {
            item.name = Some(sender.clone());
            (
                Status::Active,
                format!("Receiving Art-Net timecode from {sender}."),
            )
        }
        Some((sender, age)) => (
            Status::Listening,
            format!(
                "Last Art-Net timecode from {sender} {} s ago. Check that the timecode source is running.",
                age / 1_000
            ),
        ),
        None => (
            Status::Listening,
            "Waiting for Art-Net timecode. Point the timecode source at this address.".into(),
        ),
    };
    item
}

fn sent_universes(routes: &[OutputRoute], protocol: Protocol) -> Vec<u16> {
    routes
        .iter()
        .filter(|route| route.enabled && route.target.is_network() && route.protocol == protocol)
        .map(|route| route.destination_universe)
        .collect()
}

fn peer_status(
    protocol: Protocol,
    universes: &[u16],
    sent: &[u16],
    (current, age): (bool, u64),
    active: String,
) -> (Status, String) {
    let shared: Vec<String> = universes
        .iter()
        .filter(|universe| sent.contains(universe))
        .map(ToString::to_string)
        .collect();
    if !shared.is_empty() {
        return (
            Status::Conflict,
            format!(
                "Also sends {} universe {} that this desk sends. Move one source to another universe.",
                protocol_label(protocol),
                shared.join(", ")
            ),
        );
    }
    if current {
        (Status::Active, active)
    } else {
        (Status::Idle, format!("Last heard {} s ago.", age / 1_000))
    }
}

/// Which ToskLight application announced itself under `announced`, if any.
///
/// The desk's own Media Server and Visualizer name themselves on the network so an operator can
/// tell them apart from third-party hardware; see `light_dmx_wire::TOSKLIGHT_SOFTWARE_NAMES`.
fn own_software(announced: &str) -> Option<String> {
    light_output::tosklight_software(announced).map(str::to_owned)
}

fn peer_endpoints(routes: &[OutputRoute], activity: &NetworkActivity) -> Vec<NetworkEndpoint> {
    let mut endpoints = Vec::new();
    for poller in &activity.art_pollers {
        let mut item = endpoint(
            format!("receive:artnet:poller:{}", poller.address),
            Protocol::ArtNet,
            Direction::Receive,
            Origin::Observed,
            "Controller polling the desk",
            poller.address.to_string(),
        );
        item.name.clone_from(&poller.announced_name);
        item.software = poller.announced_name.as_deref().and_then(own_software);
        item.last_activity_millis_ago = Some(poller.last_seen_millis_ago);
        (item.status, item.detail) = if poller.last_seen_millis_ago <= 10_000 {
            (
                Status::Active,
                format!("Sent {} ArtPoll(s); the desk answers each.", poller.polls),
            )
        } else {
            (
                Status::Idle,
                format!("Last polled {} s ago.", poller.last_seen_millis_ago / 1_000),
            )
        };
        endpoints.push(item);
    }
    let art_net_sent = sent_universes(routes, Protocol::ArtNet);
    for sender in &activity.art_net_senders {
        let mut item = endpoint(
            format!("receive:artnet:sender:{}", sender.address),
            Protocol::ArtNet,
            Direction::Receive,
            Origin::Observed,
            "Art-Net sender",
            format!("{}:{}", sender.address, light_output::ARTNET_PORT),
        );
        item.name.clone_from(&sender.announced_name);
        item.software = sender.announced_name.as_deref().and_then(own_software);
        item.universes.clone_from(&sender.universes);
        item.last_activity_millis_ago = Some(sender.last_seen_millis_ago);
        (item.status, item.detail) = peer_status(
            Protocol::ArtNet,
            &sender.universes,
            &art_net_sent,
            (
                sender.last_seen_millis_ago <= ACTIVE_WITHIN_MILLIS,
                sender.last_seen_millis_ago,
            ),
            "Broadcasts ArtDmx on this network.".into(),
        );
        endpoints.push(item);
    }
    let sacn_sent = sent_universes(routes, Protocol::Sacn);
    for source in &activity.sacn_sources {
        let mut item = endpoint(
            format!("receive:sacn:source:{}", source.cid),
            Protocol::Sacn,
            Direction::Receive,
            Origin::Observed,
            "sACN source",
            format!("{}:{}", source.address, light_output::SACN_PORT),
        );
        item.name = (!source.name.trim().is_empty()).then(|| source.name.clone());
        item.software = own_software(&source.name);
        item.universes.clone_from(&source.universes);
        item.last_activity_millis_ago = Some(source.last_seen_millis_ago);
        (item.status, item.detail) = peer_status(
            Protocol::Sacn,
            &source.universes,
            &sacn_sent,
            // Announcements repeat every 10 s, so a source is current until one is missed.
            (
                source.last_seen_millis_ago <= SACN_ANNOUNCEMENT_MILLIS + ACTIVE_WITHIN_MILLIS,
                source.last_seen_millis_ago,
            ),
            "Announces its universes on this network.".into(),
        );
        endpoints.push(item);
    }
    endpoints
}

#[cfg(test)]
#[path = "network_endpoints_api_tests.rs"]
mod tests;
