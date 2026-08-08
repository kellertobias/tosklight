#![forbid(unsafe_code)]

//! Media's CITP/MSEX server.
//!
//! A media server's job on CITP is to be found, to publish what it holds, and to say what its
//! layers are doing. This adapter owns the wire: framing, every message shape, and the version
//! differences between MSEX 1.0 and 1.1. What to publish comes from the caller.
//!
//! Light's CITP *client* lives in `crates/light/adapters/media` and stays there. A shared codec is
//! extracted only once both sides prove they need identical types — following the callers rather
//! than guessing at a shape neither has yet.

pub mod message;
pub mod packet;

pub use message::{
    ElementRequest, LayerStatus, LibraryElement, LibraryFolder, LibraryRequest, Presence,
    StreamRequest, Thumbnail, ThumbnailRequest, element_library_information,
    element_library_thumbnail, element_thumbnail, failure_text, layer_status,
    media_element_information, peer_location, read_element_request, read_element_thumbnail_request,
    read_library_request, read_library_thumbnail_request, read_stream_request, server_information,
    status_flags, stream_frame, video_sources,
};
pub use packet::{FramingError, Message, content, parse, take_messages};

/// The port CITP uses. Both the discovery socket and the request listener sit on it.
pub const CITP_PORT: u16 = 4809;
/// The TCP port a media server listens for MSEX requests on.
pub const MSEX_PORT: u16 = 4811;
/// The group discovery is announced to.
pub const MULTICAST_GROUP: [u8; 4] = [224, 0, 0, 180];
