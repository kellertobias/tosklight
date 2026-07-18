#![forbid(unsafe_code)]
//! DMX frame scheduling and production Art-Net 4 / ANSI E1.31 output.

pub mod codec;
pub mod delivery;
pub mod frame;
pub mod health;
pub mod route;
pub mod scheduler;

pub use codec::{
    ARTNET_PORT, SACN_PORT, artdmx_packet, artnet_broadcast_destination, sacn_data_packet,
    sacn_multicast_destination,
};
pub use delivery::{
    ArtNetDriver, EncodedPacket, NetworkOutput, OutputDriver, RouteDiagnostic, RouteSendError,
    SacnDriver, encode_routes, next_sequence,
};
pub use frame::{DMX_SLOTS, DmxFrame};
pub use health::OutputHealth;
pub use route::{DeliveryMode, OutputRoute, Protocol};
pub use scheduler::{run_scheduler, run_scheduler_dynamic};
