//! Network settings.
//!
//! Listen addresses and destinations are separate concepts and stay separate here. A listen
//! address is something this process binds; a destination is somewhere it sends. `0.0.0.0` means
//! every local interface — it is a listen address and never a destination, which is exactly the
//! distinction a settings panel has to make visible.
//!
//! The stored settings and the addresses this run actually bound are both reported, because the
//! same-computer preset makes them differ on purpose: an operator must be able to see what they
//! typed and what the process is using at the same time.
//!
//! Art-Net, sACN and the Speed Group listener are receive-only UDP sockets the running process
//! rebinds as soon as an edit is accepted. CITP and the administration interface are bound once:
//! consoles hold TCP sessions to CITP and discover it by the port it announced, and the HTTP
//! listener serves the very page making the edit. Only those two wait for a restart.

use std::net::SocketAddr;

use media_application::configuration::{NetworkConfiguration, ResolvedNetwork, SPEED_GROUP_PORT};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One set of addresses, rendered as text because that is what an operator types and reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAddressesView {
    pub art_net_listen: String,
    pub sacn_listen: String,
    pub citp_listen: String,
    pub http_listen: String,
    /// Where this server listens for the Light desk's Speed Group OSC stream. Absent means Speed
    /// Groups are not received. Media never publishes Speed Groups.
    pub speed_group_endpoint: Option<String>,
}

impl NetworkAddressesView {
    fn stored(network: &NetworkConfiguration) -> Self {
        Self {
            art_net_listen: network.art_net_listen.to_string(),
            sacn_listen: network.sacn_listen.to_string(),
            citp_listen: network.citp_listen.to_string(),
            http_listen: network.http_listen.to_string(),
            speed_group_endpoint: network
                .speed_group_endpoint
                .map(|endpoint| endpoint.to_string()),
        }
    }

    fn resolved(resolved: &ResolvedNetwork) -> Self {
        Self {
            art_net_listen: resolved.art_net_listen.to_string(),
            sacn_listen: resolved.sacn_listen.to_string(),
            citp_listen: resolved.citp_listen.to_string(),
            http_listen: resolved.http_listen.to_string(),
            speed_group_endpoint: resolved
                .speed_group_endpoint
                .map(|endpoint| endpoint.to_string()),
        }
    }
}

/// The network fields a running process cannot rebind.
pub const RESTART_FIELDS: [&str; 2] = ["citpListen", "httpListen"];

/// The network settings, as the API reports them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct NetworkView {
    /// When set, every protocol listens on IPv4 loopback without touching the stored addresses.
    pub same_computer_preset: bool,
    /// What the operator configured.
    pub stored: NetworkAddressesView,
    /// What this process was started with, used by Revert to current settings.
    pub active_same_computer_preset: bool,
    pub active_stored: NetworkAddressesView,
    /// What this run is bound to now, after the preset was applied. The UDP listeners follow
    /// accepted edits; CITP and HTTP show the startup addresses until the next start.
    pub resolved: NetworkAddressesView,
    /// The port CITP discovery announces. Always the port CITP actually listens on.
    pub citp_advertised_port: u16,
    /// Some listeners are bound once, at startup: the fields named in `restartFields`. A change to
    /// them is stored and used by the next start; the API says so rather than letting a panel
    /// imply the change is already live.
    pub takes_effect_on_restart: bool,
    /// The fields that apply on the next start. Every other field applies immediately.
    pub restart_fields: Vec<String>,
    /// Whether a restart-bound listener would bind somewhere else on the next start.
    pub pending_restart: bool,
    /// A listener that could not bind this run is disabled rather than preventing administration.
    pub warnings: Vec<String>,
}

impl NetworkView {
    pub fn of(
        network: &NetworkConfiguration,
        active: &NetworkConfiguration,
        warnings: Vec<String>,
    ) -> Self {
        let startup = active.resolved();
        let next = network.resolved();
        // The UDP listeners already follow the stored settings; CITP and HTTP still run on what
        // this process started with.
        let running = ResolvedNetwork {
            citp_listen: startup.citp_listen,
            http_listen: startup.http_listen,
            citp_advertised_port: startup.citp_advertised_port,
            ..next.clone()
        };
        Self {
            same_computer_preset: network.same_computer_preset,
            stored: NetworkAddressesView::stored(network),
            active_same_computer_preset: active.same_computer_preset,
            active_stored: NetworkAddressesView::stored(active),
            resolved: NetworkAddressesView::resolved(&running),
            citp_advertised_port: running.citp_advertised_port,
            takes_effect_on_restart: true,
            restart_fields: RESTART_FIELDS
                .iter()
                .map(|field| (*field).to_owned())
                .collect(),
            pending_restart: next.citp_listen != startup.citp_listen
                || next.http_listen != startup.http_listen,
            warnings,
        }
    }
}

