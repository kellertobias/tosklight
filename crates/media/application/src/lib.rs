#![forbid(unsafe_code)]

//! Media Server application layer.
//!
//! Commands, state transitions, control-source ownership, and use-case coordination live here.
//! Adapters translate external input into the commands this crate defines and translate its
//! results back out; nothing in here knows what a socket, a file handle, or a GPU is.

pub mod configuration;
pub mod gdtf;

pub use configuration::{
    ConfigurationDocument, ConfigurationError, MediaConfiguration, NetworkConfiguration,
    OutputConfiguration,
};
