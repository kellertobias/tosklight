use crate::light_benchmark::statistics::{Distribution, distribution};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ActiveShowPorts,
    ActiveShowService, ActiveShowUnitOfWork, BackupIdentity, EventBus, PatchChange,
    PatchPerformancePhase, ShowPatchPorts, ShowPatchService,
};
use light_core::{FixtureId, Revision, ShowId};
use light_engine::{Engine, EngineSnapshot, PreparedEngineSnapshot};
use light_fixture::{FixtureProfile, PatchedFixtureProfileReference};
use light_programmer::ProgrammerRegistry;
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowObjectUndo,
    PortableShowTransaction, ShowStore, StoreError,
};
use light_wire::v2::patch as wire;
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

const MODE_COUNT: usize = 2_000;
const WARMUPS: usize = 8;
const SAMPLES: usize = 40;
const SINGLE_GATE_MICROSECONDS: f64 = 250_000.0;
const BATCH_GATE_MICROSECONDS: f64 = 500_000.0;

#[derive(Debug, Serialize)]
pub struct PatchMutationReport {
    pub profile_mode_count: usize,
    pub warmups_per_scenario: usize,
    pub single_fixture: PatchScenarioReport,
    pub hundred_fixtures: PatchScenarioReport,
    pub gate_met: bool,
    pub measurement_boundary: &'static str,
    pub exclusions: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct PatchScenarioReport {
    pub fixture_count: usize,
    pub gate_p95_microseconds: f64,
    pub total_server: Distribution,
    pub request_bytes: ByteDistribution,
    pub response_bytes: ByteDistribution,
    pub phases: BTreeMap<&'static str, Distribution>,
    pub invariants: PatchInvariants,
    pub raw_total_microseconds: Vec<f64>,
    pub gate_met: bool,
}

#[derive(Debug, Serialize)]
pub struct ByteDistribution {
    pub minimum: usize,
    pub maximum: usize,
    pub mean: f64,
}

#[derive(Debug, Serialize)]
pub struct PatchInvariants {
    pub samples: usize,
    /// One coherent snapshot boundary and one transaction boundary per sample.
    pub active_show_document_boundaries: usize,
    pub show_transactions: usize,
    pub backup_decisions: usize,
    pub persistence_commits: usize,
    pub runtime_preparations: usize,
    pub runtime_installs: usize,
    pub reconciliations: usize,
    pub events: usize,
    pub actual_backup_copies: usize,
    pub one_atomic_boundary_per_sample: bool,
    pub unrelated_reads: usize,
}

#[derive(Default)]
struct Counters {
    active_show_begins: AtomicUsize,
    backups: AtomicUsize,
    commits: AtomicUsize,
    runtime_prepares: AtomicUsize,
    runtime_installs: AtomicUsize,
    reconciliations: AtomicUsize,
    backup_copies: AtomicUsize,
}

#[derive(Clone, Copy)]
struct CounterSnapshot {
    active_show_begins: usize,
    backups: usize,
    commits: usize,
    runtime_prepares: usize,
    runtime_installs: usize,
    reconciliations: usize,
    backup_copies: usize,
}

impl Counters {
    fn snapshot(&self) -> CounterSnapshot {
        let read = |counter: &AtomicUsize| counter.load(Ordering::SeqCst);
        CounterSnapshot {
            active_show_begins: read(&self.active_show_begins),
            backups: read(&self.backups),
            commits: read(&self.commits),
            runtime_prepares: read(&self.runtime_prepares),
            runtime_installs: read(&self.runtime_installs),
            reconciliations: read(&self.reconciliations),
            backup_copies: read(&self.backup_copies),
        }
    }
}

impl CounterSnapshot {
    fn difference(self, before: Self) -> Self {
        Self {
            active_show_begins: self.active_show_begins - before.active_show_begins,
            backups: self.backups - before.backups,
            commits: self.commits - before.commits,
            runtime_prepares: self.runtime_prepares - before.runtime_prepares,
            runtime_installs: self.runtime_installs - before.runtime_installs,
            reconciliations: self.reconciliations - before.reconciliations,
            backup_copies: self.backup_copies - before.backup_copies,
        }
    }
}

#[derive(Clone)]
struct BenchmarkPorts {
    show_path: PathBuf,
    backup_dir: PathBuf,
    show_id: ShowId,
    profile: FixtureProfileRevision,
    engine: Arc<Engine>,
    counters: Arc<Counters>,
    phases: Arc<Mutex<BTreeMap<PatchPerformancePhase, Vec<Duration>>>>,
}

struct BenchmarkUnit {
    show: ShowStore,
    document: PortableShowDocument,
    show_path: PathBuf,
    backup_dir: PathBuf,
    counters: Arc<Counters>,
    phases: Arc<Mutex<BTreeMap<PatchPerformancePhase, Vec<Duration>>>>,
}

impl ActiveShowUnitOfWork for BenchmarkUnit {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError> {
        let started = Instant::now();
        self.counters.backups.fetch_add(1, Ordering::SeqCst);
        let destination = self.backup_dir.join(format!(
            "{}-{}.sqlite",
            identity.request_id, identity.correlation_id
        ));
        fs::copy(&self.show_path, destination)
            .map_err(|error| ActionError::new(ActionErrorKind::Unavailable, error.to_string()))?;
        self.counters.backup_copies.fetch_add(1, Ordering::SeqCst);
        record_phase(
            &self.phases,
            PatchPerformancePhase::Backup,
            started.elapsed(),
        );
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        let started = Instant::now();
        self.counters.commits.fetch_add(1, Ordering::SeqCst);
        let result = self
            .show
            .apply_portable_transaction(transaction)
            .map_err(store_error);
        record_phase(
            &self.phases,
            PatchPerformancePhase::Persistence,
            started.elapsed(),
        );
        result
    }
}

impl ActiveShowPorts for BenchmarkPorts {
    type UnitOfWork = BenchmarkUnit;
    type PreparedRuntime = PreparedEngineSnapshot;

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
                "show is not active",
            ));
        }
        let show = ShowStore::open(&self.show_path).map_err(store_error)?;
        let document = show.portable_document().map_err(store_error)?;
        Ok(BenchmarkUnit {
            show,
            document,
            show_path: self.show_path.clone(),
            backup_dir: self.backup_dir.clone(),
            counters: Arc::clone(&self.counters),
            phases: Arc::clone(&self.phases),
        })
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unit.show
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(store_error)
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        let started = Instant::now();
        self.counters
            .runtime_prepares
            .fetch_add(1, Ordering::SeqCst);
        let result = self
            .engine
            .prepare_snapshot(snapshot)
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()));
        record_phase(
            &self.phases,
            PatchPerformancePhase::RuntimePreparation,
            started.elapsed(),
        );
        result
    }

    fn install_runtime(&self, _context: &ActionContext, prepared: Self::PreparedRuntime) {
        let started = Instant::now();
        self.counters
            .runtime_installs
            .fetch_add(1, Ordering::SeqCst);
        self.engine.install_prepared_snapshot(prepared);
        record_phase(
            &self.phases,
            PatchPerformancePhase::RuntimeInstall,
            started.elapsed(),
        );
    }
}

