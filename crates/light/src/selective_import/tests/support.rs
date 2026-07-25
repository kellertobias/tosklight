use super::super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    ActiveShowObjectChange, ActiveShowPorts, ActiveShowService, ActiveShowUnitOfWork,
    BackupIdentity,
};
use light_core::{FixtureId, ShowId};
use light_engine::EngineSnapshot;
use light_fixture::{
    FixtureHead, FixtureProfile, MultiPatchInstance, PatchedFixture, PatchedHead,
    PortablePatchedFixtureRecord,
};
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowObjectUndo,
    PortableShowTransaction, ShowStore, StoreError,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

pub(super) struct TestRig {
    pub service: SelectiveShowImportService,
    pub active_show: ActiveShowService,
    pub ports: TestPorts,
    source_path: PathBuf,
    target_path: PathBuf,
    pub source_id: ShowId,
    pub target_id: ShowId,
}

impl TestRig {
    pub fn new() -> Self {
        let source_path = temporary_path("source");
        let target_path = temporary_path("target");
        let (_, source_id) = ShowStore::create(&source_path, "Source").unwrap();
        let (_, target_id) = ShowStore::create(&target_path, "Target").unwrap();
        let active_show = ActiveShowService::default();
        Self {
            service: SelectiveShowImportService::new(active_show.clone()),
            active_show,
            ports: TestPorts::new(
                source_path.clone(),
                target_path.clone(),
                source_id,
                target_id,
            ),
            source_path,
            target_path,
            source_id,
            target_id,
        }
    }

    pub fn source_object(&self, kind: &str, id: &str, body: Value) {
        put_object(&self.source_path, kind, id, body);
    }

    pub fn update_source_object(&self, kind: &str, id: &str, body: Value) {
        let current = document(&self.source_path)
            .object(kind, id)
            .expect("source object")
            .revision();
        ShowStore::open(&self.source_path)
            .unwrap()
            .put_object(kind, id, &body, current)
            .unwrap();
    }

    pub fn target_object(&self, kind: &str, id: &str, body: Value) {
        put_object(&self.target_path, kind, id, body);
    }

    pub fn source_profile(&self, profile: &FixtureProfileRevision) {
        insert_profile(&self.source_path, profile);
    }

    pub fn asset_action(&self, asset: crate::AssetReference, action: ImportManagedAssetAction) {
        self.ports.asset_actions.lock().insert(asset, action);
    }

    pub fn asset_steps(&self) -> Vec<&'static str> {
        self.ports.asset_steps.lock().clone()
    }

    pub fn pause_next_runtime_prepare(&self) {
        let mut state = self.ports.prepare_gate.state.lock().unwrap();
        *state = PrepareGateState {
            armed: true,
            entered: false,
            released: false,
        };
    }

    pub fn wait_for_runtime_prepare(&self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut state = self.ports.prepare_gate.state.lock().unwrap();
        while !state.entered {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .expect("runtime preparation did not reach the test gate");
            let (next, timeout) = self
                .ports
                .prepare_gate
                .changed
                .wait_timeout(state, remaining)
                .unwrap();
            state = next;
            assert!(
                !timeout.timed_out(),
                "runtime preparation test gate timed out"
            );
        }
    }

    pub fn release_runtime_prepare(&self) {
        let mut state = self.ports.prepare_gate.state.lock().unwrap();
        state.released = true;
        self.ports.prepare_gate.changed.notify_all();
    }

    pub fn pause_next_source_snapshot(&self) {
        let mut state = self.ports.source_gate.state.lock().unwrap();
        *state = PrepareGateState {
            armed: true,
            entered: false,
            released: false,
        };
    }

    pub fn wait_for_source_snapshot(&self) {
        wait_for_gate(&self.ports.source_gate, "source snapshot");
    }

    pub fn release_source_snapshot(&self) {
        release_gate(&self.ports.source_gate);
    }

    pub fn pause_next_import_descriptor(&self) {
        let mut state = self.ports.descriptor_gate.state.lock().unwrap();
        *state = PrepareGateState {
            armed: true,
            entered: false,
            released: false,
        };
    }

    pub fn wait_for_import_descriptor(&self) {
        wait_for_gate(&self.ports.descriptor_gate, "import descriptor");
    }

    pub fn release_import_descriptor(&self) {
        release_gate(&self.ports.descriptor_gate);
    }

    pub fn target_profile(&self, profile: &FixtureProfileRevision) {
        insert_profile(&self.target_path, profile);
    }

    pub fn request(&self, kind: &str, id: &str) -> SelectiveShowImportRequest {
        SelectiveShowImportRequest::new(
            self.source_id,
            self.target_id,
            [light_show::PortableShowObjectKey::new(kind, id)],
        )
    }

    pub fn preview(&self, request: SelectiveShowImportRequest) -> SelectiveShowImportPreview {
        self.service
            .preview(&context(), request, &self.ports)
            .unwrap()
    }

    pub fn apply(
        &self,
        preview: &SelectiveShowImportPreview,
    ) -> Result<SelectiveShowImportResult, ActionError> {
        self.service.apply(
            ActionEnvelope {
                context: context(),
                command: ApplySelectiveShowImportCommand {
                    request: preview.request.clone(),
                    expected_source_revision: preview.source_revision,
                    expected_target_revision: preview.target_revision,
                },
            },
            &self.ports,
        )
    }

    pub fn target_document(&self) -> PortableShowDocument {
        document(&self.target_path)
    }

    pub fn clear_steps(&self) {
        self.ports.steps.lock().clear();
    }

    pub fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        remove_sqlite_files(&self.source_path);
        remove_sqlite_files(&self.target_path);
    }
}

