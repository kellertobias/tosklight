use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, EventBus,
    PatchChange, PatchFixtureCandidate, PatchFixturesCommand, ShowPatchPorts, ShowPatchService,
};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::EngineSnapshot;
use light_fixture::{
    FixtureLocation, FixtureProfile, FixtureVector, PatchedFixturePatch,
    PatchedFixtureProfileReference, SplitPatch,
};
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowTransaction,
    ShowStore, StoreError,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePoint {
    None,
    Backup,
    Commit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CounterSnapshot {
    pub active_show_begins: usize,
    pub library_reads: usize,
    pub catalog_reads: usize,
    pub runtime_prepares: usize,
    pub backups: usize,
    pub commits: usize,
    pub written_fixtures: usize,
    pub written_profiles: usize,
    pub runtime_installs: usize,
    pub reconciliations: usize,
}

#[derive(Default)]
struct Counters {
    active_show_begins: AtomicUsize,
    library_reads: AtomicUsize,
    catalog_reads: AtomicUsize,
    runtime_prepares: AtomicUsize,
    backups: AtomicUsize,
    commits: AtomicUsize,
    written_fixtures: AtomicUsize,
    written_profiles: AtomicUsize,
    runtime_installs: AtomicUsize,
    reconciliations: AtomicUsize,
}

impl Counters {
    fn snapshot(&self) -> CounterSnapshot {
        let read = |counter: &AtomicUsize| counter.load(Ordering::SeqCst);
        CounterSnapshot {
            active_show_begins: read(&self.active_show_begins),
            library_reads: read(&self.library_reads),
            catalog_reads: read(&self.catalog_reads),
            runtime_prepares: read(&self.runtime_prepares),
            backups: read(&self.backups),
            commits: read(&self.commits),
            written_fixtures: read(&self.written_fixtures),
            written_profiles: read(&self.written_profiles),
            runtime_installs: read(&self.runtime_installs),
            reconciliations: read(&self.reconciliations),
        }
    }
}

pub struct TestRig {
    pub service: ShowPatchService,
    pub ports: CounterPorts,
}

impl TestRig {
    pub fn new(profile: FixtureProfileRevision, failure: FailurePoint) -> Self {
        Self {
            service: ShowPatchService::new(EventBus::new(32)),
            ports: CounterPorts::new(profile, failure),
        }
    }

    pub fn counters(&self) -> CounterSnapshot {
        self.ports.counters.snapshot()
    }

    pub fn assert_portable_patch(&self, fixtures: usize, profiles: usize) {
        let show = ShowStore::open(&self.ports.path).unwrap();
        let document = show.portable_document().unwrap();
        let stored = document
            .objects_of_kind("patched_fixture")
            .collect::<Vec<_>>();
        assert_eq!(stored.len(), fixtures);
        assert_eq!(document.fixture_profile_revisions().len(), profiles);
        assert!(stored.iter().all(|object| {
            object.body().get("definition").is_none()
                && object.body().get("profile_id").is_some()
                && object.body().get("profile_revision").is_some()
                && object.body().get("mode_id").is_some()
        }));
    }

    pub fn assert_empty_show(&self) {
        let show = ShowStore::open(&self.ports.path).unwrap();
        let document = show.portable_document().unwrap();
        assert_eq!(document.revision().value(), 0);
        assert_eq!(document.patch_revision().value(), 0);
        assert_eq!(document.objects_of_kind("patched_fixture").count(), 0);
        assert!(document.fixture_profile_revisions().is_empty());
    }

    pub fn seed_legacy_fixture(
        &self,
        profile: &FixtureProfileRevision,
        reference: PatchedFixtureProfileReference,
        patch: &PatchedFixturePatch,
    ) {
        self.seed_legacy_fixture_as(profile, reference, patch, &patch.fixture_id.0.to_string());
    }

    pub fn seed_legacy_fixture_as(
        &self,
        profile: &FixtureProfileRevision,
        reference: PatchedFixtureProfileReference,
        patch: &PatchedFixturePatch,
        object_id: &str,
    ) {
        let typed = serde_json::from_value::<FixtureProfile>(profile.profile().clone()).unwrap();
        let definition = typed.resolved_definition(reference.mode_id).unwrap();
        let mut body = serde_json::to_value(patch).unwrap();
        body.as_object_mut().unwrap().insert(
            "definition".into(),
            serde_json::to_value(definition).unwrap(),
        );
        body["definition"]["profile_snapshot"] = profile.profile().clone();
        let show = ShowStore::open(&self.ports.path).unwrap();
        show.put_object("patched_fixture", object_id, &body, 0)
            .unwrap();
    }

    pub fn portable_document(&self) -> PortableShowDocument {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }
}

pub struct CounterPorts {
    path: PathBuf,
    show_id: ShowId,
    profile: FixtureProfileRevision,
    failure: FailurePoint,
    counters: Arc<Counters>,
}

impl CounterPorts {
    fn new(profile: FixtureProfileRevision, failure: FailurePoint) -> Self {
        let path = temporary_show_path();
        let (show, show_id) = ShowStore::create(&path, "Patch service counter test").unwrap();
        drop(show);
        Self {
            path,
            show_id,
            profile,
            failure,
            counters: Arc::new(Counters::default()),
        }
    }

    pub const fn show_id(&self) -> ShowId {
        self.show_id
    }
}

impl Drop for CounterPorts {
    fn drop(&mut self) {
        remove_sqlite_files(&self.path);
    }
}

pub struct CounterUnitOfWork {
    show: ShowStore,
    document: PortableShowDocument,
    failure: FailurePoint,
    counters: Arc<Counters>,
}

impl super::super::ActiveShowUnitOfWork for CounterUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &super::super::BackupIdentity) -> Result<(), ActionError> {
        self.counters.backups.fetch_add(1, Ordering::SeqCst);
        if self.failure == FailurePoint::Backup {
            Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "injected backup failure",
            ))
        } else {
            Ok(())
        }
    }

    fn commit(
        self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.counters.commits.fetch_add(1, Ordering::SeqCst);
        if self.failure == FailurePoint::Commit {
            return Err(ActionError::new(
                ActionErrorKind::Internal,
                "injected commit failure",
            ));
        }
        let commit = self
            .show
            .apply_portable_transaction(transaction)
            .map_err(store_error)?;
        let written = commit
            .written_objects()
            .iter()
            .filter(|object| object.key().kind() == "patched_fixture")
            .count();
        self.counters
            .written_fixtures
            .fetch_add(written, Ordering::SeqCst);
        self.counters
            .written_profiles
            .fetch_add(commit.fixture_profile_revisions().len(), Ordering::SeqCst);
        Ok(commit)
    }
}

