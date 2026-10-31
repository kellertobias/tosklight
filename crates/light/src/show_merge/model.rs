use light_show::PortableShowObjectKey;
use serde_json::Value;
use std::collections::BTreeMap;

/// Every object of one show version, keyed by kind and id.
pub type MergeObjects = BTreeMap<PortableShowObjectKey, Value>;

/// The three versions a merge compares: the common ancestor, the show being merged into, and
/// the offline copy being merged in.
#[derive(Clone, Debug, Default)]
pub struct MergeInputs {
    pub base: MergeObjects,
    pub mine: MergeObjects,
    pub theirs: MergeObjects,
}

/// The operator's choice for one conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeChoice {
    Mine,
    Theirs,
}

/// Choices by conflict id. A merge applies only when every conflict has one.
pub type MergeResolutions = BTreeMap<String, MergeChoice>;

/// One place both sides changed differently.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeConflict {
    /// Stable across previews of the same inputs, so a resolution can name it.
    pub id: String,
    pub key: PortableShowObjectKey,
    /// Readable location inside the object, outermost first; empty for the whole object.
    pub path: Vec<String>,
    pub base: Option<Value>,
    pub mine: Option<Value>,
    pub theirs: Option<Value>,
}

/// An object both sides created under the same number. Theirs moves to a free number and every
/// reference the copy made to it follows.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeRenumber {
    pub from: PortableShowObjectKey,
    pub to: PortableShowObjectKey,
    pub name: Option<String>,
}

/// A Macro whose command text may still name a number the merge moved.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeMacroWarning {
    pub key: PortableShowObjectKey,
    pub name: String,
    pub mentions: Vec<String>,
}

/// What a merge does to one object kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeKindSummary {
    pub kind: String,
    pub added: usize,
    pub changed: usize,
    pub deleted: usize,
    pub conflicts: usize,
}

/// A computed merge: what it writes and what still needs a decision.
#[derive(Clone, Debug, Default)]
pub struct MergeOutcome {
    /// Objects whose merged body differs from mine; `None` deletes the object.
    pub writes: BTreeMap<PortableShowObjectKey, Option<Value>>,
    pub conflicts: Vec<MergeConflict>,
    pub renumbers: Vec<MergeRenumber>,
    pub macro_warnings: Vec<MergeMacroWarning>,
    pub summary: Vec<MergeKindSummary>,
    /// Objects that both sides changed and that merged without a conflict.
    pub auto_merged: usize,
}
