//! Wire encoders and standard destinations for supported output protocols.
//!
//! The encoders themselves live in `light-dmx-wire`, which the Media Server shares; this module
//! keeps the names the desk's delivery code already reaches for.

pub use light_dmx_wire::{
    ARTNET_PORT, SACN_PORT, artdmx_packet, artnet_broadcast_destination, sacn_data_packet,
    sacn_multicast_destination,
};
