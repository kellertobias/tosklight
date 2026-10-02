//! Playback-owned source history. Numerical contribution timestamps remain arbitration state;
//! these immutable occurrences retain the actions which supplied the actual sampled fields.
use crate::*;
use light_core::programming::{
    IntentError, ProgrammingComponent, ProgrammingFieldScope, ProgrammingFieldTransfer,
    ProgrammingOwner, ProgrammingTransitionTrace, TransitionError, interpolate_programming_trace,
};
use std::sync::{OnceLock, Weak};

pub(crate) fn take_occurrence_ordinal(next: &mut u64) -> Option<u64> {
    if *next == 0 {
        return None;
    }
    let ordinal = *next;
    *next = ordinal.checked_add(1).unwrap_or(0);
    Some(ordinal)
}

mod legs;
use legs::ManualLeg;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlaybackSourceOccurrence {
    #[serde(with = "source_wire")]
    pub source: SequenceMasterSource,
    pub action_changed_at: DateTime<Utc>,
    pub action_ordinal: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_cue_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlaybackFamilyFootprint {
    Whole,
    Component(ProgrammingComponent),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PlaybackFamilyRole {
    Authored,
    CalculationDependency,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlaybackFamilyEntry {
    pub occurrence: PlaybackSourceOccurrence,
    pub footprint: PlaybackFamilyFootprint,
    pub role: PlaybackFamilyRole,
    pub effective_fields: ProgrammingFieldScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlaybackFamilyEvidence {
    entries: Arc<[PlaybackFamilyEntry]>,
}

impl PlaybackFamilyEvidence {
    pub fn try_new(entries: impl Into<Arc<[PlaybackFamilyEntry]>>) -> Result<Self, IntentError> {
        let entries = entries.into();
        let mut owner = None;
        for entry in entries.iter() {
            if matches!(entry.footprint, PlaybackFamilyFootprint::Component(ProgrammingComponent::NativeColor(binding))
                if binding.channel_id.is_nil() || binding.function_id.is_nil())
            {
                return Err(IntentError(
                    "invalid native Playback source footprint".into(),
                ));
            }
            let source = entry.occurrence.source;
            if source.cue_list_id.0.is_nil()
                || entry.occurrence.action_ordinal == 0
                || entry
                    .occurrence
                    .authored_cue_id
                    .is_some_and(|id| id.is_nil())
                || source
                    .playback_number
                    .is_some_and(|number| number == 0 || number > MAX_VIRTUAL_PLAYBACK)
                || source.playback_identity.is_some_and(|identity| {
                    source
                        .playback_number
                        .is_some_and(|number| number != identity.number())
                })
            {
                return Err(IntentError("invalid retained Playback occurrence".into()));
            }
            if let Some(field) = entry.effective_fields.fields().first() {
                let entry_owner = field.owner();
                if owner.is_some_and(|owner| owner != entry_owner)
                    || matches!(entry.footprint, PlaybackFamilyFootprint::Component(component) if component.owner() != entry_owner)
                {
                    return Err(IntentError(
                        "Playback evidence mixes programming owners".into(),
                    ));
                }
                entry.effective_fields.validate(entry_owner)?;
                owner = Some(entry_owner);
            } else {
                return Err(IntentError(
                    "Playback source entry has no effective fields".into(),
                ));
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[PlaybackFamilyEntry] {
        &self.entries
    }
}

impl<'de> Deserialize<'de> for PlaybackFamilyEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            entries: Vec<PlaybackFamilyEntry>,
        }
        Self::try_new(Wire::deserialize(deserializer)?.entries).map_err(serde::de::Error::custom)
    }
}

/// Flattening preserves the old TimedValue row shape. Missing producer history is unknown.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlaybackRetainedValue {
    #[serde(flatten)]
    pub timed: TimedValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_evidence: Option<Arc<PlaybackFamilyEvidence>>,
}

impl<'de> Deserialize<'de> for PlaybackRetainedValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            #[serde(flatten)]
            timed: TimedValue,
            #[serde(default)]
            family_evidence: Option<Arc<PlaybackFamilyEvidence>>,
        }
        let row = Wire::deserialize(deserializer)?;
        if let Some(evidence) = &row.family_evidence {
            let owner = family_owner(&row.timed.attribute, &row.timed.value).ok_or_else(|| {
                serde::de::Error::custom("Playback evidence requires a complete owner")
            })?;
            let available = ProgrammingFieldScope::for_value(owner, &row.timed.value)
                .map_err(serde::de::Error::custom)?;
            for entry in evidence.entries() {
                entry
                    .effective_fields
                    .validate(owner)
                    .map_err(serde::de::Error::custom)?;
                if entry.effective_fields.intersection(&available) != entry.effective_fields {
                    return Err(serde::de::Error::custom(
                        "Playback evidence names absent value fields",
                    ));
                }
            }
        }
        Ok(Self {
            timed: row.timed,
            family_evidence: row.family_evidence,
        })
    }
}

