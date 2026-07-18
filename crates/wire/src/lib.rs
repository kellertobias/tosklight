//! Versioned transport contracts for ToskLight clients and adapters.
//!
//! These DTOs describe serialized boundaries only. Domain and application crates must not depend
//! on them; transport adapters translate between wire contracts and application commands.

pub mod generation;
pub mod v2;
