//! Private, versioned IPC contract between ToskLight and supervised native extensions.
//!
//! This crate intentionally contains only data-transfer types, validation, and a bounded framed
//! JSON codec. Extension processes never receive engine objects or DMX frames.

mod codec;
mod model;
mod validation;

pub use codec::{CodecError, FrameDecoder, LENGTH_PREFIX_BYTES, MAX_FRAME_BYTES, encode_frame};
pub use model::*;
pub use validation::{
    HandshakeError, HandshakeExpectations, NegotiatedHandshake, negotiate, validate_capability,
    validate_control_input, validate_device_action_request, validate_device_action_result,
    validate_telemetry_sample, validate_timecode_sample,
};