impl std::ops::Deref for PlaybackRetainedValue {
    type Target = TimedValue;
    fn deref(&self) -> &Self::Target {
        &self.timed
    }
}

impl From<PlaybackContribution> for PlaybackRetainedValue {
    fn from(value: PlaybackContribution) -> Self {
        Self {
            timed: value.value,
            family_evidence: value.family_evidence,
        }
    }
}

/// Original action clock survives pause/resume restamping. Compiled associations are runtime
/// only: restore never certifies a rebuilt Cue value by its numerical equality to old output.
#[derive(Clone, Debug, Serialize)]
pub struct PlaybackSourceHistory {
    action_changed_at: DateTime<Utc>,
    action_ordinal: u64,
    #[serde(skip)]
    source: Option<SequenceMasterSource>,
    #[serde(skip)]
    may_compile: bool,
    #[serde(skip)]
    generation: Weak<()>,
    #[serde(skip)]
    target_index: usize,
    #[serde(skip)]
    target_wrap: bool,
    #[serde(skip)]
    prior: Option<Arc<PlaybackEvidenceCache>>,
    #[serde(skip)]
    has_retained_source: bool,
    #[serde(skip)]
    cache: Arc<OnceLock<Arc<PlaybackEvidenceCache>>>,
    #[serde(skip)]
    manual: Option<Arc<ManualLeg>>,
    #[serde(skip)]
    endpoint_overrides: Option<Arc<HashMap<AttributeAddress, EndpointEvidence>>>,
}

impl<'de> Deserialize<'de> for PlaybackSourceHistory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            action_changed_at: DateTime<Utc>,
            action_ordinal: u64,
        }
        let row = Wire::deserialize(deserializer)?;
        if row.action_ordinal == 0 {
            return Err(serde::de::Error::custom(
                "invalid Playback source history ordinal",
            ));
        }
        Ok(Self {
            action_changed_at: row.action_changed_at,
            action_ordinal: row.action_ordinal,
            source: None,
            may_compile: false,
            generation: Weak::new(),
            target_index: 0,
            target_wrap: false,
            prior: None,
            has_retained_source: false,
            cache: Arc::default(),
            manual: None,
            endpoint_overrides: None,
        })
    }
}

impl PartialEq for PlaybackSourceHistory {
    fn eq(&self, other: &Self) -> bool {
        self.action_changed_at == other.action_changed_at
            && self.action_ordinal == other.action_ordinal
    }
}

impl PlaybackSourceHistory {
    pub(crate) fn forget_evaluation_cache(&mut self) {
        if self.manual.is_none() {
            self.cache = Arc::default();
        }
    }

    pub fn action_changed_at(&self) -> DateTime<Utc> {
        self.action_changed_at
    }
    pub fn action_ordinal(&self) -> u64 {
        self.action_ordinal
    }

    pub(crate) fn next(
        previous: Option<&Self>,
        at: DateTime<Utc>,
        ordinal: u64,
        compiled: &Arc<CompiledCueList>,
        target_index: usize,
        target_wrap: bool,
        source: SequenceMasterSource,
        has_retained_source: bool,
    ) -> Self {
        Self {
            action_changed_at: at,
            action_ordinal: ordinal,
            source: Some(source),
            may_compile: true,
            generation: Arc::downgrade(compiled.source_generation()),
            target_index,
            target_wrap,
            has_retained_source,
            prior: previous.and_then(|previous| previous.cache.get().cloned()),
            cache: Arc::default(),
            manual: None,
            endpoint_overrides: None,
        }
    }

    pub(crate) fn cached(
        &self,
        compiled: &Arc<CompiledCueList>,
        target_index: usize,
        target_wrap: bool,
        build: impl FnOnce(&Self) -> PlaybackEvidenceCache,
    ) -> Option<&PlaybackEvidenceCache> {
        if !self.may_compile
            || self.generation.as_ptr() != Arc::as_ptr(compiled.source_generation())
            || self.target_index != target_index
            || self.target_wrap != target_wrap
        {
            return None;
        }
        let cache = self.cache.get_or_init(|| Arc::new(build(self)));
        (cache.generation.as_ptr() == Arc::as_ptr(compiled.source_generation())
            && cache.target_index == target_index
            && cache.target_wrap == target_wrap)
            .then_some(cache.as_ref())
    }

