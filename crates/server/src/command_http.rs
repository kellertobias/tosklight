//! Command-line transport adapters.
//!
//! The HTTP and OSC surfaces share the same application service while keeping routing, domain
//! adaptation, event publication, and wire conversion independently readable and testable.

#[path = "command_http/adapter.rs"]
mod adapter;
#[path = "command_http/events.rs"]
mod events;
#[path = "command_http/interaction_wire.rs"]
mod interaction_wire;
#[path = "command_http/programming_ports.rs"]
mod programming_ports;
#[path = "command_http/routes.rs"]
mod routes;
#[path = "command_http/selection_environment.rs"]
mod selection_environment;
#[path = "command_http/selection_routes.rs"]
mod selection_routes;
#[path = "command_http/selection_wire.rs"]
mod selection_wire;
#[path = "command_http/state_event.rs"]
mod state_event;
#[path = "command_http/values_wire.rs"]
mod values_wire;
#[path = "command_http/wire.rs"]
mod wire;

pub(super) use adapter::{
    ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command, route_osc_command_key,
};
pub(super) use interaction_wire::interaction_change;
pub(crate) use programming_ports::ServerProgrammingPorts;
pub(super) use routes::router;
pub(super) use values_wire::values_change;

#[cfg(test)]
use adapter::compatibility_only_family;
#[cfg(test)]
pub(super) use adapter::osc_command_key;

#[cfg(test)]
#[path = "command_http/unit_tests.rs"]
mod unit_tests;
