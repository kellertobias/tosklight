//! Storage-neutral contracts for portable, revisioned managed assets.

use std::collections::HashSet;

use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetId(pub Uuid);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetRevision(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetReference {
    pub id: AssetId,
    pub revision: AssetRevision,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetNamespace(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetDescriptor {
    pub asset: AssetReference,
    pub name: String,
    pub media_type: String,
    pub length: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportAssetRequest {
    /// Re-importing with the same identity creates a new immutable revision.
    pub identity: Option<AssetId>,
    pub namespace: AssetNamespace,
    pub name: String,
    pub media_type: String,
    pub declared_length: u64,
    pub declared_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyAssetRequest {
    pub asset: AssetReference,
    pub destination: AssetNamespace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportAssetsRequest {
    pub assets: Vec<AssetReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupAssetsRequest {
    /// Only references owned by this namespace are considered for cleanup.
    pub namespace: AssetNamespace,
    pub retain: HashSet<AssetReference>,
    pub dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetValidation {
    pub descriptor: AssetDescriptor,
    pub valid: bool,
    pub problems: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetAvailability {
    Available(AssetDescriptor),
    Missing(AssetReference),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetStreamReport {
    pub asset: AssetReference,
    pub bytes_written: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetExportReport {
    pub manifest: AssetExportManifest,
    pub assets_written: usize,
    pub bytes_written: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetExportManifest {
    /// Ordered descriptors matching the frames written to the export sink.
    pub assets: Vec<AssetDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetCleanupReport {
    pub namespace: AssetNamespace,
    /// References detached from the requested namespace.
    pub detached: Vec<AssetReference>,
    /// Revisions whose bytes became globally unreferenced and were removed.
    pub removed: Vec<AssetReference>,
    pub retained: Vec<AssetReference>,
    pub bytes_reclaimed: u64,
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetErrorKind {
    Invalid,
    NotFound,
    Conflict,
    Unavailable,
    Io,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetError {
    pub kind: AssetErrorKind,
    pub message: String,
}

impl AssetError {
    pub fn new(kind: AssetErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub trait AssetChunkSource {
    fn read_chunk(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, AssetError>;
}

pub trait AssetChunkSink {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetError>;
}

/// A framed export destination. Asset boundaries and descriptors remain explicit even when
/// multiple revisions are written to one archive or stream.
pub trait AssetExportSink {
    fn begin_asset(&mut self, descriptor: &AssetDescriptor) -> Result<(), AssetError>;
    fn write_asset_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetError>;
    fn end_asset(&mut self, asset: AssetReference) -> Result<(), AssetError>;
}

pub trait ManagedAssetStore: Send + Sync {
    fn import(
        &self,
        request: ImportAssetRequest,
        source: &mut dyn AssetChunkSource,
    ) -> Result<AssetDescriptor, AssetError>;

    fn validate(&self, asset: AssetReference) -> Result<AssetValidation, AssetError>;

    fn stream(
        &self,
        asset: AssetReference,
        sink: &mut dyn AssetChunkSink,
    ) -> Result<AssetStreamReport, AssetError>;

    fn copy(&self, request: CopyAssetRequest) -> Result<AssetReference, AssetError>;

    fn export(
        &self,
        request: ExportAssetsRequest,
        sink: &mut dyn AssetExportSink,
    ) -> Result<AssetExportReport, AssetError>;

    fn availability(&self, asset: AssetReference) -> Result<AssetAvailability, AssetError>;

    fn revisions(&self, id: AssetId) -> Result<Vec<AssetDescriptor>, AssetError>;

    fn cleanup(&self, request: CleanupAssetsRequest) -> Result<AssetCleanupReport, AssetError>;
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
