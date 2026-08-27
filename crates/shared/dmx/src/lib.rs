#![forbid(unsafe_code)]

//! The DMX wire formats, and the frame they carry.
//!
//! Art-Net and sACN are how both products in this workspace put light on a rig: the desk sends the
//! output of its playbacks, and the Media Server sends the pixels it maps off its canvas. They are
//! encoded once, here, so the two agree by construction rather than by two implementations
//! happening to match.
//!
//! This crate knows nothing about routes, schedulers or sockets. It turns a universe and its slots
//! into bytes, and names the address those bytes conventionally go to.

mod artnet;
mod sacn;

pub use artnet::{ARTNET_PORT, artdmx_packet, artnet_broadcast_destination};
pub use sacn::{SACN_PORT, sacn_data_packet, sacn_multicast_destination};

/// One universe's worth of slots.
pub const DMX_SLOTS: usize = 512;

/// A whole universe, always the full width.
///
/// How many of these slots a given route puts on the wire is the route's business; the frame
/// itself is never short.
pub type DmxFrame = [u8; DMX_SLOTS];
