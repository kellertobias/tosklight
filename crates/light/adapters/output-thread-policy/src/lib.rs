//! Safe access to macOS requested QoS for the privately owned lighting output thread.
//!
//! The caller owns thread lifetime; this adapter neither creates threads nor changes process
//! policy. Headless only calls the policy setter from its dedicated output worker.
#![deny(unsafe_code)]

#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // Public Apple ABI is confined to this reviewed platform leaf.
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{USER_INTERACTIVE, request_interactive, requested_class};
