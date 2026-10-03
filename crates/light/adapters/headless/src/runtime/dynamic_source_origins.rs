//! Exact source assignments retained by Dynamic expressions. This is a cold, copy-on-write
//! catalogue: sampling reads IDs and immutable records; only assignment changes allocate IDs.
use chrono::{DateTime, Utc};
use light_core::{
    CueListId, FixtureId, ProgrammerId,
    programming::{IntentError, ProgrammingComponent, ProgrammingFieldScope, ProgrammingOwner},
};
use light_dynamics::DynamicSourceOccurrenceId;
use light_playback::{PlaybackIdentity, SequenceMasterSource, TemporaryPlaybackKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use uuid::Uuid;

const SNAPSHOT_VERSION: u16 = 1;

mod checkpoint;
pub(super) use checkpoint::DynamicRuntimeSourceCheckpoint;
mod projection;
pub(super) use projection::*;
mod fixed;
pub(super) use fixed::*;
mod watermark;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum DynamicSourceBinding {
    Authored {
        instance_id: Uuid,
        controller_id: Uuid,
        target: FixtureId,
        lane_id: Uuid,
    },
    /// A fixed row has a real source and address, but no runtime Dynamic scope.
    Fixed {
        source: DynamicFixedSource,
        target: FixtureId,
        owner: ProgrammingOwner,
        component: Option<ProgrammingComponent>,
    },
    /// The common pre-Dynamic family may be read by many lanes. It has no fabricated Dynamic
    /// instance, controller or lane; its original component evidence remains in the record.
    StaticBaseline {
        target: FixtureId,
        owner: ProgrammingOwner,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DynamicProgrammerSourceLane {
    Live,
    Preload,
}

/// A serializable copy of real Playback master ownership, without changing the Playback schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub(super) struct DynamicSequenceSource {
    pub playback_number: Option<u16>,
    #[serde(with = "playback_identity_wire::optional")]
    pub playback_identity: Option<PlaybackIdentity>,
    pub cue_list_id: CueListId,
    pub temporary: bool,
}

impl From<SequenceMasterSource> for DynamicSequenceSource {
    fn from(source: SequenceMasterSource) -> Self {
        Self {
            playback_number: source.playback_number,
            playback_identity: source.playback_identity,
            cue_list_id: source.cue_list_id,
            temporary: source.temporary,
        }
    }
}

impl DynamicSequenceSource {
    pub fn sequence_master_source(self) -> SequenceMasterSource {
        SequenceMasterSource {
            playback_number: self.playback_number,
            playback_identity: self.playback_identity,
            cue_list_id: self.cue_list_id,
            temporary: self.temporary,
        }
    }

    fn validate(self) -> Result<(), IntentError> {
        non_nil(self.cue_list_id.0, "Cue List")?;
        if let Some(number) = self.playback_number {
            if number == 0 || number > light_playback::MAX_VIRTUAL_PLAYBACK {
                return Err(invalid("Playback number is outside its domain"));
            }
        }
        if let Some(identity) = self.playback_identity {
            validate_playback_identity(identity)?;
            if self
                .playback_number
                .is_some_and(|number| number != identity.number())
            {
                return Err(invalid("Playback number and identity disagree"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DynamicTemporarySourceKind {
    Flash,
    TempButton,
    TempFader,
    Swap,
}

impl From<TemporaryPlaybackKind> for DynamicTemporarySourceKind {
    fn from(kind: TemporaryPlaybackKind) -> Self {
        match kind {
            TemporaryPlaybackKind::Flash => Self::Flash,
            TemporaryPlaybackKind::TempButton => Self::TempButton,
            TemporaryPlaybackKind::TempFader => Self::TempFader,
            TemporaryPlaybackKind::Swap => Self::Swap,
        }
    }
}

impl DynamicTemporarySourceKind {
    pub fn playback_kind(self) -> TemporaryPlaybackKind {
        match self {
            Self::Flash => TemporaryPlaybackKind::Flash,
            Self::TempButton => TemporaryPlaybackKind::TempButton,
            Self::TempFader => TemporaryPlaybackKind::TempFader,
            Self::Swap => TemporaryPlaybackKind::Swap,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum DynamicSourceOrigin {
    Programmer {
        programmer_id: ProgrammerId,
        lane: DynamicProgrammerSourceLane,
        instance_link: Uuid,
        /// The raw persisted millisecond stamp, not an invented higher-precision instant.
        changed_at_millis: u64,
        programmer_order: u64,
    },
    Cue {
        source: DynamicSequenceSource,
        temporary_kind: Option<DynamicTemporarySourceKind>,
        cue_id: Uuid,
        instance_link: Uuid,
        changed_at: DateTime<Utc>,
        transition_ordinal: u64,
    },
    Playback {
        #[serde(with = "playback_identity_wire")]
        identity: PlaybackIdentity,
        activated_at: DateTime<Utc>,
    },
    /// Preserve the original fixed row, including its complete payload and timing. This is
    /// neither pre-Dynamic Current nor an invented Dynamic On assignment.
    Fixed {
        stamp: DynamicFixedStamp,
        priority: i16,
        value: light_dynamics::DynamicSemanticValue,
    },
    /// Explicit engine evidence supplied by the caller. An absent Current is not an empty or
    /// guessed origin; callers omit its binding. Dependencies never become authored writes.
    StaticBaseline {
        sources: Vec<DynamicStaticSourceEntry>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct DynamicStaticSourceEntry {
    pub source: DynamicStaticSource,
    pub changed_at: DateTime<Utc>,
    pub programmer_order: u64,
    /// Exact Playback source occurrence order, independent of current LTP/transport order.
    /// Legacy evidence without it remains explicitly unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_ordinal: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_cue_id: Option<Uuid>,
    pub footprint: DynamicStaticFootprint,
    pub role: DynamicStaticRole,
    /// None preserves the original footprint from older records. Explicit scopes retain the
    /// fields affected after a fade/conversion without rewriting what the operator authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_fields: Option<ProgrammingFieldScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "type", content = "component", rename_all = "snake_case")]
pub(super) enum DynamicStaticFootprint {
    Whole,
    Component(ProgrammingComponent),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DynamicStaticRole {
    Authored,
    CalculationDependency,
}

impl DynamicStaticSourceEntry {
    /// The caller must identify the observed source explicitly; opaque engine identities are
    /// checked for equality rather than guessed from the current controller or family rank.
    pub fn from_evidence(
        source: DynamicStaticSource,
        entry: &light_engine::ContributionFamilyEntry,
    ) -> Result<Self, IntentError> {
        if !source.matches_contribution(entry.source()) {
            return Err(invalid(
                "static source does not match the observed evidence",
            ));
        }
        let stamp = entry.stamp();
        Ok(Self {
            source,
            changed_at: stamp.changed_at,
            programmer_order: stamp.programmer_order,
            transition_ordinal: entry.transition_ordinal(),
            authored_cue_id: entry.authored_cue_id(),
            footprint: match entry.footprint() {
                light_engine::ContributionFamilyFootprint::Whole => DynamicStaticFootprint::Whole,
                light_engine::ContributionFamilyFootprint::Component(component) => {
                    DynamicStaticFootprint::Component(component)
                }
            },
            role: match entry.role() {
                light_engine::ContributionFamilyRole::Authored => DynamicStaticRole::Authored,
                light_engine::ContributionFamilyRole::CalculationDependency => {
                    DynamicStaticRole::CalculationDependency
                }
            },
            effective_fields: entry.effective_fields().cloned(),
        })
    }

    pub fn family_entry(&self) -> light_engine::ContributionFamilyEntry {
        let entry = light_engine::ContributionFamilyEntry::new(
            self.source.contribution_source(),
            light_core::ProgrammerEditStamp {
                changed_at: self.changed_at,
                programmer_order: self.programmer_order,
            },
            match self.footprint {
                DynamicStaticFootprint::Whole => light_engine::ContributionFamilyFootprint::Whole,
                DynamicStaticFootprint::Component(component) => {
                    light_engine::ContributionFamilyFootprint::Component(component)
                }
            },
            match self.role {
                DynamicStaticRole::Authored => light_engine::ContributionFamilyRole::Authored,
                DynamicStaticRole::CalculationDependency => {
                    light_engine::ContributionFamilyRole::CalculationDependency
                }
            },
        )
        .with_transition_ordinal(self.transition_ordinal)
        .with_authored_cue_id(self.authored_cue_id);
        match &self.effective_fields {
            Some(fields) => entry.with_effective_fields(fields.clone()),
            None => entry,
        }
    }

    /// Common unchanged Current queries compare exact metadata without allocating a new
    /// source name or evidence vector. Equality of output values is deliberately irrelevant.
    pub fn matches_evidence(&self, entry: &light_engine::ContributionFamilyEntry) -> bool {
        self.source.matches_contribution(entry.source())
            && self.changed_at == entry.stamp().changed_at
            && self.programmer_order == entry.stamp().programmer_order
            && self.transition_ordinal == entry.transition_ordinal()
            && self.authored_cue_id == entry.authored_cue_id()
            && self.effective_fields.as_ref() == entry.effective_fields()
            && self.footprint
                == match entry.footprint() {
                    light_engine::ContributionFamilyFootprint::Whole => {
                        DynamicStaticFootprint::Whole
                    }
                    light_engine::ContributionFamilyFootprint::Component(component) => {
                        DynamicStaticFootprint::Component(component)
                    }
                }
            && self.role
                == match entry.role() {
                    light_engine::ContributionFamilyRole::Authored => DynamicStaticRole::Authored,
                    light_engine::ContributionFamilyRole::CalculationDependency => {
                        DynamicStaticRole::CalculationDependency
                    }
                }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum DynamicStaticSource {
    Programmer {
        programmer_id: ProgrammerId,
        lane: DynamicStaticProgrammerLane,
    },
    Playback {
        source: DynamicSequenceSource,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "source", rename_all = "snake_case")]
pub(super) enum DynamicStaticProgrammerLane {
    Live,
    Preload,
    Transient(String),
    Group(String),
    PreloadGroup(String),
}

impl DynamicStaticSource {
    fn matches_contribution(&self, source: &light_engine::ContributionSourceId) -> bool {
        use light_engine::{
            ContributionProgrammerLane as Lane, ContributionSourceDescriptor as Source,
        };
        match (self, source.descriptor()) {
            (
                Self::Programmer {
                    programmer_id,
                    lane,
                },
                Source::Programmer {
                    programmer_id: candidate,
                    lane: candidate_lane,
                },
            ) => {
                *programmer_id == candidate
                    && match (lane, candidate_lane) {
                        (DynamicStaticProgrammerLane::Live, Lane::Live)
                        | (DynamicStaticProgrammerLane::Preload, Lane::Preload) => true,
                        (DynamicStaticProgrammerLane::Transient(a), Lane::Transient(b))
                        | (DynamicStaticProgrammerLane::Group(a), Lane::Group(b))
                        | (DynamicStaticProgrammerLane::PreloadGroup(a), Lane::PreloadGroup(b)) => {
                            a == b
                        }
                        _ => false,
                    }
            }
            (Self::Playback { source }, Source::Playback(candidate)) => {
                source.sequence_master_source() == candidate
            }
            _ => false,
        }
    }

    /// Lossless engine-owned identity inspection. Never infer a source by matching its value
    /// against live Programmer or Playback state from a different capture.
    pub fn from_contribution(source: &light_engine::ContributionSourceId) -> Self {
        use light_engine::{
            ContributionProgrammerLane as Lane, ContributionSourceDescriptor as Source,
        };
        match source.descriptor() {
            Source::Programmer {
                programmer_id,
                lane,
            } => Self::Programmer {
                programmer_id,
                lane: match lane {
                    Lane::Live => DynamicStaticProgrammerLane::Live,
                    Lane::Preload => DynamicStaticProgrammerLane::Preload,
                    Lane::Transient(source) => {
                        DynamicStaticProgrammerLane::Transient(source.into())
                    }
                    Lane::Group(source) => DynamicStaticProgrammerLane::Group(source.into()),
                    Lane::PreloadGroup(source) => {
                        DynamicStaticProgrammerLane::PreloadGroup(source.into())
                    }
                },
            },
            Source::Playback(source) => Self::Playback {
                source: source.into(),
            },
        }
    }

    pub fn contribution_source(&self) -> light_engine::ContributionSourceId {
        use light_engine::ContributionSourceId as Source;
        match self {
            Self::Programmer {
                programmer_id,
                lane,
            } => match lane {
                DynamicStaticProgrammerLane::Live => Source::programmer(*programmer_id),
                DynamicStaticProgrammerLane::Preload => Source::preload(*programmer_id),
                DynamicStaticProgrammerLane::Transient(name) => {
                    Source::programmer_transient(*programmer_id, name.as_str())
                }
                DynamicStaticProgrammerLane::Group(name) => {
                    Source::programmer_group(*programmer_id, name.as_str())
                }
                DynamicStaticProgrammerLane::PreloadGroup(name) => {
                    Source::preload_group(*programmer_id, name.as_str())
                }
            },
            Self::Playback { source } => Source::playback(source.sequence_master_source()),
        }
    }
}

impl DynamicSourceOrigin {
    pub fn authored_controller_id(&self) -> Option<Uuid> {
        match self {
            Self::Programmer {
                programmer_id,
                instance_link,
                ..
            } => Some(light_dynamics::programmer_dynamic_controller_id(
                *programmer_id,
                *instance_link,
            )),
            Self::Cue {
                source,
                temporary_kind,
                instance_link,
                ..
            } => {
                let source = source.sequence_master_source();
                let key = match temporary_kind {
                    Some(kind) => light_playback::CueDynamicSourceKey::Temporary {
                        source,
                        kind: kind.playback_kind(),
                    },
                    None => light_playback::CueDynamicSourceKey::Normal { source },
                };
                Some(key.controller_id(*instance_link))
            }
            Self::Playback { .. } | Self::StaticBaseline { .. } | Self::Fixed { .. } => None,
        }
    }

    fn validate_controller(&self, binding: DynamicSourceBinding) -> Result<(), IntentError> {
        if let DynamicSourceBinding::Authored { controller_id, .. } = binding
            && self
                .authored_controller_id()
                .is_some_and(|actual| actual != controller_id)
        {
            return Err(invalid(
                "authored source and scoped controller identity disagree",
            ));
        }
        Ok(())
    }

    fn canonicalize(&mut self) {
        if let Self::StaticBaseline { sources } = self {
            // Engine evidence is a set. Hash iteration order must not create new occurrences.
            sources.sort_by(|left, right| {
                compare_static_sources(&left.source, &right.source)
                    .then_with(|| left.changed_at.cmp(&right.changed_at))
                    .then_with(|| left.programmer_order.cmp(&right.programmer_order))
                    .then_with(|| left.transition_ordinal.cmp(&right.transition_ordinal))
                    .then_with(|| left.authored_cue_id.cmp(&right.authored_cue_id))
                    .then_with(|| left.footprint.cmp(&right.footprint))
                    .then_with(|| left.role.cmp(&right.role))
                    .then_with(|| left.effective_fields.cmp(&right.effective_fields))
            });
            sources.dedup();
        }
    }

    fn validate(&self, binding: DynamicSourceBinding) -> Result<(), IntentError> {
        if matches!(self, Self::StaticBaseline { .. })
            != matches!(binding, DynamicSourceBinding::StaticBaseline { .. })
        {
            return Err(invalid(
                "authored source and static dependency roles cannot be interchanged",
            ));
        }
        if matches!(self, Self::Fixed { .. })
            != matches!(binding, DynamicSourceBinding::Fixed { .. })
        {
            return Err(invalid(
                "fixed and Dynamic authored sources cannot be interchanged",
            ));
        }
        match self {
            Self::Fixed { stamp, value, .. } => validate_fixed_origin(binding, stamp, value)?,
            Self::Programmer {
                programmer_id,
                instance_link,
                ..
            } => {
                non_nil(programmer_id.0, "Programmer")?;
                non_nil(*instance_link, "authored Dynamic link")?;
            }
            Self::Cue {
                source,
                temporary_kind,
                cue_id,
                instance_link,
                ..
            } => {
                source.validate()?;
                if source.temporary != temporary_kind.is_some() {
                    return Err(invalid(
                        "Cue temporary ownership and temporary kind disagree",
                    ));
                }
                non_nil(*cue_id, "Cue")?;
                non_nil(*instance_link, "authored Dynamic link")?;
            }
            Self::Playback { identity, .. } => validate_playback_identity(*identity)?,
            Self::StaticBaseline { sources } => {
                if sources.is_empty() {
                    return Err(invalid("static dependency requires actual source evidence"));
                }
                for entry in sources {
                    if let Some(cue_id) = entry.authored_cue_id {
                        non_nil(cue_id, "authored Cue")?;
                        if !matches!(entry.source, DynamicStaticSource::Playback { .. }) {
                            return Err(invalid("only Playback evidence can name an authored Cue"));
                        }
                    }
                    if let DynamicSourceBinding::StaticBaseline { owner, .. } = binding
                        && let Some(fields) = &entry.effective_fields
                    {
                        fields.validate(owner)?;
                    }
                    if let (
                        DynamicSourceBinding::StaticBaseline { owner, .. },
                        DynamicStaticFootprint::Component(component),
                    ) = (binding, entry.footprint)
                        && component.owner() != owner
                    {
                        return Err(invalid(
                            "static component evidence belongs to a different family",
                        ));
                    }
                    match &entry.source {
                        DynamicStaticSource::Programmer {
                            programmer_id,
                            lane,
                        } => {
                            non_nil(programmer_id.0, "Programmer")?;
                            match lane {
                                DynamicStaticProgrammerLane::Transient(name)
                                | DynamicStaticProgrammerLane::Group(name)
                                | DynamicStaticProgrammerLane::PreloadGroup(name)
                                    if name.is_empty() =>
                                {
                                    return Err(invalid("static Programmer source name is empty"));
                                }
                                _ => {}
                            }
                        }
                        DynamicStaticSource::Playback { source } => source.validate()?,
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct DynamicSourceRecord {
    pub occurrence_id: DynamicSourceOccurrenceId,
    /// Retain the original scope even after the active binding selects a newer assignment.
    pub binding: DynamicSourceBinding,
    pub origin: DynamicSourceOrigin,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct DynamicSourceActiveBinding {
    pub binding: DynamicSourceBinding,
    pub occurrence_id: DynamicSourceOccurrenceId,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct DynamicSourceOriginsSnapshot {
    pub version: u16,
    pub records: Vec<DynamicSourceRecord>,
    pub bindings: Vec<DynamicSourceActiveBinding>,
}

/// Preview forks share immutable records and bindings. A changed assignment copies shared maps
/// only on the cold write; unchanged binds and frame reads allocate neither maps nor UUIDs.
/// Every publication of this shared catalogue is ordered under the authoritative Dynamics
/// runtime mutex, so a cold checkpoint can capture the matching runtime and origins together.
pub(in crate::runtime) type SharedDynamicSourceOrigins =
    Arc<arc_swap::ArcSwap<DynamicSourceOrigins>>;

#[derive(Clone, Debug, Default)]
pub(super) struct DynamicSourceOrigins {
    /// Fx-hashed (TL-639 round 4): every family projection and static binding looks a record
    /// up per target and frame. Iteration order is never observable: `snapshot` sorts.
    records: Arc<SourceRecords>,
    /// Fx-hashed (TL-639): every captured assignment looks itself up once per frame. Iteration
    /// order is never observable.
    bindings: Arc<rustc_hash::FxHashMap<DynamicSourceBinding, DynamicSourceOccurrenceId>>,
    /// Runtime-only validation cache. Weak identity keeps neither the captured source frame nor
    /// its historical evidence alive; at most one entry belongs to each active static binding.
    static_evidence: Arc<rustc_hash::FxHashMap<DynamicSourceBinding, CachedStaticEvidence>>,
}

/// See [`DynamicSourceOrigins::records_identity`]. The `Weak` keeps the allocation reserved,
/// so a later catalogue can never reuse its address.
#[derive(Debug)]
pub(in crate::runtime) struct RecordsIdentity(Weak<SourceRecords>);

/// The immutable record catalogue, by occurrence.
type SourceRecords = rustc_hash::FxHashMap<DynamicSourceOccurrenceId, Arc<DynamicSourceRecord>>;

#[derive(Clone, Debug)]
struct CachedStaticEvidence {
    occurrence_id: DynamicSourceOccurrenceId,
    evidence: Weak<light_engine::ContributionFamilyEvidence>,
}

impl DynamicSourceOrigins {
    /// A frame that only read the catalogue can keep the published Arc unchanged.
    pub fn shares_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.records, &other.records)
            && Arc::ptr_eq(&self.bindings, &other.bindings)
            && Arc::ptr_eq(&self.static_evidence, &other.static_evidence)
    }

    /// Validate a captured static family once per immutable evidence allocation. Ordinary
    /// singleton observations compare borrowed metadata because Playback may recreate them on
    /// every frame. Multi-source transitions reuse their captured Arc throughout the fade.
    pub fn bind_static_evidence(
        &mut self,
        binding: DynamicSourceBinding,
        evidence: &Arc<light_engine::ContributionFamilyEvidence>,
    ) -> Result<DynamicSourceOccurrenceId, IntentError> {
        if let Some(id) = self.binding(&binding) {
            if let [entry] = evidence.entries() {
                if let DynamicSourceOrigin::StaticBaseline { sources } = &self.records[&id].origin
                    && let [existing] = sources.as_slice()
                    && existing.matches_evidence(entry)
                {
                    self.forget_static_evidence(&binding);
                    return Ok(id);
                }
            } else if let Some(cached) = self.static_evidence.get(&binding)
                && cached.occurrence_id == id
                && cached.evidence.as_ptr() == Arc::as_ptr(evidence)
            {
                // The Weak keeps its allocation identity reserved even after the value dies;
                // an incoming strong Arc cannot match a different, recycled allocation.
                return Ok(id);
            }
        }
        let sources = evidence
            .entries()
            .iter()
            .map(|entry| {
                DynamicStaticSourceEntry::from_evidence(
                    DynamicStaticSource::from_contribution(entry.source()),
                    entry,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        // bind validates before changing records, bindings or the runtime cache. A distinct
        // allocation with equivalent evidence keeps its occurrence and only warms this cache.
        let id = self.bind(binding, DynamicSourceOrigin::StaticBaseline { sources })?;
        if evidence.entries().len() > 1 {
            Arc::make_mut(&mut self.static_evidence).insert(
                binding,
                CachedStaticEvidence {
                    occurrence_id: id,
                    evidence: Arc::downgrade(evidence),
                },
            );
        } else {
            self.forget_static_evidence(&binding);
        }
        Ok(id)
    }

    fn forget_static_evidence(&mut self, binding: &DynamicSourceBinding) {
        if self.static_evidence.contains_key(binding) {
            Arc::make_mut(&mut self.static_evidence).remove(binding);
        }
    }

    pub fn bind(
        &mut self,
        binding: DynamicSourceBinding,
        mut origin: DynamicSourceOrigin,
    ) -> Result<DynamicSourceOccurrenceId, IntentError> {
        validate_binding(binding)?;
        origin.validate(binding)?;
        origin.validate_controller(binding)?;
        origin.canonicalize();
        if let Some(id) = self.bindings.get(&binding) {
            let existing = &self.records[id];
            if existing.origin == origin {
                return Ok(*id);
            }
        }
        let id = loop {
            let candidate = DynamicSourceOccurrenceId::new(Uuid::new_v4())?;
            if !self.records.contains_key(&candidate) {
                break candidate;
            }
        };
        Arc::make_mut(&mut self.records).insert(
            id,
            Arc::new(DynamicSourceRecord {
                occurrence_id: id,
                binding,
                origin,
            }),
        );
        Arc::make_mut(&mut self.bindings).insert(binding, id);
        self.forget_static_evidence(&binding);
        Ok(id)
    }

    pub fn get(&self, id: DynamicSourceOccurrenceId) -> Option<&Arc<DynamicSourceRecord>> {
        self.records.get(&id)
    }

    /// Identity of the record catalogue. Records are immutable and only a write replaces the
    /// catalogue; while a `Weak` is held, a write moves it to a new allocation (TL-639).
    pub fn records_identity(&self) -> RecordsIdentity {
        RecordsIdentity(Arc::downgrade(&self.records))
    }

    /// Whether no record was added, removed or restored since `identity` was taken.
    pub fn has_records_identity(&self, identity: &RecordsIdentity) -> bool {
        std::ptr::eq(identity.0.as_ptr(), Arc::as_ptr(&self.records))
    }

    pub fn binding(&self, binding: &DynamicSourceBinding) -> Option<DynamicSourceOccurrenceId> {
        self.bindings.get(binding).copied()
    }

    pub fn unbind(&mut self, binding: &DynamicSourceBinding) -> bool {
        if !self.bindings.contains_key(binding) {
            return false;
        }
        Arc::make_mut(&mut self.bindings).remove(binding);
        self.forget_static_evidence(binding);
        true
    }

    /// Retire active assignments that no longer belong to the captured source set. Their
    /// immutable records stay available to paused and interrupted expression history.
    pub fn retain_bindings(
        &mut self,
        mut predicate: impl FnMut(&DynamicSourceRecord) -> bool,
    ) -> usize {
        let removed = self
            .bindings
            .iter()
            .filter_map(|(binding, occurrence_id)| {
                let record = self
                    .records
                    .get(occurrence_id)
                    .expect("active Dynamic binding has an immutable source record");
                (!predicate(record)).then_some(*binding)
            })
            .collect::<Vec<_>>();
        self.remove_bindings(&removed);
        removed.len()
    }

    /// [`Self::retain_bindings`] for a predicate that decides from the binding and its
    /// occurrence alone (TL-639): no record is looked up for any binding.
    pub fn retain_bindings_by_key(
        &mut self,
        mut keep: impl FnMut(&DynamicSourceBinding, DynamicSourceOccurrenceId) -> bool,
    ) -> usize {
        let removed = self
            .bindings
            .iter()
            .filter_map(|(binding, occurrence_id)| {
                (!keep(binding, *occurrence_id)).then_some(*binding)
            })
            .collect::<Vec<_>>();
        self.remove_bindings(&removed);
        removed.len()
    }

    /// [`Self::retain_bindings`] over authored bindings only; static-baseline and fixed
    /// bindings are kept without a record lookup (TL-639). Their records can only carry
    /// static-baseline and fixed origins (`DynamicSourceOrigin::validate`).
    pub fn retain_authored_bindings(
        &mut self,
        mut keep: impl FnMut(&DynamicSourceRecord) -> bool,
    ) -> usize {
        let removed = self
            .bindings
            .iter()
            .filter(|(binding, _)| matches!(binding, DynamicSourceBinding::Authored { .. }))
            .filter_map(|(binding, occurrence_id)| {
                let record = self
                    .records
                    .get(occurrence_id)
                    .expect("active Dynamic binding has an immutable source record");
                (!keep(record)).then_some(*binding)
            })
            .collect::<Vec<_>>();
        self.remove_bindings(&removed);
        removed.len()
    }

    fn remove_bindings(&mut self, removed: &[DynamicSourceBinding]) {
        if !removed.is_empty() {
            let bindings = Arc::make_mut(&mut self.bindings);
            for binding in removed {
                bindings.remove(binding);
            }
            for binding in removed {
                self.forget_static_evidence(binding);
            }
        }
    }

    /// Only explicit cold cleanup may retire history. Validate every reachable expression ID
    /// before touching either map; active assignments are retained even if not sampled this tick.
    pub fn prune(
        &mut self,
        reachable: impl IntoIterator<Item = DynamicSourceOccurrenceId>,
    ) -> Result<usize, IntentError> {
        let mut retained = self.checked_reachable(reachable)?;
        retained.extend(self.bindings.values().copied());
        let removed = self.records.len() - retained.len();
        if removed != 0 {
            Arc::make_mut(&mut self.records).retain(|id, _| retained.contains(id));
        }
        Ok(removed)
    }

    pub fn validate_reachable(
        &self,
        reachable: impl IntoIterator<Item = DynamicSourceOccurrenceId>,
    ) -> Result<(), IntentError> {
        self.checked_reachable(reachable).map(|_| ())
    }

    fn checked_reachable(
        &self,
        reachable: impl IntoIterator<Item = DynamicSourceOccurrenceId>,
    ) -> Result<HashSet<DynamicSourceOccurrenceId>, IntentError> {
        let mut result = HashSet::new();
        for id in reachable {
            if !self.records.contains_key(&id) {
                return Err(invalid(
                    "expression history refers to an unknown source occurrence",
                ));
            }
            result.insert(id);
        }
        Ok(result)
    }

    pub fn snapshot(&self) -> DynamicSourceOriginsSnapshot {
        let mut bindings = self
            .bindings
            .iter()
            .map(|(binding, occurrence_id)| DynamicSourceActiveBinding {
                binding: *binding,
                occurrence_id: *occurrence_id,
            })
            .collect::<Vec<_>>();
        // Every active binding has a distinct occurrence, so this is a stable total ordering
        // without inventing missing Dynamic scope for static-family dependencies.
        bindings.sort_by_key(|row| row.occurrence_id);
        let mut records = self
            .records
            .values()
            .map(|record| record.as_ref().clone())
            .collect::<Vec<_>>();
        // Occurrence order, as the ordered catalogue this replaced iterated.
        records.sort_by_key(|record| record.occurrence_id);
        DynamicSourceOriginsSnapshot {
            version: SNAPSHOT_VERSION,
            records,
            bindings,
        }
    }

    /// Build and validate a replacement before publishing it. The caller supplies IDs reachable
    /// from the matching runtime checkpoint; restoring each side independently is insufficient.
    pub fn restore(
        snapshot: DynamicSourceOriginsSnapshot,
        reachable: impl IntoIterator<Item = DynamicSourceOccurrenceId>,
    ) -> Result<Self, IntentError> {
        if snapshot.version != SNAPSHOT_VERSION {
            return Err(invalid("unsupported snapshot version"));
        }
        let mut records = SourceRecords::default();
        for mut record in snapshot.records {
            DynamicSourceOccurrenceId::new(record.occurrence_id.get())?;
            validate_binding(record.binding)?;
            record.origin.validate(record.binding)?;
            record.origin.validate_controller(record.binding)?;
            record.origin.canonicalize();
            if records
                .insert(record.occurrence_id, Arc::new(record))
                .is_some()
            {
                return Err(invalid("duplicate source occurrence ID"));
            }
        }
        let mut bindings = rustc_hash::FxHashMap::default();
        for row in snapshot.bindings {
            validate_binding(row.binding)?;
            let record = records
                .get(&row.occurrence_id)
                .ok_or_else(|| invalid("active binding refers to an unknown source occurrence"))?;
            if record.binding != row.binding {
                return Err(invalid(
                    "active binding and original occurrence scope disagree",
                ));
            }
            if bindings.insert(row.binding, row.occurrence_id).is_some() {
                return Err(invalid("duplicate active source binding"));
            }
        }
        let result = Self {
            records: Arc::new(records),
            bindings: Arc::new(bindings),
            static_evidence: Arc::default(),
        };
        result.validate_reachable(reachable)?;
        Ok(result)
    }
}

fn validate_binding(binding: DynamicSourceBinding) -> Result<(), IntentError> {
    match binding {
        DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            target,
            lane_id,
        } => {
            non_nil(instance_id, "runtime instance")?;
            non_nil(controller_id, "controller")?;
            non_nil(target.0, "target")?;
            non_nil(lane_id, "lane")
        }
        DynamicSourceBinding::Fixed {
            source,
            target,
            owner,
            component,
        } => {
            source.validate()?;
            non_nil(target.0, "target")?;
            if component.is_some_and(|component| component.owner() != owner) {
                return Err(invalid("fixed component belongs to another owner"));
            }
            Ok(())
        }
        DynamicSourceBinding::StaticBaseline { target, .. } => non_nil(target.0, "target"),
    }
}

fn validate_playback_identity(identity: PlaybackIdentity) -> Result<(), IntentError> {
    match identity {
        PlaybackIdentity::Physical(number) => PlaybackIdentity::physical(number.get()),
        PlaybackIdentity::Virtual(address) => {
            PlaybackIdentity::virtual_playback(address.page(), address.number().get())
        }
    }
    .map(|_| ())
    .map_err(IntentError)
}

fn non_nil(id: Uuid, what: &str) -> Result<(), IntentError> {
    if id.is_nil() {
        Err(invalid(&format!("{what} identity must not be nil")))
    } else {
        Ok(())
    }
}

fn invalid(message: &str) -> IntentError {
    IntentError(format!("Dynamic source origins: {message}"))
}

fn compare_static_sources(
    left: &DynamicStaticSource,
    right: &DynamicStaticSource,
) -> std::cmp::Ordering {
    match (left, right) {
        (
            DynamicStaticSource::Programmer {
                programmer_id: a,
                lane: al,
            },
            DynamicStaticSource::Programmer {
                programmer_id: b,
                lane: bl,
            },
        ) => {
            let lane_key = |lane: &DynamicStaticProgrammerLane| match lane {
                DynamicStaticProgrammerLane::Live => 0,
                DynamicStaticProgrammerLane::Preload => 1,
                DynamicStaticProgrammerLane::Transient(_) => 2,
                DynamicStaticProgrammerLane::Group(_) => 3,
                DynamicStaticProgrammerLane::PreloadGroup(_) => 4,
            };
            a.0.cmp(&b.0)
                .then_with(|| lane_key(al).cmp(&lane_key(bl)))
                .then_with(|| match (al, bl) {
                    (
                        DynamicStaticProgrammerLane::Transient(a),
                        DynamicStaticProgrammerLane::Transient(b),
                    )
                    | (
                        DynamicStaticProgrammerLane::Group(a),
                        DynamicStaticProgrammerLane::Group(b),
                    )
                    | (
                        DynamicStaticProgrammerLane::PreloadGroup(a),
                        DynamicStaticProgrammerLane::PreloadGroup(b),
                    ) => a.cmp(b),
                    _ => std::cmp::Ordering::Equal,
                })
        }
        (
            DynamicStaticSource::Playback { source: a },
            DynamicStaticSource::Playback { source: b },
        ) => (
            a.cue_list_id.0,
            a.playback_number,
            a.playback_identity,
            a.temporary,
        )
            .cmp(&(
                b.cue_list_id.0,
                b.playback_number,
                b.playback_identity,
                b.temporary,
            )),
        (DynamicStaticSource::Programmer { .. }, DynamicStaticSource::Playback { .. }) => {
            std::cmp::Ordering::Less
        }
        (DynamicStaticSource::Playback { .. }, DynamicStaticSource::Programmer { .. }) => {
            std::cmp::Ordering::Greater
        }
    }
}

// Keep this checkpoint's representation independent of Playback's serde implementation. In
// particular, internally tagged enums cannot encode a transparent integer newtype variant.
mod playback_identity_wire {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Identity {
        Physical { number: u16 },
        Virtual { page: u8, number: u16 },
    }

    impl From<PlaybackIdentity> for Identity {
        fn from(value: PlaybackIdentity) -> Self {
            match value {
                PlaybackIdentity::Physical(number) => Self::Physical {
                    number: number.get(),
                },
                PlaybackIdentity::Virtual(address) => Self::Virtual {
                    page: address.page(),
                    number: address.number().get(),
                },
            }
        }
    }

    impl Identity {
        fn decode(self) -> Result<PlaybackIdentity, String> {
            match self {
                Self::Physical { number } => PlaybackIdentity::physical(number),
                Self::Virtual { page, number } => PlaybackIdentity::virtual_playback(page, number),
            }
        }
    }

    pub fn serialize<S: serde::Serializer>(
        value: &PlaybackIdentity,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Identity::from(*value).serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PlaybackIdentity, D::Error> {
        Identity::deserialize(deserializer)?
            .decode()
            .map_err(serde::de::Error::custom)
    }

    pub mod optional {
        use super::*;

        pub fn serialize<S: serde::Serializer>(
            value: &Option<PlaybackIdentity>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            value.map(Identity::from).serialize(serializer)
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<PlaybackIdentity>, D::Error> {
            Option::<Identity>::deserialize(deserializer)?
                .map(Identity::decode)
                .transpose()
                .map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
#[path = "dynamic_source_origins/tests.rs"]
mod tests;