pub(super) struct TestPorts {
    source_path: PathBuf,
    target_path: PathBuf,
    source_id: ShowId,
    target_id: ShowId,
    pub steps: Arc<Mutex<Vec<&'static str>>>,
    pub fail_prepare: AtomicBool,
    pub fail_commit: Arc<AtomicBool>,
    pub installed: Mutex<Option<EngineSnapshot>>,
    pub reconciled: Mutex<Vec<SelectiveShowImportChange>>,
    pub asset_actions: Mutex<BTreeMap<crate::AssetReference, ImportManagedAssetAction>>,
    pub fail_asset_prepare: AtomicBool,
    pub mismatch_prepared_assets: AtomicBool,
    pub asset_steps: Mutex<Vec<&'static str>>,
    prepare_gate: Arc<PrepareGate>,
    source_gate: Arc<PrepareGate>,
    descriptor_gate: Arc<PrepareGate>,
}

impl TestPorts {
    fn new(
        source_path: PathBuf,
        target_path: PathBuf,
        source_id: ShowId,
        target_id: ShowId,
    ) -> Self {
        Self {
            source_path,
            target_path,
            source_id,
            target_id,
            steps: Arc::new(Mutex::new(Vec::new())),
            fail_prepare: AtomicBool::new(false),
            fail_commit: Arc::new(AtomicBool::new(false)),
            installed: Mutex::new(None),
            reconciled: Mutex::new(Vec::new()),
            asset_actions: Mutex::new(BTreeMap::new()),
            fail_asset_prepare: AtomicBool::new(false),
            mismatch_prepared_assets: AtomicBool::new(false),
            asset_steps: Mutex::new(Vec::new()),
            prepare_gate: Arc::new(PrepareGate::default()),
            source_gate: Arc::new(PrepareGate::default()),
            descriptor_gate: Arc::new(PrepareGate::default()),
        }
    }
}

pub(super) struct TestUnitOfWork {
    store: ShowStore,
    document: PortableShowDocument,
    steps: Arc<Mutex<Vec<&'static str>>>,
    fail_commit: Arc<AtomicBool>,
}

impl ActiveShowUnitOfWork for TestUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        self.steps.lock().push("backup");
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.steps.lock().push("commit");
        if self.fail_commit.load(Ordering::SeqCst) {
            return Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "injected commit failure",
            ));
        }
        self.store
            .apply_portable_transaction(transaction)
            .map_err(store_error)
    }
}

