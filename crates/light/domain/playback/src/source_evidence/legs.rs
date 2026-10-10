use super::*;

type EndpointMap = HashMap<AttributeAddress, EndpointEvidence>;

/// One retained manual leg. Reversing the fader selects another phase of this same occurrence.
/// The underlying timed route remains available if an existing control cancels the manual leg.
#[derive(Clone, Debug)]
pub(super) struct ManualLeg {
    pub(super) base: Option<Box<PlaybackSourceHistory>>,
    from_index: usize,
    to_index: usize,
    pub(super) from: Arc<EndpointMap>,
}

impl PlaybackSourceHistory {
    pub(crate) fn matches_manual_route(&self, from: Option<usize>, to: Option<usize>) -> bool {
        match (&self.manual, from, to) {
            (None, None, None) => true,
            (Some(leg), Some(from), Some(to)) => leg.from_index == from && leg.to_index == to,
            _ => false,
        }
    }

    /// Construct evidence for the exact compiled endpoints used by the existing evaluator.
    /// Manual legs and automatic temporary transitions deliberately do not capture live interiors.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn cue_leg(
        previous: Option<&Self>,
        compiled: &Arc<CompiledCueList>,
        source: SequenceMasterSource,
        at: DateTime<Utc>,
        ordinal: u64,
        from_index: usize,
        to_index: usize,
        target_wrap: bool,
        manual: bool,
    ) -> Self {
        let from = Arc::new(endpoint_sources(previous, compiled, from_index, false));
        let mut history = Self::next(
            None,
            at,
            ordinal,
            compiled,
            to_index,
            target_wrap,
            source,
            false,
        );
        let mut cache = PlaybackEvidenceCache {
            generation: Arc::downgrade(compiled.source_generation()),
            target_index: to_index,
            target_wrap,
            targets: HashMap::new(),
            phases: HashMap::new(),
        };
        for attribute in compiled.attributes() {
            let from_value = attribute.value(from_index, false);
            let to_value = attribute.value(to_index, target_wrap);
            if ![from_value, to_value]
                .into_iter()
                .flatten()
                .any(|value| family_owner(attribute.attribute(), value).is_some())
            {
                continue;
            }
            let address = (attribute.fixture_id(), attribute.attribute().clone());
            let previous = from.get(&address);
            let mut target = history.target(compiled, attribute, to_index, target_wrap);
            if target.change.is_some_and(|(index, _)| index != to_index)
                && let Some(previous) = previous.filter(|previous| previous.change == target.change)
            {
                target = previous.clone();
            }
            cache.phases.insert(
                address.clone(),
                EvidencePhases::new(
                    attribute.attribute(),
                    from_value,
                    to_value,
                    previous.and_then(|entry| entry.evidence.clone()),
                    target.evidence.clone(),
                ),
            );
            cache.targets.insert(address, target);
        }
        history.cache = Arc::new(OnceLock::from(Arc::new(cache)));
        history.manual = manual.then(|| {
            Arc::new(ManualLeg {
                base: previous.cloned().map(Box::new),
                from_index,
                to_index,
                from,
            })
        });
        history
    }

    pub(crate) fn cancel_manual(self) -> Option<Self> {
        match &self.manual {
            Some(leg) => leg.base.as_deref().cloned(),
            None => Some(self),
        }
    }

    pub(crate) fn complete_manual(
        mut self,
        compiled: &Arc<CompiledCueList>,
        target_index: usize,
        target_wrap: bool,
    ) -> Option<Self> {
        let leg = self.manual.as_ref()?;
        if leg.to_index != target_index
            || self.generation.as_ptr() != Arc::as_ptr(compiled.source_generation())
        {
            return None;
        }
        let targets = &self.cache.get()?.targets;
        let mut completed = EndpointMap::new();
        for attribute in compiled.attributes() {
            let Some((index, cue, _)) = attribute.author(target_index, target_wrap) else {
                continue;
            };
            let address = (attribute.fixture_id(), attribute.attribute().clone());
            let change = Some((index, cue));
            // A tracking wrap can expose retained final-Cue rows only after the leg completes.
            // Match their immutable authored row to the outgoing endpoint, never by value.
            let entry = targets
                .get(&address)
                .filter(|entry| entry.change == change)
                .or_else(|| {
                    leg.from
                        .get(&address)
                        .filter(|entry| entry.change == change)
                })
                .cloned()
                .unwrap_or(EndpointEvidence {
                    change,
                    evidence: None,
                });
            completed.insert(address, entry);
        }
        self.manual = None;
        self.target_index = target_index;
        self.target_wrap = target_wrap;
        self.endpoint_overrides = Some(Arc::new(completed));
        self.cache = Arc::default();
        self.prior = None;
        Some(self)
    }
}

fn endpoint_sources(
    history: Option<&PlaybackSourceHistory>,
    compiled: &Arc<CompiledCueList>,
    index: usize,
    wrap: bool,
) -> EndpointMap {
    let history = history.filter(|history| {
        history.may_compile
            && history.generation.as_ptr() == Arc::as_ptr(compiled.source_generation())
    });
    compiled
        .attributes()
        .iter()
        .filter_map(|attribute| {
            let value = attribute.value(index, wrap)?;
            family_owner(attribute.attribute(), value)?;
            let (change_index, cue, _) = attribute.author(index, wrap)?;
            let change = Some((change_index, cue));
            let address = (attribute.fixture_id(), attribute.attribute().clone());
            let entry = history
                .and_then(|history| {
                    if let Some(entry) = history
                        .cache
                        .get()
                        .and_then(|cache| cache.targets.get(&address))
                        .filter(|entry| entry.change == change)
                    {
                        return Some(entry.clone());
                    }
                    (history.target_index == index
                        && history.target_wrap == wrap
                        && history.manual.is_none())
                    .then(|| history.target(compiled, attribute, index, wrap))
                })
                .unwrap_or(EndpointEvidence {
                    change,
                    evidence: None,
                });
            Some((address, entry))
        })
        .collect()
}
