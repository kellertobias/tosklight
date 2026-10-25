//! Private ephemeral content identities, never persisted, transmitted or used for security.
//! FxHasher is usize-based: retain DefaultHasher's full 64-bit entropy on 32-bit targets.
#[cfg(target_pointer_width = "64")]
pub(super) use rustc_hash::FxHasher as RuntimeHasher;
#[cfg(not(target_pointer_width = "64"))]
pub(super) use std::collections::hash_map::DefaultHasher as RuntimeHasher;