impl ShowPatchPorts for CounterPorts {
    type UnitOfWork = CounterUnitOfWork;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.counters
            .active_show_begins
            .fetch_add(1, Ordering::SeqCst);
        if show_id != self.show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not active",
            ));
        }
        let show = ShowStore::open(&self.path).map_err(store_error)?;
        let document = show.portable_document().map_err(store_error)?;
        Ok(CounterUnitOfWork {
            show,
            document,
            failure: self.failure,
            counters: Arc::clone(&self.counters),
        })
    }

    fn resolve_profile_revision(
        &self,
        _profile_id: FixtureId,
        _revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        self.counters.library_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.profile.clone())
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.counters
            .runtime_prepares
            .fetch_add(1, Ordering::SeqCst);
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _prepared: Self::PreparedRuntime) {
        self.counters
            .runtime_installs
            .fetch_add(1, Ordering::SeqCst);
    }

    fn reconcile_patch_change(&self, _change: &PatchChange) {
        self.counters.reconciliations.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn profile_with_modes(
    mode_count: usize,
) -> (FixtureProfileRevision, PatchedFixtureProfileReference) {
    let mut profile = FixtureProfile::blank();
    profile.revision = 7;
    profile.manufacturer = "Acme".into();
    profile.name = "Large mode fixture".into();
    profile.short_name = "Large".into();
    let template = profile.modes[0].clone();
    profile.modes = (0..mode_count)
        .map(|index| {
            let mut mode = template.clone();
            mode.id = Uuid::from_u128(10_000 + index as u128);
            mode.name = format!("Mode {index}");
            mode
        })
        .collect();
    let mode_id = profile.modes[mode_count / 2].id;
    let profile_id = profile.id;
    let profile_revision = Revision::from(profile.revision);
    let stored =
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap()).unwrap();
    (
        stored,
        PatchedFixtureProfileReference {
            profile_id,
            profile_revision,
            mode_id,
        },
    )
}

pub fn patch_batch(
    show_id: ShowId,
    profile: PatchedFixtureProfileReference,
    fixture_count: usize,
) -> PatchFixturesCommand {
    PatchFixturesCommand {
        show_id,
        fixtures: (0..fixture_count)
            .map(|index| fixture_candidate(profile, index))
            .collect(),
        remove_fixture_ids: Vec::new(),
    }
}

pub fn envelope(
    command: PatchFixturesCommand,
    request_id: &str,
    expected_revision: u64,
) -> ActionEnvelope<PatchFixturesCommand> {
    ActionEnvelope {
        context: ActionContext::operator(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            ActionSource::Http,
        )
        .with_request_id(request_id)
        .with_expected_revision(expected_revision),
        command,
    }
}

fn fixture_candidate(
    profile: PatchedFixtureProfileReference,
    index: usize,
) -> PatchFixtureCandidate {
    let address = u16::try_from(index + 1).unwrap();
    PatchFixtureCandidate {
        profile,
        patch: PatchedFixturePatch {
            fixture_id: FixtureId(Uuid::from_u128(100_000 + index as u128)),
            fixture_number: Some(u32::try_from(index + 1).unwrap()),
            virtual_fixture_number: None,
            name: format!("Fixture {}", index + 1),
            universe: Some(1),
            address: Some(address),
            split_patches: vec![SplitPatch {
                split: 1,
                universe: Some(1),
                address: Some(address),
            }],
            layer_id: "default".into(),
            direct_control: None,
            location: FixtureLocation::default(),
            rotation: FixtureVector::default(),
            logical_heads: Vec::new(),
            multipatch: Vec::new(),
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        },
    }
}

fn temporary_show_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "light-show-patch-counter-{}.sqlite",
        Uuid::new_v4()
    ))
}

fn remove_sqlite_files(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-shm", "-wal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}

fn store_error(error: StoreError) -> ActionError {
    let kind = match error {
        StoreError::DocumentRevisionConflict { .. } => ActionErrorKind::Conflict,
        _ => ActionErrorKind::Internal,
    };
    ActionError::new(kind, error.to_string())
}
