#![forbid(unsafe_code)]
//! Art-Net and sACN input for the Viz renderer.
//!
//! Decoding is pure and testable; receiving is bounded, non-blocking relative to rendering, and
//! coalesced to the newest complete frame per logical universe. Nothing here can backpressure the
//! desk that sent the packets.

mod mapping;
mod packet;
mod receiver;
mod statistics;

pub use mapping::{Delivery, InputMapping, Protocol, UniverseInput, apply_overrides};
pub use packet::{
    ARTNET_PORT, DMX_SLOTS, DecodedFrame, PacketReject, SACN_PORT, decode_artdmx, decode_sacn,
    sacn_multicast_group, sequence_is_stale,
};
pub use receiver::{DmxReceiver, UniverseFrame};
pub use statistics::{CRITICAL_HZ, DEGRADED_HZ, UniverseStatistics, WINDOW_SECONDS};
