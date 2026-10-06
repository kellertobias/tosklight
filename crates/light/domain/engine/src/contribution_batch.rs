use light_core::{
    AttributeKey, AttributeValue, FixtureId, FrameAddress, ProgrammerEditStamp, ProgrammerId,
    TimedValue, programming::ProgrammingComponent,
};
use light_playback::SequenceMasterSource;
use rustc_hash::FxHashMap;
use std::sync::Arc;

mod field_evidence;

/// Opaque identity of the semantic source whose assignment produced a sampled value.
///
/// This identifies ownership only. It deliberately does not describe a Dynamics, Phase, or
/// fixed-value product model.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContributionSourceId(SourceIdentity);

/// Borrowed, lossless ownership of an engine source. Describing an ID does not resolve values
/// or inspect current desk state; names borrow the immutable identity already held by the ID.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContributionSourceDescriptor<'a> {
    Programmer {
        programmer_id: ProgrammerId,
        lane: ContributionProgrammerLane<'a>,
    },
    Playback(SequenceMasterSource),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContributionProgrammerLane<'a> {
    Live,
    Preload,
    Transient(&'a str),
    Group(&'a str),
    PreloadGroup(&'a str),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SourceIdentity {
    Programmer {
        programmer_id: ProgrammerId,
        lane: ProgrammerLane,
    },
    Playback(SequenceMasterSource),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ProgrammerLane {
    Live,
    Preload,
    Transient(Arc<str>),
    Group(Arc<str>),
    PreloadGroup(Arc<str>),
}

impl ContributionSourceId {
    pub fn descriptor(&self) -> ContributionSourceDescriptor<'_> {
        match &self.0 {
            SourceIdentity::Programmer {
                programmer_id,
                lane,
            } => ContributionSourceDescriptor::Programmer {
                programmer_id: *programmer_id,
                lane: match lane {
                    ProgrammerLane::Live => ContributionProgrammerLane::Live,
                    ProgrammerLane::Preload => ContributionProgrammerLane::Preload,
                    ProgrammerLane::Transient(name) => ContributionProgrammerLane::Transient(name),
                    ProgrammerLane::Group(name) => ContributionProgrammerLane::Group(name),
                    ProgrammerLane::PreloadGroup(name) => {
                        ContributionProgrammerLane::PreloadGroup(name)
                    }
                },
            },
            SourceIdentity::Playback(source) => ContributionSourceDescriptor::Playback(*source),
        }
    }

    pub const fn programmer(programmer_id: ProgrammerId) -> Self {
        Self(SourceIdentity::Programmer {
            programmer_id,
            lane: ProgrammerLane::Live,
        })
    }

    pub const fn preload(programmer_id: ProgrammerId) -> Self {
        Self(SourceIdentity::Programmer {
            programmer_id,
            lane: ProgrammerLane::Preload,
        })
    }

    pub fn programmer_transient(programmer_id: ProgrammerId, source: impl Into<Arc<str>>) -> Self {
        Self(SourceIdentity::Programmer {
            programmer_id,
            lane: ProgrammerLane::Transient(source.into()),
        })
    }

    pub fn programmer_group(programmer_id: ProgrammerId, group_id: impl Into<Arc<str>>) -> Self {
        Self(SourceIdentity::Programmer {
            programmer_id,
            lane: ProgrammerLane::Group(group_id.into()),
        })
    }

    pub fn preload_group(programmer_id: ProgrammerId, group_id: impl Into<Arc<str>>) -> Self {
        Self(SourceIdentity::Programmer {
            programmer_id,
            lane: ProgrammerLane::PreloadGroup(group_id.into()),
        })
    }

    pub const fn playback(source: SequenceMasterSource) -> Self {
        Self(SourceIdentity::Playback(source))
    }
}

/// One immutable sampled semantic value plus its source-replacement context.
#[derive(Clone, Debug)]
pub struct ContributionSample {
    value: TimedValue,
    transition_ordinal: Option<u64>,
    replacement_source: Option<ContributionSourceId>,
    family_evidence: Option<Arc<ContributionFamilyEvidence>>,
    /// Where the producer already knows this pair lives. Read by number when it belongs to the
    /// frame's generation, otherwise looked up by name as before.
    address: Option<FrameAddress>,
}

impl ContributionSample {
    /// Create an independent contribution which competes with every existing source normally.
    pub fn independent(value: TimedValue) -> Self {
        Self {
            value,
            transition_ordinal: None,
            replacement_source: None,
            family_evidence: None,
            address: None,
        }
    }

    /// Say where this sample's pair lives, when the producer knows.
    pub fn at(mut self, address: Option<FrameAddress>) -> Self {
        self.address = address;
        self
    }

    pub const fn address(&self) -> Option<FrameAddress> {
        self.address
    }

    /// Attach the producer's exact authored and dependency trace. Evidence is observational:
    /// it neither replaces another source nor changes the sample's arbitration rank.
    pub fn with_family_evidence(mut self, evidence: Arc<ContributionFamilyEvidence>) -> Self {
        self.family_evidence = Some(evidence);
        self
    }

    pub fn family_evidence(&self) -> Option<&Arc<ContributionFamilyEvidence>> {
        self.family_evidence.as_ref()
    }

    /// Replace the originating semantic assignment at the same fixture and attribute.
    pub fn replacing(value: TimedValue, source: ContributionSourceId) -> Self {
        Self {
            value,
            transition_ordinal: None,
            replacement_source: Some(source),
            family_evidence: None,
            address: None,
        }
    }

    /// Replace one Playback assignment with a semantic sample from the same Playback.
    ///
    /// The sample is the Playback's own output parameter: a level (Intensity, Volume) already
    /// carries the Cue master, as every Playback contribution does, so HTP compares effective
    /// levels. The master is never applied a second time here.
    pub fn replacing_playback(
        value: TimedValue,
        source: SequenceMasterSource,
        transition_ordinal: u64,
    ) -> Self {
        Self {
            value,
            transition_ordinal: Some(transition_ordinal),
            replacement_source: Some(ContributionSourceId::playback(source)),
            family_evidence: None,
            address: None,
        }
    }

    pub fn value(&self) -> &TimedValue {
        &self.value
    }

    pub const fn transition_ordinal(&self) -> Option<u64> {
        self.transition_ordinal
    }

    pub fn replacement_source(&self) -> Option<&ContributionSourceId> {
        self.replacement_source.as_ref()
    }
}

type ReplacementIndex = FxHashMap<
    ContributionSourceId,
    FxHashMap<FixtureId, FxHashMap<AttributeKey, ReplacementSamples>>,
>;

/// One sample per source/address is the normal hot path; duplicates need no allocation unless
/// a producer actually supplies them.
#[derive(Debug)]
struct ReplacementSamples {
    first: usize,
    rest: Vec<usize>,
}
type ExclusionIndex = FxHashMap<
    ContributionSourceId,
    FxHashMap<FixtureId, FxHashMap<AttributeKey, Option<ContributionReleaseCutoff>>>,
>;

/// Authored edit boundary of a Release. Go may restamp its clock, but keeps its edit order so
/// a normal edit made after preparing the Release remains in control.
pub type ContributionReleaseCutoff = light_core::ProgrammerEditStamp;

/// The part of a family that an explicitly traced contributor authored. A whole-family edit and
/// an edit to one component have different ownership even when they resolve to the same value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContributionFamilyFootprint {
    Whole,
    Component(ProgrammingComponent),
}

/// A calculation may read an older source without giving that source authorship of the result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContributionFamilyRole {
    Authored,
    CalculationDependency,
}

