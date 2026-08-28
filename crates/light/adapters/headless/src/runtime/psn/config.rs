//! The tracking configuration, as the runtime reads it.
//!
//! The types themselves are show data and live with the other show-object families in
//! `light-application`; what is here is the desk-side view of them, plus the one constant that
//! ties a written position to the value a 3D Point is read back with.

pub(in crate::runtime) use light_application::PsnConfiguration;
#[cfg(test)]
pub(in crate::runtime) use light_application::{PsnBinding, PsnCalibration, PsnZone};

/// How far a stored 3D Point offset can reach along one axis, in metres.
///
/// The same number the resolved value is read back with, so that writing a position and reading it
/// produce the same metres. It lives beside the reader in `programmer_aim_command` and is
/// re-exported rather than copied.
pub(in crate::runtime) use super::super::programmer_aim_command::POINT_AXIS_METRES;

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