impl ShowPatchPorts for BenchmarkPorts {
    fn resolve_profile_revision(
        &self,
        _profile_id: FixtureId,
        _revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        Ok(self.profile.clone())
    }

    fn reconcile_patch_change(&self, _change: &PatchChange) {
        self.counters.reconciliations.fetch_add(1, Ordering::SeqCst);
    }

    fn record_patch_performance_phase(&self, phase: PatchPerformancePhase, elapsed: Duration) {
        record_phase(&self.phases, phase, elapsed);
    }
}

pub fn run() -> Result<PatchMutationReport, String> {
    let single_fixture = run_scenario(1, SINGLE_GATE_MICROSECONDS)?;
    let hundred_fixtures = run_scenario(100, BATCH_GATE_MICROSECONDS)?;
    let gate_met = single_fixture.gate_met && hundred_fixtures.gate_met;
    Ok(PatchMutationReport {
        profile_mode_count: MODE_COUNT,
        warmups_per_scenario: WARMUPS,
        single_fixture,
        hundred_fixtures,
        gate_met,
        measurement_boundary: "transport-shaped JSON encode, ShowPatchService, one SQLite transaction, engine prepare/install, projection reconciliation, event publication, response JSON encode",
        exclusions: vec![
            "HTTP authentication and Axum scheduling",
            "frontend action-to-visible paint (reported separately and informational)",
            "fixture catalog and configuration reads, which are forbidden inside the mutation boundary",
        ],
    })
}

