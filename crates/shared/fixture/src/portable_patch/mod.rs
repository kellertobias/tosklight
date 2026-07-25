mod codec;
mod compiler;
mod digest;
mod identity;
mod legacy;
mod merge;
mod model;

pub use compiler::{
    FixtureProfileRevisionResolver, PatchedFixtureCompiler, ResolvedFixtureProfileRevision,
};
pub use digest::fixture_profile_content_digest;
pub use model::{
    PORTABLE_PATCH_RECORD_SCHEMA_VERSION, PatchedFixturePatch, PatchedFixtureProfileReference,
    PortablePatchError, PortablePatchedFixtureRecord, RETAINED_LEGACY_DEFINITION_FIELDS,
};

#[cfg(test)]
mod tests;