/// An intent-shaped network edit: only the fields being changed.
///
/// Stored configuration, so it carries a request id — a dropped response must never become a
/// second edit that moves a listener somewhere the operator did not ask for.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNetwork {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_computer_preset: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art_net_listen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sacn_listen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citp_listen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_listen: Option<String>,
    /// The Speed Group listen address. `null` or an empty string turns reception off; leaving
    /// the field out keeps it, which is why an absent field and an explicit null differ here.
    #[serde(
        default,
        deserialize_with = "explicit_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null")]
    pub speed_group_endpoint: Option<Option<String>>,
}

/// Distinguishes "the client sent null" from "the client left the field out".
fn explicit_null<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

/// Why a network edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkEditError {
    #[error("{field} is not an address and port, such as {example}")]
    Unparseable {
        field: &'static str,
        example: String,
    },
    #[error(
        "{field} needs a real port; port 0 asks the operating system to choose one, and a desk cannot find a server that moves"
    )]
    EphemeralPort { field: &'static str },
    #[error("{first} and {second} cannot share port {port}; they are both {protocol}")]
    PortCollision {
        first: &'static str,
        second: &'static str,
        port: u16,
        protocol: &'static str,
    },
}

impl UpdateNetwork {
    /// The configuration this edit describes, or why it was refused.
    ///
    /// Validation happens before anything is stored, so a refused edit leaves both the file and
    /// the running process exactly as they were.
    pub fn applied(
        &self,
        current: &NetworkConfiguration,
    ) -> Result<NetworkConfiguration, NetworkEditError> {
        let mut next = current.clone();
        if let Some(preset) = self.same_computer_preset {
            next.same_computer_preset = preset;
        }
        next.art_net_listen = listen(
            "artNetListen",
            self.art_net_listen.as_deref(),
            next.art_net_listen,
        )?;
        next.sacn_listen = listen("sacnListen", self.sacn_listen.as_deref(), next.sacn_listen)?;
        next.citp_listen = listen("citpListen", self.citp_listen.as_deref(), next.citp_listen)?;
        next.http_listen = listen("httpListen", self.http_listen.as_deref(), next.http_listen)?;

        if let Some(endpoint) = &self.speed_group_endpoint {
            next.speed_group_endpoint = match endpoint {
                None => None,
                Some(text) if text.trim().is_empty() => None,
                Some(text) => Some(listen(
                    "speedGroupEndpoint",
                    Some(text),
                    SocketAddr::from(([0, 0, 0, 0], SPEED_GROUP_PORT)),
                )?),
            };
        }
        if let Some(speed_groups) = next.speed_group_endpoint {
            for (field, other) in [
                ("artNetListen", next.art_net_listen),
                ("sacnListen", next.sacn_listen),
            ] {
                collision(field, other, "speedGroupEndpoint", speed_groups, "UDP")?;
            }
        }

        // Two listeners on one port fail at bind time. Catching it here means the operator is
        // told while they can still fix it, rather than on a start that half comes up.
        collision(
            "artNetListen",
            next.art_net_listen,
            "sacnListen",
            next.sacn_listen,
            "UDP",
        )?;
        collision(
            "citpListen",
            next.citp_listen,
            "httpListen",
            next.http_listen,
            "TCP",
        )?;
        Ok(next)
    }
}

/// A listen address: parsed, and refused if it names an ephemeral port.
fn listen(
    field: &'static str,
    edited: Option<&str>,
    current: SocketAddr,
) -> Result<SocketAddr, NetworkEditError> {
    let Some(text) = edited else {
        return Ok(current);
    };
    // The example an operator is shown keeps their own port, so the message reads as advice
    // rather than as a suggestion to move the listener somewhere else.
    let parsed = parse(field, text, format!("0.0.0.0:{}", current.port()))?;
    if parsed.port() == 0 {
        return Err(NetworkEditError::EphemeralPort { field });
    }
    Ok(parsed)
}

