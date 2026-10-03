//! Forks of the captured programming sources for a parallel section (TL-639 round 5).
//!
//! While a section runs, the frame's sources lend their caches to the workers as they stood
//! ([`FrozenSources`]) and each worker composes through its own fork: the same methods over
//! layered caches (`memo`), an overlay of the source transaction and of the Current verification
//! cache. Every key a group reads or writes belongs to its own target, and a section gives each
//! target to one worker, so a fork answers exactly what the frame's sources would have answered
//! at that point of the single-threaded loop. The frame applies each fork's changes in group
//! order ([`CapturedProgrammingSources::apply_fork`]).
use super::current_native::{
    CurrentNativeVerificationCache, NativeCurrentChanges, NativeCurrentFork,
};
use super::*;
use crate::runtime::dynamic_source_origins::{
    OriginsChanges, OriginsOverlay, OriginsStore, RecordsIdentity, SourceRecordLookup,
};

/// The frame's source transaction, or a worker's overlay of it.
pub(super) enum SourceTransaction<'a> {
    Frame(&'a mut DynamicSourceOrigins),
    Fork(OriginsOverlay<'a>),
}

impl SourceTransaction<'_> {
    /// The frame's own catalogue; a fork never binds authored sources or retires bindings.
    pub fn frame(&mut self) -> Result<&mut DynamicSourceOrigins, IntentError> {
        match self {
            Self::Frame(origins) => Ok(origins),
            Self::Fork(_) => Err(IntentError(
                "a parallel worker cannot change authored or retired source bindings".into(),
            )),
        }
    }

    fn catalogue(&self) -> &DynamicSourceOrigins {
        match self {
            Self::Frame(origins) => origins,
            Self::Fork(overlay) => overlay.base(),
        }
    }

    /// See `DynamicSourceOrigins::records_identity`. A fork answers the catalogue's identity:
    /// records it added belong to its own targets and never change an existing record, so a
    /// kept projection that matches it is still exact.
    pub fn records_identity(&self) -> RecordsIdentity {
        self.catalogue().records_identity()
    }

    pub fn has_records_identity(&self, identity: &RecordsIdentity) -> bool {
        self.catalogue().has_records_identity(identity)
    }
}

impl SourceRecordLookup for SourceTransaction<'_> {
    fn record(
        &self,
        id: light_dynamics::DynamicSourceOccurrenceId,
    ) -> Option<&Arc<crate::runtime::dynamic_source_origins::DynamicSourceRecord>> {
        match self {
            Self::Frame(origins) => origins.record(id),
            Self::Fork(overlay) => overlay.record(id),
        }
    }
}

macro_rules! delegate {
    ($self:ident, $store:ident => $body:expr) => {
        match $self {
            Self::Frame($store) => $body,
            Self::Fork($store) => $body,
        }
    };
}

impl OriginsStore for SourceTransaction<'_> {
    fn binding(
        &self,
        binding: &DynamicSourceBinding,
    ) -> Option<light_dynamics::DynamicSourceOccurrenceId> {
        delegate!(self, store => store.binding(binding))
    }

    fn cached_static(
        &self,
        binding: &DynamicSourceBinding,
    ) -> Option<&crate::runtime::dynamic_source_origins::CachedStaticEvidence> {
        delegate!(self, store => store.cached_static(binding))
    }

    fn insert_record(
        &mut self,
        id: light_dynamics::DynamicSourceOccurrenceId,
        record: Arc<crate::runtime::dynamic_source_origins::DynamicSourceRecord>,
    ) {
        delegate!(self, store => store.insert_record(id, record))
    }

    fn set_binding(
        &mut self,
        binding: DynamicSourceBinding,
        id: light_dynamics::DynamicSourceOccurrenceId,
    ) {
        delegate!(self, store => store.set_binding(binding, id))
    }

    fn remove_binding(&mut self, binding: &DynamicSourceBinding) {
        delegate!(self, store => store.remove_binding(binding))
    }

    fn set_cached_static(
        &mut self,
        binding: DynamicSourceBinding,
        cached: crate::runtime::dynamic_source_origins::CachedStaticEvidence,
    ) {
        delegate!(self, store => store.set_cached_static(binding, cached))
    }

    fn remove_cached_static(&mut self, binding: &DynamicSourceBinding) {
        delegate!(self, store => store.remove_cached_static(binding))
    }
}

/// The frame's Current verification cache, or a worker's fork of it.
pub(super) enum NativeCurrent<'a> {
    Frame(&'a RefCell<CurrentNativeVerificationCache>),
    Fork(RefCell<NativeCurrentFork<'a>>),
}

impl NativeCurrent<'_> {
    pub fn verify(
        &self,
        target: FixtureId,
        base: &AttributeValue,
        models: &dyn light_dynamics::DynamicNativeModelResolver,
    ) -> Result<(), TransitionError> {
        match self {
            Self::Frame(cache) => cache.borrow_mut().verify(target, base, models),
            Self::Fork(fork) => fork.borrow_mut().verify(target, base, models),
        }
    }
}