impl ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnitOfWork;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.steps.lock().push("begin");
        if show_id != self.target_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "target is not active",
            ));
        }
        let store = ShowStore::open(&self.target_path).map_err(store_error)?;
        let document = store.portable_document().map_err(store_error)?;
        Ok(TestUnitOfWork {
            store,
            document,
            steps: Arc::clone(&self.steps),
            fail_commit: Arc::clone(&self.fail_commit),
        })
    }

    fn prepare_object_undo(
        &self,
        _unit: &Self::UnitOfWork,
        _kind: &str,
        _object_id: &str,
        _expected_object_revision: light_core::Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        Err(ActionError::new(
            ActionErrorKind::Invalid,
            "undo is outside this test port",
        ))
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.steps.lock().push("prepare");
        if self.fail_prepare.load(Ordering::SeqCst) {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "injected runtime failure",
            ));
        }
        let mut gate = self.prepare_gate.state.lock().unwrap();
        if gate.armed {
            gate.entered = true;
            self.prepare_gate.changed.notify_all();
            while !gate.released {
                gate = self.prepare_gate.changed.wait(gate).unwrap();
            }
            gate.armed = false;
        }
        drop(gate);
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _context: &ActionContext, prepared: Self::PreparedRuntime) {
        self.steps.lock().push("install");
        *self.installed.lock() = Some(prepared);
    }

    fn reconcile_object_changes(&self, _changes: &[ActiveShowObjectChange]) {}
}

impl SelectiveShowImportPorts for TestPorts {
    type ImportSourceSnapshot = PortableShowDocument;
    type PreparedImportAssets = PreparedTestAssets;

    fn open_import_source_snapshot(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::ImportSourceSnapshot, ActionError> {
        self.steps.lock().push("source");
        if show_id != self.source_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "source is missing",
            ));
        }
        let snapshot = ShowStore::open(&self.source_path)
            .and_then(|store| store.portable_document())
            .map_err(store_error)?;
        pass_gate(&self.source_gate);
        Ok(snapshot)
    }

    fn import_source_document<'a>(
        &self,
        source: &'a Self::ImportSourceSnapshot,
    ) -> &'a PortableShowDocument {
        source
    }

    fn describe_import_object(
        &self,
        object: &light_show::PortableShowObject,
    ) -> Result<Option<ImportObjectDescriptor>, ActionError> {
        match object.key().kind() {
            "audio_cue" => {
                pass_gate(&self.descriptor_gate);
                describe_audio_cue(object).map(Some)
            }
            "custom_object" => {
                pass_gate(&self.descriptor_gate);
                describe_custom_object(object).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn reconcile_selective_import(&self, change: &SelectiveShowImportChange) {
        self.steps.lock().push("reconcile");
        self.reconciled.lock().push(change.clone());
    }

    fn inspect_import_asset(
        &self,
        _source: &Self::ImportSourceSnapshot,
        _target_show_id: ShowId,
        _asset: crate::AssetReference,
    ) -> Result<ImportManagedAssetAction, ActionError> {
        Ok(self
            .asset_actions
            .lock()
            .get(&_asset)
            .copied()
            .unwrap_or(ImportManagedAssetAction::SkipIdentical))
    }

    fn prepare_import_assets(
        &self,
        _context: &ActionContext,
        _source: &Self::ImportSourceSnapshot,
        _target_show_id: ShowId,
        assets: &[crate::AssetReference],
    ) -> Result<Self::PreparedImportAssets, ActionError> {
        self.steps.lock().push("asset_prepare");
        self.asset_steps.lock().push("asset_prepare");
        if self.fail_asset_prepare.load(Ordering::SeqCst) {
            self.steps.lock().push("asset_partial_cleanup");
            self.asset_steps.lock().push("asset_partial_cleanup");
            return Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "injected asset preparation failure",
            ));
        }
        Ok(PreparedTestAssets {
            assets: if self.mismatch_prepared_assets.load(Ordering::SeqCst) {
                Vec::new()
            } else {
                assets.to_vec()
            },
        })
    }

    fn prepared_import_assets<'a>(
        &self,
        prepared: &'a Self::PreparedImportAssets,
    ) -> &'a [crate::AssetReference] {
        &prepared.assets
    }

    fn compensate_import_assets(
        &self,
        _prepared: Self::PreparedImportAssets,
    ) -> Result<(), ActionError> {
        self.steps.lock().push("asset_compensate");
        self.asset_steps.lock().push("asset_compensate");
        Ok(())
    }

    fn publish_import_assets(&self, _prepared: Self::PreparedImportAssets) {
        self.steps.lock().push("asset_publish");
        self.asset_steps.lock().push("asset_publish");
    }
}