/// One producer-supplied trace entry. The engine does not derive this from rank or value equality.
#[derive(Clone, Debug)]
pub struct ContributionFamilyEntry {
    source: ContributionSourceId,
    stamp: ProgrammerEditStamp,
    transition_ordinal: Option<u64>,
    /// The stored Cue which authored this value, independently of the Playback currently
    /// tracking it. None leaves unavailable historical author metadata unknown.
    authored_cue_id: Option<uuid::Uuid>,
    footprint: ContributionFamilyFootprint,
    role: ContributionFamilyRole,
    /// Original authorship is retained above. This optional scope describes where that source
    /// participates after interpolation or representation conversion. None uses its original
    /// footprint; it is not an inferred conversion from a native channel to a visible color.
    effective_fields: Option<light_core::programming::ProgrammingFieldScope>,
}

impl ContributionFamilyEntry {
    pub const fn new(
        source: ContributionSourceId,
        stamp: ProgrammerEditStamp,
        footprint: ContributionFamilyFootprint,
        role: ContributionFamilyRole,
    ) -> Self {
        Self {
            source,
            stamp,
            transition_ordinal: None,
            authored_cue_id: None,
            footprint,
            role,
            effective_fields: None,
        }
    }

    pub fn source(&self) -> &ContributionSourceId {
        &self.source
    }
    pub const fn stamp(&self) -> ProgrammerEditStamp {
        self.stamp
    }
    /// Playback actions can share a timestamp while remaining distinct authored occurrences.
    /// This producer history ordinal is not the current contribution's LTP/transport ordinal.
    pub const fn with_transition_ordinal(mut self, transition_ordinal: Option<u64>) -> Self {
        self.transition_ordinal = transition_ordinal;
        self
    }
    pub const fn transition_ordinal(&self) -> Option<u64> {
        self.transition_ordinal
    }
    pub const fn authored_cue_id(&self) -> Option<uuid::Uuid> {
        self.authored_cue_id
    }
    pub const fn with_authored_cue_id(mut self, cue_id: Option<uuid::Uuid>) -> Self {
        self.authored_cue_id = cue_id;
        self
    }
    pub const fn footprint(&self) -> ContributionFamilyFootprint {
        self.footprint
    }
    pub const fn role(&self) -> ContributionFamilyRole {
        self.role
    }
    pub fn effective_fields(&self) -> Option<&light_core::programming::ProgrammingFieldScope> {
        self.effective_fields.as_ref()
    }
    pub fn with_effective_fields(
        mut self,
        fields: light_core::programming::ProgrammingFieldScope,
    ) -> Self {
        self.effective_fields = Some(fields);
        self
    }
}

