//! Compatibility re-exports for the portable-show codec's lossless typed merge operations.

pub use light_show::{apply_delta, merge_typed, merge_typed_request, strip_zero_u64_echo};

#[cfg(test)]
mod tests;