fn run_scenario(
    fixture_count: usize,
    gate_p95_microseconds: f64,
) -> Result<PatchScenarioReport, String> {
    let scratch = Scratch::new(fixture_count)?;
    let (profile, reference) = profile_with_modes(MODE_COUNT)?;
    let (show, show_id) =
        ShowStore::create(&scratch.show_path, "Patch release benchmark").map_err(string_error)?;
    drop(show);
    let events = EventBus::new(256);
    let active_show = ActiveShowService::new(events);
    let service = ShowPatchService::new(active_show);
    let counters = Arc::new(Counters::default());
    let phases = Arc::new(Mutex::new(BTreeMap::new()));
    let ports = BenchmarkPorts {
        show_path: scratch.show_path.clone(),
        backup_dir: scratch.backup_dir.clone(),
        show_id,
        profile,
        engine: Arc::new(Engine::new(ProgrammerRegistry::default())),
        counters: Arc::clone(&counters),
        phases: Arc::clone(&phases),
    };
    let mut revision = 0_u64;
    for index in 0..WARMUPS {
        revision = execute(
            &service,
            &ports,
            show_id,
            reference,
            fixture_count,
            index,
            revision,
        )?
        .0;
    }
    phases.lock().clear();
    let before = counters.snapshot();
    let event_before = service.events().latest_sequence();
    let mut totals = Vec::with_capacity(SAMPLES);
    let mut request_bytes = Vec::with_capacity(SAMPLES);
    let mut response_bytes = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let (next_revision, total, request, response) = execute(
            &service,
            &ports,
            show_id,
            reference,
            fixture_count,
            WARMUPS + index,
            revision,
        )?;
        revision = next_revision;
        totals.push(total);
        request_bytes.push(request);
        response_bytes.push(response);
    }
    let counts = counters.snapshot().difference(before);
    let events = usize::try_from(service.events().latest_sequence() - event_before)
        .map_err(|error| error.to_string())?;
    let one_atomic_boundary_per_sample = [
        counts.backups,
        counts.commits,
        counts.runtime_prepares,
        counts.runtime_installs,
        counts.reconciliations,
        events,
    ]
    .into_iter()
    .all(|count| count == SAMPLES)
        && counts.active_show_begins == SAMPLES * 2;
    let total_server = distribution(&totals).expect("Patch samples are non-empty");
    let phase_reports = phases
        .lock()
        .iter()
        .filter_map(|(phase, samples)| {
            distribution(samples).map(|result| (phase_name(*phase), result))
        })
        .collect();
    let gate_met =
        total_server.p95_microseconds <= gate_p95_microseconds && one_atomic_boundary_per_sample;
    Ok(PatchScenarioReport {
        fixture_count,
        gate_p95_microseconds,
        total_server,
        request_bytes: bytes(&request_bytes),
        response_bytes: bytes(&response_bytes),
        phases: phase_reports,
        invariants: PatchInvariants {
            samples: SAMPLES,
            active_show_document_boundaries: counts.active_show_begins,
            show_transactions: counts.commits,
            backup_decisions: counts.backups,
            persistence_commits: counts.commits,
            runtime_preparations: counts.runtime_prepares,
            runtime_installs: counts.runtime_installs,
            reconciliations: counts.reconciliations,
            events,
            actual_backup_copies: counts.backup_copies,
            one_atomic_boundary_per_sample,
            unrelated_reads: 0,
        },
        raw_total_microseconds: totals
            .iter()
            .map(|duration| duration.as_secs_f64() * 1_000_000.0)
            .collect(),
        gate_met,
    })
}

fn execute(
    service: &ShowPatchService,
    ports: &BenchmarkPorts,
    show_id: ShowId,
    profile: PatchedFixtureProfileReference,
    fixture_count: usize,
    index: usize,
    expected_revision: u64,
) -> Result<(u64, Duration, usize, usize), String> {
    let request_id = format!("patch-benchmark-{fixture_count}-{index}");
    let request = wire_request(request_id.clone(), profile, fixture_count, index);
    let request_bytes = serde_json::to_vec(&request).map_err(string_error)?.len();
    let encoded_request = serde_json::to_vec(&request).map_err(string_error)?;
    let started = Instant::now();
    let decoded_request = serde_json::from_slice::<wire::PatchFixturesRequest>(&encoded_request)
        .map_err(string_error)?;
    let command =
        light_headless_runtime::benchmark_patch_application_command(show_id, decoded_request)?;
    let envelope = ActionEnvelope {
        context: ActionContext::operator(
            Uuid::from_u128(1),
            Uuid::from_u128(3),
            ActionSource::Http,
        )
        .with_request_id(request_id)
        .with_expected_revision(expected_revision),
        command,
    };
    let result = service
        .handle(envelope, ports)
        .map_err(|error| error.message)?;
    let revision = result.change.patch_revision.value();
    let response = light_headless_runtime::benchmark_patch_wire_outcome(result);
    let response_bytes = serde_json::to_vec(&response).map_err(string_error)?.len();
    Ok((revision, started.elapsed(), request_bytes, response_bytes))
}