/// The frame's caches while a section runs.
pub(super) struct FrozenSources {
    captured_families: FxHashMap<(FixtureId, ProgrammingOwner), Option<AttributeValue>>,
    current_occurrences:
        FxHashMap<(FixtureId, ProgrammingOwner), Option<light_dynamics::DynamicSourceOccurrenceId>>,
    current: FxHashMap<FixtureId, Vec<(DynamicValueAddress, CurrentResult)>>,
    failure: Vec<TransitionError>,
    requirements: Vec<CurrentResolutionRequirement>,
}

/// What one fork changed, in group order.
pub(super) struct SourcesChanges {
    captured_families: FxHashMap<(FixtureId, ProgrammingOwner), Option<AttributeValue>>,
    current_occurrences:
        FxHashMap<(FixtureId, ProgrammingOwner), Option<light_dynamics::DynamicSourceOccurrenceId>>,
    current: FxHashMap<FixtureId, Vec<(DynamicValueAddress, CurrentResult)>>,
    failure: Vec<TransitionError>,
    requirements: Vec<CurrentResolutionRequirement>,
    origins: Option<OriginsChanges>,
    native: Option<NativeCurrentChanges>,
}

/// What a section's forks share: the frame's frozen caches, its catalogue and its Current
/// verification cache, borrowed for the section.
pub(super) struct ForkBase<'f, S> {
    static_sources: &'f S,
    frozen: &'f FrozenSources,
    origins: Option<&'f DynamicSourceOrigins>,
    native: Option<(
        &'f dyn light_dynamics::DynamicNativeModelResolver,
        &'f CurrentNativeVerificationCache,
    )>,
}

impl<S> Clone for ForkBase<'_, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for ForkBase<'_, S> {}

impl<'a, S: DynamicTickSource> CapturedProgrammingSources<'a, S> {
    /// Whether this frame's sources can fork: the preset resolver is frame-local.
    pub(super) fn forks(&self) -> bool {
        self.presets.is_none()
    }

    /// Lend the caches to a section. Nothing may use these sources until [`Self::thaw`].
    pub(super) fn freeze(&self) -> FrozenSources {
        FrozenSources {
            captured_families: self.captured_families.borrow_mut().take_own(),
            current_occurrences: self.current_occurrences.borrow_mut().take_own(),
            current: self.current.borrow_mut().take_own(),
            failure: self.failure.borrow_mut().take_own(),
            requirements: self.requirements.borrow_mut().take_own(),
        }
    }

    pub(super) fn thaw(&self, frozen: FrozenSources) {
        self.captured_families
            .borrow_mut()
            .restore_own(frozen.captured_families);
        self.current_occurrences
            .borrow_mut()
            .restore_own(frozen.current_occurrences);
        self.current.borrow_mut().restore_own(frozen.current);
        self.failure.borrow_mut().restore_own(frozen.failure);
        self.requirements
            .borrow_mut()
            .restore_own(frozen.requirements);
    }

