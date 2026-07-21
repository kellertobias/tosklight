//! Command-line transport adapters.
//!
//! The HTTP and OSC surfaces share the same application service while keeping routing, domain
//! adaptation, event publication, and wire conversion independently readable and testable.

#[path = "command_http/adapter.rs"]
mod adapter;
#[path = "command_http/cue_recording_command.rs"]
mod cue_recording_command;
#[path = "command_http/cue_recording_environment.rs"]
mod cue_recording_environment;
#[path = "command_http/cue_recording_osc.rs"]
mod cue_recording_osc;
#[path = "command_http/cue_recording_ports.rs"]
mod cue_recording_ports;
#[path = "command_http/cue_recording_programming_ports.rs"]
mod cue_recording_programming_ports;
#[path = "command_http/cue_recording_routes.rs"]
mod cue_recording_routes;
#[path = "command_http/cue_recording_wire.rs"]
mod cue_recording_wire;
#[path = "command_http/cue_transfer_command.rs"]
mod cue_transfer_command;
#[path = "command_http/cue_transfer_ports.rs"]
mod cue_transfer_ports;
#[path = "command_http/cue_transfer_programming_ports.rs"]
mod cue_transfer_programming_ports;
#[path = "command_http/cue_transfer_routes.rs"]
mod cue_transfer_routes;
#[path = "command_http/cue_transfer_wire.rs"]
mod cue_transfer_wire;
#[path = "command_http/events.rs"]
mod events;
#[path = "command_http/group_recording_ports.rs"]
mod group_recording_ports;
#[path = "command_http/group_recording_routes.rs"]
mod group_recording_routes;
#[path = "command_http/group_recording_wire.rs"]
mod group_recording_wire;
#[path = "command_http/interaction_wire.rs"]
mod interaction_wire;
#[path = "command_http/lifecycle_routes.rs"]
mod lifecycle_routes;
#[path = "command_http/lifecycle_wire.rs"]
mod lifecycle_wire;
#[path = "command_http/preload_playback_queue_routes.rs"]
mod preload_playback_queue_routes;
#[path = "command_http/preload_playback_queue_wire.rs"]
mod preload_playback_queue_wire;
#[path = "command_http/preload_values_routes.rs"]
mod preload_values_routes;
#[path = "command_http/preload_values_wire.rs"]
mod preload_values_wire;
#[path = "command_http/preset_recording_ports.rs"]
mod preset_recording_ports;
#[path = "command_http/preset_recording_routes.rs"]
mod preset_recording_routes;
#[path = "command_http/preset_recording_wire.rs"]
mod preset_recording_wire;
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
#[path = "command_http/values_environment.rs"]
mod values_environment;
#[path = "command_http/values_routes.rs"]
mod values_routes;
#[path = "command_http/values_wire.rs"]
mod values_wire;
#[path = "command_http/wire.rs"]
mod wire;

pub(super) use adapter::{
    ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command, route_osc_command_key,
};
pub(crate) use cue_recording_osc::intercept_armed_playback as intercept_armed_cue_playback;
pub(crate) use cue_transfer_ports::ServerProgrammingCueTransferPorts;
pub(super) use interaction_wire::interaction_change;
pub(super) use lifecycle_wire::lifecycle_change;
pub(super) use preload_playback_queue_wire::change as preload_playback_queue_change;
pub(super) use preload_values_wire::change as preload_values_change;
pub(crate) use programming_ports::ServerProgrammingPorts;
pub(super) use routes::router;
pub(super) use values_wire::{capture_mode_change, values_change};
pub(super) use wire::wire_choice;

#[cfg(test)]
use adapter::compatibility_only_family;
#[cfg(test)]
pub(super) use adapter::osc_command_key;

#[cfg(test)]
#[path = "command_http/unit_tests.rs"]
mod unit_tests;