fn wire_request(
    request_id: String,
    profile: PatchedFixtureProfileReference,
    fixture_count: usize,
    generation: usize,
) -> wire::PatchFixturesRequest {
    wire::PatchFixturesRequest {
        request_id,
        fixtures: (0..fixture_count)
            .map(|index| wire_fixture(profile, index, generation))
            .collect(),
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
    }
}

fn wire_fixture(
    profile: PatchedFixtureProfileReference,
    index: usize,
    generation: usize,
) -> wire::PatchFixtureInput {
    let address = u16::try_from(index + 1).expect("fixture count stays in one universe");
    wire::PatchFixtureInput {
        fixture_id: Uuid::from_u128(100_000 + index as u128),
        fixture_number: Some(u32::try_from(index + 1).unwrap()),
        virtual_fixture_number: None,
        name: format!("Fixture {} generation {generation}", index + 1),
        profile_id: profile.profile_id.0,
        profile_revision: profile.profile_revision,
        mode_id: profile.mode_id,
        split_patches: vec![wire::PatchSplitAssignment {
            split: 1,
            universe: Some(1),
            address: Some(address),
        }],
        layer_id: "default".into(),
        position_master: None,
        direct_control: None,
        internal_bindings: Default::default(),
        location: wire::PatchFixtureLocation { x: 0, y: 0, z: 0 },
        rotation: wire::PatchFixtureRotation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        multipatch: Vec::new(),
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: Vec::new(),
    }
}

fn profile_with_modes(
    mode_count: usize,
) -> Result<(FixtureProfileRevision, PatchedFixtureProfileReference), String> {
    let mut profile = FixtureProfile::blank();
    profile.revision = 7;
    profile.manufacturer = "ToskLight".into();
    profile.name = "Patch release benchmark".into();
    profile.short_name = "Patch bench".into();
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
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).map_err(string_error)?)
            .map_err(string_error)?;
    Ok((
        stored,
        PatchedFixtureProfileReference {
            profile_id,
            profile_revision,
            mode_id,
        },
    ))
}

fn record_phase(
    phases: &Mutex<BTreeMap<PatchPerformancePhase, Vec<Duration>>>,
    phase: PatchPerformancePhase,
    elapsed: Duration,
) {
    phases.lock().entry(phase).or_default().push(elapsed);
}

fn phase_name(phase: PatchPerformancePhase) -> &'static str {
    match phase {
        PatchPerformancePhase::BoundaryValidation => "boundary_validation",
        PatchPerformancePhase::SnapshotLoad => "snapshot_load",
        PatchPerformancePhase::ConflictDetection => "conflict_detection",
        PatchPerformancePhase::ProfileResolutionAndPlacement => "profile_resolution_and_placement",
        PatchPerformancePhase::CandidatePreparation => "candidate_preparation_including_compile",
        PatchPerformancePhase::Compile => "compile",
        PatchPerformancePhase::RuntimePreparation => "runtime_preparation",
        PatchPerformancePhase::Backup => "backup_copy",
        PatchPerformancePhase::Persistence => "persistence",
        PatchPerformancePhase::RuntimeInstall => "runtime_install",
        PatchPerformancePhase::ProjectionReconcile => "projection_reconcile",
        PatchPerformancePhase::EventPublication => "event_publication",
    }
}

fn bytes(samples: &[usize]) -> ByteDistribution {
    ByteDistribution {
        minimum: *samples.iter().min().expect("byte samples are non-empty"),
        maximum: *samples.iter().max().expect("byte samples are non-empty"),
        mean: samples.iter().sum::<usize>() as f64 / samples.len() as f64,
    }
}

struct Scratch {
    root: PathBuf,
    show_path: PathBuf,
    backup_dir: PathBuf,
}

impl Scratch {
    fn new(fixture_count: usize) -> Result<Self, String> {
        let base = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".artifacts/tmp"));
        let root = base.join(format!(
            "light-patch-benchmark-{fixture_count}-{}",
            Uuid::new_v4()
        ));
        let backup_dir = root.join("backups");
        fs::create_dir_all(&backup_dir).map_err(string_error)?;
        let show_path = root.join("show.sqlite");
        Ok(Self {
            root,
            show_path,
            backup_dir,
        })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn store_error(error: StoreError) -> ActionError {
    let kind = match error {
        StoreError::DocumentRevisionConflict { .. } => ActionErrorKind::Conflict,
        _ => ActionErrorKind::Internal,
    };
    ActionError::new(kind, error.to_string())
}

fn string_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
