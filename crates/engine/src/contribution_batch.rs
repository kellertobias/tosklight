use light_core::{AttributeKey, FixtureId, ProgrammerId, TimedValue};
use light_playback::SequenceMasterSource;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Opaque identity of the semantic source whose assignment produced a sampled value.
///
/// This identifies ownership only. It deliberately does not describe a Dynamics, Phaser, or
/// fixed-value product model.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContributionSourceId(SourceIdentity);

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

/// Playback master metadata retained by a sampled non-Intensity contribution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContributionSequenceMaster {
    pub(crate) source: SequenceMasterSource,
    pub(crate) scale: f32,
}

impl ContributionSequenceMaster {
    pub const fn new(source: SequenceMasterSource, scale: f32) -> Self {
        Self { source, scale }
    }

    pub const fn source(self) -> SequenceMasterSource {
        self.source
    }

    pub const fn scale(self) -> f32 {
        self.scale
    }
}

/// One immutable sampled semantic value plus its source-replacement and master context.
#[derive(Clone, Debug)]
pub struct ContributionSample {
    value: TimedValue,
    replacement_source: Option<ContributionSourceId>,
    sequence_master: Option<ContributionSequenceMaster>,
}

impl ContributionSample {
    /// Create an independent contribution which competes with every existing source normally.
    pub fn independent(value: TimedValue) -> Self {
        Self {
            value,
            replacement_source: None,
            sequence_master: None,
        }
    }

    /// Replace the originating semantic assignment at the same fixture and attribute.
    pub fn replacing(value: TimedValue, source: ContributionSourceId) -> Self {
        Self {
            value,
            replacement_source: Some(source),
            sequence_master: None,
        }
    }

    /// Replace one Playback assignment while retaining its sequence-master context.
    pub fn replacing_playback(
        value: TimedValue,
        source: SequenceMasterSource,
        sequence_master: f32,
    ) -> Self {
        Self {
            value,
            replacement_source: Some(ContributionSourceId::playback(source)),
            sequence_master: Some(ContributionSequenceMaster::new(source, sequence_master)),
        }
    }

    pub fn with_sequence_master(mut self, source: SequenceMasterSource, scale: f32) -> Self {
        self.sequence_master = Some(ContributionSequenceMaster::new(source, scale));
        self
    }

    pub fn value(&self) -> &TimedValue {
        &self.value
    }

    pub fn replacement_source(&self) -> Option<&ContributionSourceId> {
        self.replacement_source.as_ref()
    }

    pub const fn sequence_master(&self) -> Option<ContributionSequenceMaster> {
        self.sequence_master
    }
}

type ReplacementIndex = HashMap<ContributionSourceId, HashMap<FixtureId, HashSet<AttributeKey>>>;

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
}

impl ContributionBatch {
    pub fn new(samples: impl IntoIterator<Item = ContributionSample>) -> Self {
        Self::from(samples.into_iter().collect::<Vec<_>>())
    }

    pub fn samples(&self) -> &[ContributionSample] {
        &self.samples
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub(crate) fn replaces(&self, source: &ContributionSourceId, value: &TimedValue) -> bool {
        self.replacements
            .get(source)
            .and_then(|fixtures| fixtures.get(&value.fixture_id))
            .is_some_and(|attributes| attributes.contains(&value.attribute))
    }

    pub(crate) fn has_replacements(&self) -> bool {
        !self.replacements.is_empty()
    }
}

impl From<Vec<ContributionSample>> for ContributionBatch {
    fn from(samples: Vec<ContributionSample>) -> Self {
        let mut replacements = ReplacementIndex::new();
        for sample in &samples {
            let Some(source) = sample.replacement_source.clone() else {
                continue;
            };
            replacements
                .entry(source)
                .or_default()
                .entry(sample.value.fixture_id)
                .or_default()
                .insert(sample.value.attribute.clone());
        }
        Self {
            samples: Arc::from(samples),
            replacements: Arc::new(replacements),
        }
    }
}

pub(crate) fn sampled_values(
    batches: &[ContributionBatch],
) -> impl Iterator<Item = &ContributionSample> {
    batches.iter().flat_map(|batch| batch.samples())
}

pub(crate) fn replaces_source(
    batches: &[ContributionBatch],
    source: &ContributionSourceId,
    value: &TimedValue,
) -> bool {
    batches.iter().any(|batch| batch.replaces(source, value))
}