fn describe_audio_cue(
    object: &light_show::PortableShowObject,
) -> Result<ImportObjectDescriptor, ActionError> {
    let id = object
        .body()
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "audio cue id"))?;
    let asset_id = object
        .body()
        .get("asset_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "audio cue asset id"))?;
    let revision = object
        .body()
        .get("asset_revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "audio cue asset revision"))?;
    Ok(ImportObjectDescriptor {
        identities: vec![ImportOwnedIdentity {
            slot: "object".into(),
            value: id.into(),
            location: Some(ImportReferenceLocation::Value {
                pointer: "/id".into(),
                format: ImportIdentityFormat::Full,
            }),
        }],
        managed_assets: vec![crate::AssetReference {
            id: crate::AssetId(asset_id),
            revision: crate::AssetRevision(revision),
        }],
        ..ImportObjectDescriptor::default()
    })
}

fn describe_custom_object(
    object: &light_show::PortableShowObject,
) -> Result<ImportObjectDescriptor, ActionError> {
    let identity = object
        .body()
        .get("identity")
        .and_then(Value::as_str)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "custom object identity"))?;
    let mut descriptor = ImportObjectDescriptor {
        identities: vec![ImportOwnedIdentity {
            slot: "object".into(),
            value: identity.into(),
            location: Some(ImportReferenceLocation::Value {
                pointer: "/identity".into(),
                format: ImportIdentityFormat::Full,
            }),
        }],
        ..ImportObjectDescriptor::default()
    };
    if let Some(reference) = object.body().get("reference") {
        let field = |name| {
            reference.get(name).and_then(Value::as_str).ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::Invalid,
                    format!("custom object reference {name}"),
                )
            })
        };
        descriptor.references.push(ImportObjectReference {
            target: light_show::PortableShowObjectKey::new(field("kind")?, field("id")?),
            target_slot: field("slot")?.into(),
            source_identity: field("identity")?.into(),
            location: ImportReferenceLocation::Value {
                pointer: "/reference/identity".into(),
                format: ImportIdentityFormat::Full,
            },
        });
    }
    Ok(descriptor)
}

pub(super) struct PreparedTestAssets {
    assets: Vec<crate::AssetReference>,
}

#[derive(Default)]
struct PrepareGate {
    state: StdMutex<PrepareGateState>,
    changed: Condvar,
}

#[derive(Default)]
struct PrepareGateState {
    armed: bool,
    entered: bool,
    released: bool,
}

fn pass_gate(gate: &PrepareGate) {
    let mut state = gate.state.lock().unwrap();
    if !state.armed {
        return;
    }
    state.entered = true;
    gate.changed.notify_all();
    while !state.released {
        state = gate.changed.wait(state).unwrap();
    }
    state.armed = false;
}

fn wait_for_gate(gate: &PrepareGate, label: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut state = gate.state.lock().unwrap();
    while !state.entered {
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .unwrap_or_else(|| panic!("{label} did not reach the test gate"));
        let (next, timeout) = gate.changed.wait_timeout(state, remaining).unwrap();
        state = next;
        assert!(!timeout.timed_out(), "{label} test gate timed out");
    }
}

fn release_gate(gate: &PrepareGate) {
    let mut state = gate.state.lock().unwrap();
    state.released = true;
    gate.changed.notify_all();
}

pub(super) fn key(kind: &str, id: &str) -> light_show::PortableShowObjectKey {
    light_show::PortableShowObjectKey::new(kind, id)
}

pub(super) fn profile(id: FixtureId, revision: u64, extra: Value) -> FixtureProfileRevision {
    let mut body = json!({
        "id": id.0,
        "revision": revision,
        "name": "Portable",
        "modes": []
    });
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    FixtureProfileRevision::from_profile(body).unwrap()
}

