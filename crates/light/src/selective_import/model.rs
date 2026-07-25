use crate::{ActionContext, ApplicationCommand, AssetReference, CommandFamily};
use light_core::{FixtureId, Revision, ShowId};
use light_show::{PortableShowObjectKey, PortableShowRevision};
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

/// Operator choices for a source object whose stable key is already occupied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportConflictResolution {
    KeepDestination,
    ReplaceDestination,
    Duplicate,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportProfileKey {
    pub profile_id: FixtureId,
    pub revision: Revision,
}

impl Ord for ImportProfileKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.profile_id
            .0
            .as_bytes()
            .cmp(other.profile_id.0.as_bytes())
            .then_with(|| self.revision.cmp(&other.revision))
    }
}

impl PartialOrd for ImportProfileKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Immutable profile conflicts can either bind explicitly to the destination or copy safely under
/// a new profile identity. Replacing an immutable revision is intentionally unsupported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportProfileConflictResolution {
    KeepDestination,
    Duplicate,
}

/// Complete input needed to build a repeatable selective-import preview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveShowImportRequest {
    pub source_show_id: ShowId,
    pub target_show_id: ShowId,
    pub selected_objects: BTreeSet<PortableShowObjectKey>,
    pub conflict_resolutions: BTreeMap<PortableShowObjectKey, ImportConflictResolution>,
    pub profile_conflict_resolutions: BTreeMap<ImportProfileKey, ImportProfileConflictResolution>,
}

impl SelectiveShowImportRequest {
    pub fn new(
        source_show_id: ShowId,
        target_show_id: ShowId,
        selected_objects: impl IntoIterator<Item = PortableShowObjectKey>,
    ) -> Self {
        Self {
            source_show_id,
            target_show_id,
            selected_objects: selected_objects.into_iter().collect(),
            conflict_resolutions: BTreeMap::new(),
            profile_conflict_resolutions: BTreeMap::new(),
        }
    }

    pub fn resolve(
        mut self,
        key: PortableShowObjectKey,
        resolution: ImportConflictResolution,
    ) -> Self {
        self.conflict_resolutions.insert(key, resolution);
        self
    }

    pub fn resolve_profile(
        mut self,
        key: ImportProfileKey,
        resolution: ImportProfileConflictResolution,
    ) -> Self {
        self.profile_conflict_resolutions.insert(key, resolution);
        self
    }
}

/// A precise JSON location owned by a typed object descriptor.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ImportReferenceLocation {
    Value {
        pointer: String,
        format: ImportIdentityFormat,
    },
    ObjectKey {
        object_pointer: String,
        key: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ImportIdentityFormat {
    Full,
    NumericSuffix,
}

/// One identity owned by an object. `slot` is semantic within the object, allowing references to a
/// logical head to bind to the corresponding destination head when the parent fixture is kept.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImportOwnedIdentity {
    pub slot: String,
    pub value: String,
    pub location: Option<ImportReferenceLocation>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ImportObjectReference {
    pub target: PortableShowObjectKey,
    pub target_slot: String,
    pub source_identity: String,
    pub location: ImportReferenceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportProfileReference {
    pub key: ImportProfileKey,
    pub id_locations: Vec<ImportReferenceLocation>,
    /// Legacy inline fixture definitions carry the complete immutable snapshot in their body.
    pub inline_profile: Option<Value>,
}

/// Typed import metadata for one losslessly retained raw object body.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImportObjectDescriptor {
    pub identities: Vec<ImportOwnedIdentity>,
    pub references: Vec<ImportObjectReference>,
    pub profile_references: Vec<ImportProfileReference>,
    pub managed_assets: Vec<AssetReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportObjectAction {
    ImportPreservingId,
    SkipIdentical,
    KeepDestination,
    ReplaceDestination,
    Duplicate { destination: PortableShowObjectKey },
    BlockedConflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedImportObject {
    pub source: PortableShowObjectKey,
    pub destination: PortableShowObjectKey,
    pub action: ImportObjectAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportDependencyDisposition {
    Selected,
    Included,
    BoundToDestination,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDependency {
    pub owner: PortableShowObjectKey,
    pub dependency: PortableShowObjectKey,
    pub disposition: ImportDependencyDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportConflict {
    pub key: PortableShowObjectKey,
    pub resolution: Option<ImportConflictResolution>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportProfileAction {
    Copy,
    SkipIdentical,
    KeepDestination,
    Duplicate { destination: ImportProfileKey },
    BlockedConflict,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportProfilePreview {
    pub source: ImportProfileKey,
    pub destination: ImportProfileKey,
    pub action: ImportProfileAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportManagedAssetAction {
    Copy,
    SkipIdentical,
    Missing,
    BlockedConflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportManagedAssetPreview {
    pub asset: AssetReference,
    pub action: ImportManagedAssetAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportBlocker {
    EmptySelection,
    SameShow,
    UnsupportedObject {
        key: PortableShowObjectKey,
    },
    MissingObject {
        key: PortableShowObjectKey,
        required_by: Option<PortableShowObjectKey>,
    },
    ObjectConflict {
        key: PortableShowObjectKey,
    },
    InvalidResolution {
        key: PortableShowObjectKey,
        message: String,
    },
    InvalidProfileResolution {
        key: ImportProfileKey,
        message: String,
    },
    InvalidDescriptor {
        key: PortableShowObjectKey,
        message: String,
    },
    MissingProfile {
        key: ImportProfileKey,
        required_by: PortableShowObjectKey,
    },
    ProfileConflict {
        key: ImportProfileKey,
    },
    MissingManagedAsset {
        asset: AssetReference,
    },
    ManagedAssetConflict {
        asset: AssetReference,
    },
    ReferenceRewrite {
        owner: PortableShowObjectKey,
        message: String,
    },
    CandidateInvalid {
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveShowImportPreview {
    pub request: SelectiveShowImportRequest,
    pub source_revision: PortableShowRevision,
    pub target_revision: PortableShowRevision,
    pub objects: Vec<AppliedImportObject>,
    pub dependencies: Vec<ImportDependency>,
    pub conflicts: Vec<ImportConflict>,
    pub profiles: Vec<ImportProfilePreview>,
    pub managed_assets: Vec<ImportManagedAssetPreview>,
    pub blockers: Vec<ImportBlocker>,
}

impl SelectiveShowImportPreview {
    pub fn can_apply(&self) -> bool {
        self.blockers.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplySelectiveShowImportCommand {
    pub request: SelectiveShowImportRequest,
    pub expected_source_revision: PortableShowRevision,
    pub expected_target_revision: PortableShowRevision,
}

impl ApplicationCommand for ApplySelectiveShowImportCommand {
    type Value = SelectiveShowImportResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveShowObjectChange {
    pub key: PortableShowObjectKey,
    pub object_revision: Revision,
    pub body: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveShowProfileChange {
    pub source: ImportProfileKey,
    pub destination: ImportProfileKey,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveShowImportChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub outcomes: Vec<AppliedImportObject>,
    /// Exact committed bodies and object revisions, including compatibility migrations that
    /// joined the active-show transaction.
    pub objects: Vec<SelectiveShowObjectChange>,
    pub profiles: Vec<SelectiveShowProfileChange>,
    pub managed_assets: Vec<AssetReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveShowImportResult {
    pub context: ActionContext,
    pub changed: bool,
    pub change: SelectiveShowImportChange,
    pub event_sequence: Option<u64>,
}

#[derive(Clone)]
pub(super) struct PlannedWrite {
    pub destination: PortableShowObjectKey,
    pub body: Value,
}
