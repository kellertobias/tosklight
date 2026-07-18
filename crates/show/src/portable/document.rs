use light_core::{Revision, ShowId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{cmp::Ordering, collections::BTreeMap, fmt};

/// Monotonic revision of the complete portable show document.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct PortableShowRevision(Revision);

impl PortableShowRevision {
    pub(crate) const fn new(value: Revision) -> Self {
        Self(value)
    }

    /// Returns the revision value used for optimistic concurrency checks.
    pub const fn value(self) -> Revision {
        self.0
    }
}

impl fmt::Display for PortableShowRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identity of one object inside a portable show.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PortableShowObjectKey {
    kind: String,
    id: String,
}

impl PortableShowObjectKey {
    /// Creates an object key without normalizing either persisted identifier.
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Raw versioned object whose JSON body retains fields unknown to this build.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableShowObject {
    key: PortableShowObjectKey,
    body: Value,
    revision: Revision,
    updated_at: String,
}

impl PortableShowObject {
    pub(crate) fn new(
        key: PortableShowObjectKey,
        body: Value,
        revision: Revision,
        updated_at: String,
    ) -> Self {
        Self {
            key,
            body,
            revision,
            updated_at,
        }
    }

    pub fn key(&self) -> &PortableShowObjectKey {
        &self.key
    }

    pub fn body(&self) -> &Value {
        &self.body
    }

    /// Mutates the retained raw JSON so unowned fields survive typed edits.
    pub fn body_mut(&mut self) -> &mut Value {
        &mut self.body
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }
}

/// Complete portable-show projection, including unrecognized metadata and objects.
#[derive(Clone, Debug, PartialEq)]
pub struct PortableShowDocument {
    id: ShowId,
    name: String,
    revision: PortableShowRevision,
    metadata: BTreeMap<String, String>,
    objects: Vec<PortableShowObject>,
}

impl PortableShowDocument {
    pub(crate) fn new(
        id: ShowId,
        name: String,
        revision: PortableShowRevision,
        metadata: BTreeMap<String, String>,
        mut objects: Vec<PortableShowObject>,
    ) -> Self {
        objects.sort_by(|left, right| left.key.cmp(&right.key));
        Self {
            id,
            name,
            revision,
            metadata,
            objects,
        }
    }

    pub const fn id(&self) -> ShowId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn revision(&self) -> PortableShowRevision {
        self.revision
    }

    /// Includes metadata keys this build does not own so callers can retain them.
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    pub fn objects(&self) -> impl ExactSizeIterator<Item = &PortableShowObject> {
        self.objects.iter()
    }

    pub fn objects_of_kind<'a>(
        &'a self,
        kind: &'a str,
    ) -> impl Iterator<Item = &'a PortableShowObject> + 'a {
        self.objects
            .iter()
            .filter(move |object| object.key.kind == kind)
    }

    pub fn object(&self, kind: &str, id: &str) -> Option<&PortableShowObject> {
        self.object_index(kind, id)
            .ok()
            .map(|index| &self.objects[index])
    }

    pub fn object_mut(&mut self, kind: &str, id: &str) -> Option<&mut PortableShowObject> {
        self.object_index(kind, id)
            .ok()
            .map(|index| &mut self.objects[index])
    }

    fn object_index(&self, kind: &str, id: &str) -> Result<usize, usize> {
        self.objects
            .binary_search_by(|object| compare_key(object.key.kind(), object.key.id(), kind, id))
    }
}

fn compare_key(left_kind: &str, left_id: &str, kind: &str, id: &str) -> Ordering {
    left_kind.cmp(kind).then_with(|| left_id.cmp(id))
}