    pub(crate) fn target(
        &self,
        compiled: &Arc<CompiledCueList>,
        attribute: &CompiledAttribute,
        target_index: usize,
        target_wrap: bool,
    ) -> EndpointEvidence {
        let Some((change_index, authored_cue_id, automatic_restore)) =
            attribute.author(target_index, target_wrap)
        else {
            return EndpointEvidence::default();
        };
        let change = Some((change_index, authored_cue_id));
        if let Some(endpoints) = &self.endpoint_overrides {
            return endpoints
                .get(&(attribute.fixture_id(), attribute.attribute().clone()))
                .filter(|entry| entry.change == change)
                .cloned()
                .unwrap_or(EndpointEvidence {
                    change,
                    evidence: None,
                });
        }
        // A carried change is the same immutable authored row, not merely an equal value.
        if change_index != target_index
            && let Some(prior) = &self.prior
            && prior.generation.as_ptr() == Arc::as_ptr(compiled.source_generation())
            && let Some(previous) = prior
                .targets
                .get(&(attribute.fixture_id(), attribute.attribute().clone()))
            && previous.change == change
        {
            return previous.clone();
        }
        // An unproven retained field cannot regain an earlier occurrence from authored values.
        if change_index != target_index && self.has_retained_source {
            return EndpointEvidence {
                change,
                evidence: None,
            };
        }
        let evidence = (!automatic_restore)
            .then(|| {
                let value = attribute.value(target_index, target_wrap)?;
                let owner = family_owner(attribute.attribute(), value)?;
                let fields = ProgrammingFieldScope::for_value(owner, value).ok()?;
                PlaybackFamilyEvidence::try_new(vec![PlaybackFamilyEntry {
                    occurrence: PlaybackSourceOccurrence {
                        source: self.source?,
                        action_changed_at: self.action_changed_at,
                        action_ordinal: self.action_ordinal,
                        authored_cue_id: Some(authored_cue_id),
                    },
                    footprint: PlaybackFamilyFootprint::Whole,
                    role: PlaybackFamilyRole::Authored,
                    effective_fields: fields,
                }])
                .ok()
                .map(Arc::new)
            })
            .flatten();
        EndpointEvidence { change, evidence }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EndpointEvidence {
    change: Option<(usize, Uuid)>,
    pub(crate) evidence: Option<Arc<PlaybackFamilyEvidence>>,
}

#[derive(Debug)]
pub(crate) struct PlaybackEvidenceCache {
    pub(crate) generation: Weak<()>,
    pub(crate) target_index: usize,
    pub(crate) target_wrap: bool,
    pub(crate) targets: HashMap<AttributeAddress, EndpointEvidence>,
    pub(crate) phases: HashMap<AttributeAddress, EvidencePhases>,
}

#[derive(Debug)]
pub(crate) struct EvidencePhases {
    from: Option<Arc<PlaybackFamilyEvidence>>,
    interior: Option<Arc<PlaybackFamilyEvidence>>,
    to: Option<Arc<PlaybackFamilyEvidence>>,
}

impl EvidencePhases {
    pub(crate) fn new(
        attribute: &AttributeKey,
        from: Option<&AttributeValue>,
        to: Option<&AttributeValue>,
        from_evidence: Option<Arc<PlaybackFamilyEvidence>>,
        to_evidence: Option<Arc<PlaybackFamilyEvidence>>,
    ) -> Self {
        let owner = from
            .and_then(|value| family_owner(attribute, value))
            .or_else(|| to.and_then(|value| family_owner(attribute, value)));
        let zero = (from.is_none()
            && matches!(to, Some(AttributeValue::Normalized(_)))
            && owner.is_some())
        .then(|| {
            Arc::new(PlaybackFamilyEvidence {
                entries: Arc::default(),
            })
        });
        let interior = match (owner, from, to) {
            (Some(owner), Some(from), Some(to)) => {
                match light_core::programming::interpolate_programming_value(from, to, 0.5) {
                    Ok(_) => interpolate_programming_trace(owner, from, to, 0.5)
                        .ok()
                        .and_then(|trace| {
                            transferred(
                                owner,
                                from,
                                to,
                                from_evidence.as_ref(),
                                to_evidence.as_ref(),
                                &trace,
                            )
                        }),
                    // Playback's present evaluator actually holds from for unresolved conversions.
                    Err(TransitionError::Requires(_)) => from_evidence.clone(),
                    Err(TransitionError::Invalid(_)) => None,
                }
            }
            (Some(_), None, Some(AttributeValue::Normalized(_))) => to_evidence.clone(),
            (Some(_), Some(_), None) => from_evidence.clone(),
            _ => None,
        };
        Self {
            from: from_evidence.or(zero),
            interior,
            to: to_evidence,
        }
    }

    pub(crate) fn at(&self, progress: f32) -> Option<Arc<PlaybackFamilyEvidence>> {
        if progress >= 1.0 {
            self.to.clone()
        } else if progress <= 0.0 {
            self.from.clone()
        } else {
            self.interior.clone()
        }
    }
}

pub(crate) fn family_owner(
    attribute: &AttributeKey,
    value: &AttributeValue,
) -> Option<ProgrammingOwner> {
    match (attribute.0.as_ref(), value) {
        ("color", AttributeValue::ColorProgram(_)) => Some(ProgrammingOwner::Color),
        ("position", AttributeValue::Position(_)) => Some(ProgrammingOwner::Position),
        ("zoom", AttributeValue::Zoom(_)) => Some(ProgrammingOwner::Zoom),
        ("focus", AttributeValue::Normalized(_)) => Some(ProgrammingOwner::Focus),
        _ => None,
    }
}

fn transferred(
    owner: ProgrammingOwner,
    from_value: &AttributeValue,
    to_value: &AttributeValue,
    from: Option<&Arc<PlaybackFamilyEvidence>>,
    to: Option<&Arc<PlaybackFamilyEvidence>>,
    trace: &ProgrammingTransitionTrace,
) -> Option<Arc<PlaybackFamilyEvidence>> {
    let mut entries: Vec<PlaybackFamilyEntry> = vec![];
    let mut append = |value,
                      evidence: Option<&Arc<PlaybackFamilyEvidence>>,
                      transfer: &ProgrammingFieldTransfer|
     -> Option<()> {
        let available = ProgrammingFieldScope::for_value(owner, value).ok()?;
        if transfer.forward(&available).is_empty() {
            return Some(());
        }
        for entry in evidence?.entries() {
            entry.effective_fields.validate(owner).ok()?;
            if entry.effective_fields.intersection(&available) != entry.effective_fields {
                return None;
            }
            let fields = transfer.forward(&entry.effective_fields);
            if fields.is_empty() {
                continue;
            }
            if let Some(existing) = entries.iter_mut().find(|candidate| {
                candidate.occurrence == entry.occurrence
                    && candidate.footprint == entry.footprint
                    && candidate.role == entry.role
            }) {
                existing.effective_fields = existing.effective_fields.union(&fields);
            } else {
                let mut entry = entry.clone();
                entry.effective_fields = fields;
                entries.push(entry);
            }
        }
        Some(())
    };
    append(from_value, from, &trace.from)?;
    append(to_value, to, &trace.to)?;
    PlaybackFamilyEvidence::try_new(entries).ok().map(Arc::new)
}

mod source_wire {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Identity {
        Physical { number: u16 },
        Virtual { page: u8, number: u16 },
    }
    #[derive(Serialize, Deserialize)]
    struct Source {
        playback_number: Option<u16>,
        playback_identity: Option<Identity>,
        cue_list_id: CueListId,
        temporary: bool,
    }
    pub(super) fn serialize<S: serde::Serializer>(
        value: &SequenceMasterSource,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Source {
            playback_number: value.playback_number,
            cue_list_id: value.cue_list_id,
            temporary: value.temporary,
            playback_identity: value.playback_identity.map(|identity| match identity {
                PlaybackIdentity::Physical(number) => Identity::Physical {
                    number: number.get(),
                },
                PlaybackIdentity::Virtual(address) => Identity::Virtual {
                    page: address.page(),
                    number: address.number().get(),
                },
            }),
        }
        .serialize(serializer)
    }
    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SequenceMasterSource, D::Error> {
        let value = Source::deserialize(deserializer)?;
        let playback_identity = value
            .playback_identity
            .map(|identity| match identity {
                Identity::Physical { number } => PlaybackIdentity::physical(number),
                Identity::Virtual { page, number } => {
                    PlaybackIdentity::virtual_playback(page, number)
                }
            })
            .transpose()
            .map_err(serde::de::Error::custom)?;
        if value.cue_list_id.0.is_nil()
            || playback_identity.is_some_and(|identity| {
                value
                    .playback_number
                    .is_some_and(|number| number != identity.number())
            })
        {
            return Err(serde::de::Error::custom("invalid retained Playback source"));
        }
        Ok(SequenceMasterSource {
            playback_number: value.playback_number,
            playback_identity,
            cue_list_id: value.cue_list_id,
            temporary: value.temporary,
        })
    }
}
