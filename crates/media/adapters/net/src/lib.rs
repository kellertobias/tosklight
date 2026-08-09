#![forbid(unsafe_code)]

//! Art-Net and sACN ingress.
//!
//! Both protocols parse here and then translate into the same domain frame through the canonical
//! personality, so identical payloads produce identical state. Neither adapter holds a copy of the
//! DMX mapping.

pub mod arbitration;
pub mod artnet;
pub mod ingress;
pub mod sacn;

pub use arbitration::{SourceArbiter, SourceKey, Winner};
pub use ingress::{ArtNetListener, IngressError, SacnListener, UniverseFrame};