    /// Run `section` with the base every fork of this frame shares (caches frozen).
    pub(super) fn with_fork_base<R>(
        &self,
        frozen: &FrozenSources,
        section: impl FnOnce(ForkBase<'_, S>) -> R,
    ) -> R {
        let origins = self.origins.as_ref().map(RefCell::borrow);
        let native = match &self.native_current {
            Some((models, NativeCurrent::Frame(cache))) => Some((*models, cache.borrow())),
            _ => None,
        };
        section(ForkBase {
            static_sources: self.static_sources,
            frozen,
            origins: origins.as_deref().map(SourceTransaction::catalogue),
            native: native.as_ref().map(|(models, cache)| (*models, &**cache)),
        })
    }

    /// Apply one fork's keyed changes: every cache entry it added or changed, its source
    /// bindings and Current proofs. Returns its ordered logs (requirements, failures) for
    /// [`Self::append_fork_logs`], which the frame appends group by group in order.
    #[allow(clippy::type_complexity)]
    pub(super) fn apply_fork(
        &self,
        changes: SourcesChanges,
    ) -> Result<(Vec<CurrentResolutionRequirement>, Vec<TransitionError>), IntentError> {
        self.captured_families
            .borrow_mut()
            .apply(changes.captured_families);
        self.current_occurrences
            .borrow_mut()
            .apply(changes.current_occurrences);
        self.current.borrow_mut().apply(changes.current);
        if let (Some(origins), Some(changes)) = (&self.origins, changes.origins) {
            origins.borrow_mut().frame()?.apply_overlay(changes)?;
        }
        if let (Some((_, NativeCurrent::Frame(cache))), Some(changes)) =
            (&self.native_current, changes.native)
        {
            cache.borrow_mut().apply(changes);
        }
        Ok((changes.requirements, changes.failure))
    }

    /// Append one fork group's requirements and failure, as that group would have recorded them
    /// in turn: requirements once per target and address, the first failure only.
    pub(super) fn append_fork_logs(
        &self,
        requirements: &[CurrentResolutionRequirement],
        failures: &[TransitionError],
    ) {
        self.requirements.borrow_mut().apply(requirements.to_vec());
        if let Some(failure) = failures.first() {
            self.remember_failure(failure.clone());
        }
    }

    /// The lengths of a fork's requirement and failure logs over its finished groups.
    pub(super) fn log_lengths(&self) -> (usize, usize) {
        (
            self.requirements.borrow().chunk_len(),
            self.failure.borrow().chunk_len(),
        )
    }
}

impl<'f, S: DynamicTickSource> ForkBase<'f, S> {
    /// One worker's sources: the frame's answers so far, its own adoption.
    pub(super) fn fork(self, adopt: &'f CurrentAdoption<'f>) -> CapturedProgrammingSources<'f, S> {
        CapturedProgrammingSources {
            static_sources: self.static_sources,
            adopt,
            presets: None,
            origins: self
                .origins
                .map(|origins| RefCell::new(SourceTransaction::Fork(OriginsOverlay::new(origins)))),
            captured_families: RefCell::new(Memo::fork(&self.frozen.captured_families)),
            current_occurrences: RefCell::new(Memo::fork(&self.frozen.current_occurrences)),
            current: RefCell::new(Memo::fork(&self.frozen.current)),
            failure: RefCell::new(Log::fork(&self.frozen.failure)),
            requirements: RefCell::new(Log::fork(&self.frozen.requirements)),
            native_current: self.native.map(|(models, cache)| {
                (
                    models,
                    NativeCurrent::Fork(RefCell::new(NativeCurrentFork::new(cache))),
                )
            }),
        }
    }
}

impl<S> CapturedProgrammingSources<'_, S> {
    /// Replace the frame's recorded failure (tests inject and clear one).
    #[cfg(test)]
    pub(super) fn set_failure(&self, failure: Option<TransitionError>) {
        let mut log = self.failure.borrow_mut();
        log.take_own();
        log.restore_own(failure.into_iter().collect());
    }

    /// A fork's group finished: keep what it added.
    pub(super) fn commit_group(&self) {
        self.captured_families.borrow_mut().commit_group();
        self.current_occurrences.borrow_mut().commit_group();
        self.current.borrow_mut().commit_group();
        self.failure.borrow_mut().commit_group();
        self.requirements.borrow_mut().commit_group();
        if let Some(origins) = &self.origins
            && let SourceTransaction::Fork(overlay) = &mut *origins.borrow_mut()
        {
            overlay.commit_group();
        }
        if let Some((_, NativeCurrent::Fork(fork))) = &self.native_current {
            fork.borrow_mut().commit_group();
        }
    }

    /// A fork's group is rerun by the frame: forget everything it added.
    pub(super) fn abort_group(&self) {
        self.captured_families.borrow_mut().abort_group();
        self.current_occurrences.borrow_mut().abort_group();
        self.current.borrow_mut().abort_group();
        self.failure.borrow_mut().abort_group();
        self.requirements.borrow_mut().abort_group();
        if let Some(origins) = &self.origins
            && let SourceTransaction::Fork(overlay) = &mut *origins.borrow_mut()
        {
            overlay.abort_group();
        }
        if let Some((_, NativeCurrent::Fork(fork))) = &self.native_current {
            fork.borrow_mut().abort_group();
        }
    }

    /// A fork's changes, for [`CapturedProgrammingSources::apply_fork`].
    pub(super) fn into_changes(self) -> SourcesChanges {
        SourcesChanges {
            captured_families: self.captured_families.into_inner().into_chunk(),
            current_occurrences: self.current_occurrences.into_inner().into_chunk(),
            current: self.current.into_inner().into_chunk(),
            failure: self.failure.into_inner().into_chunk(),
            requirements: self.requirements.into_inner().into_chunk(),
            origins: self.origins.and_then(|origins| match origins.into_inner() {
                SourceTransaction::Fork(overlay) => Some(overlay.into_changes()),
                SourceTransaction::Frame(_) => None,
            }),
            native: self.native_current.and_then(|(_, native)| match native {
                NativeCurrent::Fork(fork) => Some(fork.into_inner().into_changes()),
                NativeCurrent::Frame(_) => None,
            }),
        }
    }
}