pub(super) struct PortableFixtureTestRecord {
    pub profile: FixtureProfileRevision,
    pub body: Value,
    pub fixture_id: FixtureId,
    pub head_id: FixtureId,
    pub multipatch_id: Uuid,
}

pub(super) fn portable_fixture_record(
    identity_base: u128,
    fixture_number: u32,
) -> PortableFixtureTestRecord {
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(Uuid::from_u128(identity_base));
    profile.revision = 7;
    profile.manufacturer = "Acme".into();
    profile.name = "Selective import".into();
    profile.short_name = "Import".into();
    profile.modes[0].id = Uuid::from_u128(identity_base + 1);
    let head_profile_id = Uuid::from_u128(identity_base + 2);
    profile.modes[0].heads.push(FixtureHead {
        id: head_profile_id,
        name: "Cell".into(),
        master_shared: false,
    });
    let fixture_id = FixtureId(Uuid::from_u128(identity_base + 10));
    let head_id = FixtureId(Uuid::from_u128(identity_base + 11));
    let multipatch_id = Uuid::from_u128(identity_base + 12);
    let fixture = PatchedFixture {
        fixture_id,
        fixture_number: Some(fixture_number),
        virtual_fixture_number: None,
        name: format!("Fixture {fixture_number}"),
        definition: profile.resolved_definition(profile.modes[0].id).unwrap(),
        universe: None,
        address: None,
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![PatchedHead {
            profile_head_id: Some(head_profile_id),
            head_index: 1,
            fixture_id: head_id,
        }],
        multipatch: vec![MultiPatchInstance {
            id: multipatch_id,
            name: "Balcony".into(),
            universe: None,
            address: None,
            split_patches: Vec::new(),
            location: Default::default(),
            rotation: Default::default(),
        }],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    let mut profile_body = serde_json::to_value(&profile).unwrap();
    profile_body["future_profile"] = json!({"retained": true});
    let stored_profile = FixtureProfileRevision::from_profile(profile_body).unwrap();
    let mut body = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
        .unwrap()
        .into_body();
    body["future_fixture"] = json!({"profile_id": "must-not-change", "retained": true});
    PortableFixtureTestRecord {
        profile: stored_profile,
        body,
        fixture_id,
        head_id,
        multipatch_id,
    }
}

pub(super) fn legacy_fixture_record(
    identity_base: u128,
    fixture_number: u32,
) -> PortableFixtureTestRecord {
    let portable = portable_fixture_record(identity_base, fixture_number);
    let profile: FixtureProfile =
        serde_json::from_value(portable.profile.profile().clone()).unwrap();
    let fixture = PatchedFixture {
        fixture_id: portable.fixture_id,
        fixture_number: Some(fixture_number),
        virtual_fixture_number: None,
        name: "Legacy".into(),
        definition: profile.resolved_definition(profile.modes[0].id).unwrap(),
        universe: None,
        address: None,
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![PatchedHead {
            profile_head_id: None,
            head_index: 1,
            fixture_id: portable.head_id,
        }],
        multipatch: Vec::new(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    let mut body = serde_json::to_value(fixture).unwrap();
    body["future_fixture"] = json!({"retained": true});
    PortableFixtureTestRecord { body, ..portable }
}

fn put_object(path: &PathBuf, kind: &str, id: &str, body: Value) {
    ShowStore::open(path)
        .unwrap()
        .put_object(kind, id, &body, 0)
        .unwrap();
}

fn insert_profile(path: &PathBuf, profile: &FixtureProfileRevision) {
    ShowStore::open(path)
        .unwrap()
        .insert_fixture_profile_revision(profile)
        .unwrap();
}

fn document(path: &PathBuf) -> PortableShowDocument {
    ShowStore::open(path).unwrap().portable_document().unwrap()
}

pub(super) fn context() -> ActionContext {
    ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        ActionSource::UserInterface,
    )
    .with_request_id("selective-import-test")
}

fn store_error(error: StoreError) -> ActionError {
    ActionError::new(ActionErrorKind::Internal, error.to_string())
}

fn temporary_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-import-{label}-{}.sqlite", Uuid::new_v4()))
}

fn remove_sqlite_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
