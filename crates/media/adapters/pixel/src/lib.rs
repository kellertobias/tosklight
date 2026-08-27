#![forbid(unsafe_code)]

//! Sending what the Media Server maps.
//!
//! The domain decides which slots a canvas turns into; this crate puts those slots on a socket.
//! It owns the sequence numbers each universe carries and the sockets each protocol needs, and
//! nothing else: the frames arrive already built.

mod sender;

pub use sender::{PixelSendError, PixelSender, RouteDestination, route_destination};
