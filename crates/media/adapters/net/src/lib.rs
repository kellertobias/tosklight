#![forbid(unsafe_code)]

//! Art-Net and sACN ingress, and Speed Group reception.
//!
//! Both protocols parse here and then translate into the same domain frame through the canonical
//! personality, so identical payloads produce identical state. Neither adapter holds a copy of the
//! DMX mapping.

pub mod arbitration;
pub mod artnet;
pub mod ingress;
pub mod sacn;
pub mod speed_group_osc;
pub mod speed_group_reception;

pub use arbitration::{SourceArbiter, SourceKey, Winner};
pub use ingress::{ArtNetListener, IngressError, SacnListener, UniverseFrame};
pub use speed_group_osc::{SpeedGroupDecodeError, SpeedGroupListener};
pub use speed_group_reception::{
    SpeedGroupConnection, SpeedGroupReception, SpeedGroupReceptionStatus, SpeedGroupRejection,
    SpeedGroupUpdate,
};