fn parse(field: &'static str, text: &str, example: String) -> Result<SocketAddr, NetworkEditError> {
    text.trim()
        .parse()
        .map_err(|_| NetworkEditError::Unparseable { field, example })
}

fn collision(
    first: &'static str,
    one: SocketAddr,
    second: &'static str,
    other: SocketAddr,
    protocol: &'static str,
) -> Result<(), NetworkEditError> {
    if one.port() == other.port() {
        return Err(NetworkEditError::PortCollision {
            first,
            second,
            port: one.port(),
            protocol,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(body: &str) -> UpdateNetwork {
        serde_json::from_str(body).expect("a network edit")
    }

    #[test]
    fn the_view_reports_what_was_typed_and_what_was_bound_separately() {
        let network = NetworkConfiguration {
            same_computer_preset: true,
            art_net_listen: "192.168.1.40:6454".parse().unwrap(),
            ..Default::default()
        };
        let view = NetworkView::of(&network, &network, Vec::new());

        assert_eq!(view.stored.art_net_listen, "192.168.1.40:6454");
        assert_eq!(
            view.resolved.art_net_listen, "127.0.0.1:6454",
            "the preset is visible without destroying the LAN setting"
        );
        assert_eq!(view.citp_advertised_port, 4809);
        assert!(view.takes_effect_on_restart);
        assert!(!view.pending_restart);
    }

    #[test]
    fn udp_listeners_apply_live_and_only_citp_and_http_wait_for_a_restart() {
        let active = NetworkConfiguration::default();
        let live = NetworkConfiguration {
            art_net_listen: "10.0.0.5:6454".parse().unwrap(),
            sacn_listen: "10.0.0.5:5568".parse().unwrap(),
            speed_group_endpoint: Some("10.0.0.5:4810".parse().unwrap()),
            ..active.clone()
        };
        let view = NetworkView::of(&live, &active, Vec::new());
        assert_eq!(view.resolved.art_net_listen, "10.0.0.5:6454");
        assert_eq!(view.resolved.sacn_listen, "10.0.0.5:5568");
        assert_eq!(
            view.resolved.speed_group_endpoint.as_deref(),
            Some("10.0.0.5:4810")
        );
        assert!(!view.pending_restart, "nothing here waits for a restart");
        assert_eq!(view.restart_fields, ["citpListen", "httpListen"]);

        let moved = NetworkConfiguration {
            citp_listen: "10.0.0.5:4809".parse().unwrap(),
            ..live.clone()
        };
        let view = NetworkView::of(&moved, &active, Vec::new());
        assert!(view.pending_restart);
        assert_eq!(
            view.resolved.citp_listen, "0.0.0.0:4809",
            "CITP keeps its startup socket until the next start"
        );

        let preset = NetworkConfiguration {
            same_computer_preset: true,
            ..live
        };
        let view = NetworkView::of(&preset, &active, Vec::new());
        assert_eq!(view.resolved.art_net_listen, "127.0.0.1:6454");
        assert!(
            view.pending_restart,
            "the preset moves CITP and HTTP only on the next start"
        );
    }

    #[test]
    fn an_edit_changes_only_the_addresses_it_carries() {
        let current = NetworkConfiguration::default();
        let next = edit(r#"{"requestId":"a","artNetListen":"10.0.0.5:6454"}"#)
            .applied(&current)
            .expect("accepted");

        assert_eq!(next.art_net_listen, "10.0.0.5:6454".parse().unwrap());
        assert_eq!(next.sacn_listen, current.sacn_listen);
        assert_eq!(next.http_listen, current.http_listen);
    }

    #[test]
    fn the_speed_group_listener_is_validated_like_every_other_listener() {
        let current = NetworkConfiguration::default();
        let wildcard = edit(r#"{"requestId":"a","speedGroupEndpoint":"0.0.0.0:4810"}"#)
            .applied(&current)
            .expect("every interface is a valid place to listen");
        assert_eq!(
            wildcard.speed_group_endpoint,
            Some("0.0.0.0:4810".parse().unwrap())
        );
        let empty = edit(r#"{"requestId":"b","speedGroupEndpoint":"  "}"#)
            .applied(&wildcard)
            .expect("an empty field turns reception off");
        assert_eq!(empty.speed_group_endpoint, None);

        let ephemeral = edit(r#"{"requestId":"c","speedGroupEndpoint":"0.0.0.0:0"}"#)
            .applied(&current)
            .expect_err("refused");
        assert_eq!(
            ephemeral,
            NetworkEditError::EphemeralPort {
                field: "speedGroupEndpoint"
            }
        );
        let garbage = edit(r#"{"requestId":"d","speedGroupEndpoint":"desk"}"#)
            .applied(&current)
            .expect_err("refused");
        assert!(garbage.to_string().contains("0.0.0.0:4810"), "{garbage}");
        let shared = edit(r#"{"requestId":"e","speedGroupEndpoint":"0.0.0.0:6454"}"#)
            .applied(&current)
            .expect_err("refused");
        assert!(matches!(shared, NetworkEditError::PortCollision { .. }));
    }

    #[test]
    fn the_speed_group_listener_can_be_set_and_cleared_but_omitting_it_keeps_it() {
        let mut current = NetworkConfiguration::default();
        let set = edit(r#"{"requestId":"a","speedGroupEndpoint":"192.168.1.9:9000"}"#)
            .applied(&current)
            .expect("accepted");
        assert_eq!(
            set.speed_group_endpoint,
            Some("192.168.1.9:9000".parse().unwrap())
        );

        current = set;
        let kept = edit(r#"{"requestId":"b","artNetListen":"0.0.0.0:6454"}"#)
            .applied(&current)
            .expect("accepted");
        assert_eq!(
            kept.speed_group_endpoint, current.speed_group_endpoint,
            "an absent field is not a cleared field"
        );

        let cleared = edit(r#"{"requestId":"c","speedGroupEndpoint":null}"#)
            .applied(&current)
            .expect("accepted");
        assert_eq!(cleared.speed_group_endpoint, None);
    }

    #[test]
    fn an_address_that_is_not_an_address_is_refused_by_name() {
        let error = edit(r#"{"requestId":"a","sacnListen":"the lighting network"}"#)
            .applied(&NetworkConfiguration::default())
            .expect_err("refused");
        assert_eq!(
            error,
            NetworkEditError::Unparseable {
                field: "sacnListen",
                example: "0.0.0.0:5568".to_owned()
            }
        );
        assert!(error.to_string().contains("0.0.0.0:5568"));
    }

    #[test]
    fn a_listener_may_not_ask_the_operating_system_to_choose_its_port() {
        let error = edit(r#"{"requestId":"a","citpListen":"0.0.0.0:0"}"#)
            .applied(&NetworkConfiguration::default())
            .expect_err("refused");
        assert_eq!(
            error,
            NetworkEditError::EphemeralPort {
                field: "citpListen"
            }
        );
    }

    #[test]
    fn two_listeners_of_one_protocol_may_not_share_a_port() {
        let error = edit(r#"{"requestId":"a","sacnListen":"0.0.0.0:6454"}"#)
            .applied(&NetworkConfiguration::default())
            .expect_err("refused");
        assert_eq!(
            error,
            NetworkEditError::PortCollision {
                first: "artNetListen",
                second: "sacnListen",
                port: 6454,
                protocol: "UDP",
            }
        );

        let tcp = edit(r#"{"requestId":"b","httpListen":"0.0.0.0:4809"}"#)
            .applied(&NetworkConfiguration::default())
            .expect_err("refused");
        assert!(tcp.to_string().contains("TCP"));
    }

    #[test]
    fn the_preset_is_a_setting_of_its_own_and_leaves_the_addresses_alone() {
        let current = NetworkConfiguration {
            art_net_listen: "192.168.1.40:6454".parse().unwrap(),
            ..Default::default()
        };
        let next = edit(r#"{"requestId":"a","sameComputerPreset":true}"#)
            .applied(&current)
            .expect("accepted");

        assert!(next.same_computer_preset);
        assert_eq!(next.art_net_listen, current.art_net_listen);
        assert_eq!(
            next.resolved().art_net_listen,
            "127.0.0.1:6454".parse().unwrap()
        );
    }
}