/// Immutable family trace retained only for the winning explicitly traced sample.
#[derive(Clone, Debug)]
pub struct ContributionFamilyEvidence {
    entries: Arc<[ContributionFamilyEntry]>,
}

impl ContributionFamilyEvidence {
    pub fn new(entries: impl Into<Arc<[ContributionFamilyEntry]>>) -> Self {
        Self {
            entries: entries.into(),
        }
    }

    pub fn entries(&self) -> &[ContributionFamilyEntry] {
        &self.entries
    }

    /// The producer has proved that this is its complete authored endpoint. A sampled fade or
    /// calculated value cannot use this constructor: its historical dependencies are unknown.
    /// Only materialized programming owners qualify; legacy channel values remain untraced.
    pub(crate) fn authored_endpoint(
        origin: &ContributionOrigin,
        value: &TimedValue,
    ) -> Option<Arc<Self>> {
        use light_core::programming::{
            ProgrammingFieldScope, ProgrammingOwner, ProgrammingValueScope,
        };
        let complete = match (value.attribute.0.as_ref(), &value.value) {
            ("color", AttributeValue::ColorProgram(_))
            | ("position", AttributeValue::Position(_))
            | ("zoom", AttributeValue::Zoom(_)) => true,
            ("focus", AttributeValue::Normalized(value)) => {
                value.is_finite() && (0.0..=1.0).contains(value)
            }
            _ => false,
        };
        if !complete
            || value
                .value
                .validate_programming_address(&value.attribute)
                .is_err()
            || value
                .value
                .validate_programming_scope(ProgrammingValueScope::Fixture)
                .is_err()
        {
            return None;
        }
        let owner = value.value.programming_owner().or_else(|| {
            (value.attribute.0.as_ref() == "focus").then_some(ProgrammingOwner::Focus)
        })?;
        let fields = ProgrammingFieldScope::for_value(owner, &value.value).ok()?;
        Some(Arc::new(Self::new(vec![
            ContributionFamilyEntry::new(
                origin.source.clone(),
                origin.stamp,
                ContributionFamilyFootprint::Whole,
                ContributionFamilyRole::Authored,
            )
            .with_transition_ordinal(origin.transition_ordinal)
            .with_effective_fields(fields),
        ])))
    }
}

