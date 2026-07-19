//! Command-line transport adapters.
//!
//! The HTTP and OSC surfaces share the same application service while keeping routing, domain
//! adaptation, event publication, and wire conversion independently readable and testable.

#[path = "command_http/adapter.rs"]
mod adapter;
#[path = "command_http/events.rs"]
mod events;
#[path = "command_http/routes.rs"]
mod routes;
#[path = "command_http/state_event.rs"]
mod state_event;
#[path = "command_http/wire.rs"]
mod wire;

pub(super) use adapter::{
    ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command, route_osc_command_key,
};
pub(super) use routes::router;

#[cfg(test)]
use adapter::compatibility_only_family;
#[cfg(test)]
pub(super) use adapter::osc_command_key;

#[cfg(test)]
#[path = "command_http/unit_tests.rs"]
mod unit_tests;