/// Observer-only provenance of a winning contribution. Regular output leaves this unallocated.
#[derive(Clone, Debug)]
pub struct ContributionOrigin {
    pub(crate) source: ContributionSourceId,
    pub(crate) stamp: light_core::ProgrammerEditStamp,
    transition_ordinal: Option<u64>,
}

impl ContributionOrigin {
    pub fn source(&self) -> &ContributionSourceId {
        &self.source
    }
    pub fn stamp(&self) -> light_core::ProgrammerEditStamp {
        self.stamp
    }
    pub const fn transition_ordinal(&self) -> Option<u64> {
        self.transition_ordinal
    }
    /// Whether this origin is the one `with_transition_ordinal(source, value, ordinal)` builds.
    pub(crate) fn describes(
        &self,
        source: &ContributionSourceId,
        value: &TimedValue,
        transition_ordinal: Option<u64>,
    ) -> bool {
        self.source == *source
            && self.stamp.changed_at == value.changed_at
            && self.stamp.programmer_order == value.programmer_order
            && self.transition_ordinal == transition_ordinal
    }
    pub(crate) fn new(source: ContributionSourceId, value: &TimedValue) -> Arc<Self> {
        Self::with_transition_ordinal(source, value, None)
    }
    pub(crate) fn with_transition_ordinal(
        source: ContributionSourceId,
        value: &TimedValue,
        transition_ordinal: Option<u64>,
    ) -> Arc<Self> {
        Arc::new(Self {
            source,
            stamp: light_core::ProgrammerEditStamp {
                changed_at: value.changed_at,
                programmer_order: value.programmer_order,
            },
            transition_ordinal,
        })
    }
}

/// One immutable sample from an externally owned semantic contribution source.
///
/// Stateful producers retain their own phase, pause, restart, and suppression policy. At a render
/// instant they hand the engine a finite batch of ordinary fixture-and-attribute values, which
/// then use the same priority, HTP/LTP, fixture projection, and output path as every built-in
/// source. The batch deliberately carries no product-specific Dynamics or fixed-value model.
#[derive(Clone, Debug, Default)]
#[must_use = "a sampled contribution batch has no effect until it is passed to the engine"]
pub struct ContributionBatch {
    samples: Arc<[ContributionSample]>,
    replacements: Arc<ReplacementIndex>,
    exclusions: Arc<ExclusionIndex>,
}

impl ContributionBatch {
    pub fn new(samples: impl IntoIterator<Item = ContributionSample>) -> Self {
        Self::from(samples.into_iter().collect::<Vec<_>>())
    }

    /// Suppress exact source assignments before arbitration, without fabricating a zero or a
    /// replacement winner. Used by observational Release projection to reveal the true underlay.
    pub fn excluding(
        values: impl IntoIterator<Item = (ContributionSourceId, FixtureId, AttributeKey)>,
    ) -> Self {
        Self::with_exclusions(
            values
                .into_iter()
                .map(|(source, fixture, attribute)| (source, fixture, attribute, None)),
        )
    }

    /// Release only older contributions from the exact authored scope. Unlike replacement
    /// samples, these also suppress older external samples from that same source.
    pub fn releasing(
        values: impl IntoIterator<
            Item = (
                ContributionSourceId,
                FixtureId,
                AttributeKey,
                ContributionReleaseCutoff,
            ),
        >,
    ) -> Self {
        Self::with_exclusions(
            values
                .into_iter()
                .map(|(source, fixture, attribute, cutoff)| {
                    (source, fixture, attribute, Some(cutoff))
                }),
        )
    }

    fn with_exclusions(
        values: impl IntoIterator<
            Item = (
                ContributionSourceId,
                FixtureId,
                AttributeKey,
                Option<ContributionReleaseCutoff>,
            ),
        >,
    ) -> Self {
        let mut exclusions = ExclusionIndex::default();
        for (source, fixture, attribute, cutoff) in values {
            exclusions
                .entry(source)
                .or_default()
                .entry(fixture)
                .or_default()
                .entry(attribute)
                .and_modify(|known| {
                    if cutoff.is_none()
                        || cutoff.zip(*known).is_some_and(|(next, old)| {
                            next.supersedes(old.changed_at, old.programmer_order)
                        })
                    {
                        *known = cutoff;
                    }
                })
                .or_insert(cutoff);
        }
        Self {
            samples: Arc::from([]),
            replacements: Arc::default(),
            exclusions: Arc::new(exclusions),
        }
    }

    pub fn samples(&self) -> &[ContributionSample] {
        &self.samples
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty() && !self.has_replacements()
    }

    /// Number of produced samples. An exclusion-only batch has zero samples but is not empty.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    fn replacement_samples<'a>(
        &'a self,
        source: &ContributionSourceId,
        value: &TimedValue,
    ) -> impl Iterator<Item = &'a ContributionSample> {
        self.replacements
            .get(source)
            .and_then(|fixtures| fixtures.get(&value.fixture_id))
            .and_then(|attributes| attributes.get(&value.attribute))
            .into_iter()
            .flat_map(|indices| std::iter::once(indices.first).chain(indices.rest.iter().copied()))
            .map(|index| &self.samples[index])
    }

    fn excludes(&self, source: &ContributionSourceId, value: &TimedValue) -> bool {
        self.excludes_origin(
            source,
            value.fixture_id,
            &value.attribute,
            value.changed_at,
            value.programmer_order,
        )
    }

    pub(crate) fn excludes_origin(
        &self,
        source: &ContributionSourceId,
        fixture: FixtureId,
        attribute: &AttributeKey,
        changed_at: chrono::DateTime<chrono::Utc>,
        programmer_order: u64,
    ) -> bool {
        self.exclusions
            .get(source)
            .and_then(|fixtures| fixtures.get(&fixture))
            .and_then(|attributes| attributes.get(attribute))
            .is_some_and(|cutoff| {
                cutoff.is_none_or(|cutoff| cutoff.supersedes(changed_at, programmer_order))
            })
    }

    pub(crate) fn has_replacements(&self) -> bool {
        !self.replacements.is_empty() || !self.exclusions.is_empty()
    }

    pub(crate) fn excluded_addresses(&self) -> impl Iterator<Item = (FixtureId, &AttributeKey)> {
        self.exclusions.values().flat_map(|fixtures| {
            fixtures.iter().flat_map(|(fixture, attributes)| {
                attributes
                    .keys()
                    .map(move |attribute| (*fixture, attribute))
            })
        })
    }
}

impl From<Vec<ContributionSample>> for ContributionBatch {
    fn from(samples: Vec<ContributionSample>) -> Self {
        // A batch of independent samples — every Dynamics tick — replaces nothing, and is told
        // so without hashing each sample's source, fixture and attribute to find out.
        let mut replacements = ReplacementIndex::default();
        for (index, sample) in samples.iter().enumerate() {
            let Some(source) = sample.replacement_source.as_ref() else {
                continue;
            };
            replacements
                .entry(source.clone())
                .or_default()
                .entry(sample.value.fixture_id)
                .or_default()
                .entry(sample.value.attribute.clone())
                .and_modify(|indices| indices.rest.push(index))
                .or_insert_with(|| ReplacementSamples {
                    first: index,
                    rest: Vec::new(),
                });
        }
        Self {
            samples: Arc::from(samples),
            replacements: Arc::new(replacements),
            exclusions: Arc::default(),
        }
    }
}

pub(crate) fn sampled_values(
    batches: &[ContributionBatch],
) -> impl Iterator<Item = &ContributionSample> {
    let has_exclusions = batches.iter().any(|batch| !batch.exclusions.is_empty());
    batches
        .iter()
        .flat_map(|batch| batch.samples())
        .filter(move |sample| {
            !has_exclusions
                || !sample.replacement_source().is_some_and(|source| {
                    batches
                        .iter()
                        .any(|batch| batch.excludes(source, sample.value()))
                })
        })
}

pub(crate) fn replaces_source(
    batches: &[ContributionBatch],
    source: &ContributionSourceId,
    value: &TimedValue,
) -> bool {
    batches.iter().any(|batch| batch.excludes(source, value))
        || batches.iter().any(|batch| {
            batch.replacement_samples(source, value).any(|sample| {
                !batches
                    .iter()
                    .any(|batch| batch.excludes(source, sample.value()))
            })
        })
}
