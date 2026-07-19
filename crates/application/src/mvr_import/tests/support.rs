use super::super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ActiveShowPorts,
    ActiveShowService, ActiveShowUnitOfWork, BackupIdentity, EventBus, PatchChange, ShowPatchPorts,
};
use light_core::{AttributeKey, FixtureId, Revision, ShowId};
use light_engine::EngineSnapshot;
use light_fixture::{
    ByteOrder, ChannelComponent, FixtureDefinition, LogicalHead, Parameter, PatchedFixture,
};
use light_mvr::{MvrDocument, MvrFixture};
use light_show::{
    FixtureProfileRevision, PortableShowCommit, PortableShowDocument, PortableShowObjectUndo,
    PortableShowTransaction, ShowStore,
};
use parking_lot::Mutex;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use uuid::Uuid;

#[derive(Default)]
pub struct Counters {
    pub begins: AtomicUsize,
    pub backups: AtomicUsize,
    pub commits: AtomicUsize,
    pub runtime_prepares: AtomicUsize,
    pub runtime_installs: AtomicUsize,
    pub reconciles: AtomicUsize,
}

#[derive(Clone)]
pub struct TestPorts {
    path: PathBuf,
    pub counters: Arc<Counters>,
    pub installed: Arc<Mutex<Option<EngineSnapshot>>>,
    pub last_change: Arc<Mutex<Option<PatchChange>>>,
    pub fail_backup: Arc<AtomicBool>,
    pub fail_runtime: Arc<AtomicBool>,
}

impl TestPorts {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            counters: Arc::default(),
            installed: Arc::default(),
            last_change: Arc::default(),
            fail_backup: Arc::default(),
            fail_runtime: Arc::default(),
        }
    }

    pub fn store(&self) -> ShowStore {
        ShowStore::open(&self.path).unwrap()
    }
}

pub struct TestUnit {
    store: ShowStore,
    document: PortableShowDocument,
    counters: Arc<Counters>,
    fail_backup: Arc<AtomicBool>,
}

impl ActiveShowUnitOfWork for TestUnit {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        self.counters.backups.fetch_add(1, Ordering::Relaxed);
        if self.fail_backup.load(Ordering::Relaxed) {
            Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "backup failed",
            ))
        } else {
            Ok(())
        }
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.counters.commits.fetch_add(1, Ordering::Relaxed);
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| ActionError::new(ActionErrorKind::Unavailable, error.to_string()))
    }
}

impl ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnit;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.counters.begins.fetch_add(1, Ordering::Relaxed);
        let store = self.store();
        let document = store
            .portable_document()
            .map_err(|error| ActionError::new(ActionErrorKind::Unavailable, error.to_string()))?;
        if document.id() != show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not active",
            ));
        }
        Ok(TestUnit {
            store,
            document,
            counters: Arc::clone(&self.counters),
            fail_backup: Arc::clone(&self.fail_backup),
        })
    }

    fn prepare_object_undo(
        &self,
        _unit: &Self::UnitOfWork,
        _kind: &str,
        _object_id: &str,
        _expected_object_revision: Revision,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        Err(ActionError::new(
            ActionErrorKind::Invalid,
            "undo is not part of the MVR test adapter",
        ))
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.counters
            .runtime_prepares
            .fetch_add(1, Ordering::Relaxed);
        if self.fail_runtime.load(Ordering::Relaxed) {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "runtime preparation rejected the candidate",
            ));
        }
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, prepared: Self::PreparedRuntime) {
        self.counters
            .runtime_installs
            .fetch_add(1, Ordering::Relaxed);
        *self.installed.lock() = Some(prepared);
    }
}

impl ShowPatchPorts for TestPorts {
    fn resolve_profile_revision(
        &self,
        _profile_id: FixtureId,
        _revision: Revision,
    ) -> Result<FixtureProfileRevision, ActionError> {
        Err(ActionError::new(
            ActionErrorKind::NotFound,
            "profile lookup is not used by MVR imports",
        ))
    }

    fn reconcile_patch_change(&self, change: &PatchChange) {
        self.counters.reconciles.fetch_add(1, Ordering::Relaxed);
        *self.last_change.lock() = Some(change.clone());
    }
}

pub struct Rig {
    pub path: PathBuf,
    pub show_id: ShowId,
    pub events: EventBus,
    pub service: MvrImportService,
    pub ports: TestPorts,
}

impl Rig {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("light-mvr-import-{}.show", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "MVR application test").unwrap();
        drop(store);
        let events = EventBus::new(32);
        let active_show = ActiveShowService::new(events.clone());
        Self {
            ports: TestPorts::new(path.clone()),
            service: MvrImportService::new(active_show.clone()),
            path,
            show_id,
            events,
        }
    }

    pub fn envelope(
        &self,
        fixtures: Vec<MvrFixture>,
        definitions: Vec<FixtureDefinition>,
    ) -> ActionEnvelope<ApplyActiveMvrImportCommand> {
        ActionEnvelope {
            context: ActionContext::system(Uuid::from_u128(7), ActionSource::Http)
                .with_request_id("mvr-apply"),
            command: ApplyActiveMvrImportCommand {
                show_id: self.show_id,
                document: MvrDocument {
                    fixtures,
                    ..MvrDocument::default()
                },
                definitions,
                resolutions: Default::default(),
            },
        }
    }

    pub fn document(&self) -> PortableShowDocument {
        self.ports.store().portable_document().unwrap()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn fixture_definition(footprint: u16) -> FixtureDefinition {
    FixtureDefinition {
        schema_version: 1,
        id: FixtureId(Uuid::from_u128(800)),
        revision: 1,
        manufacturer: "MVR Maker".into(),
        device_type: "other".into(),
        name: "MVR Model".into(),
        model: "MVR Model".into(),
        mode: "Standard".into(),
        footprint,
        heads: vec![LogicalHead {
            index: 0,
            name: "Main".into(),
            shared: true,
            parameters: vec![Parameter {
                attribute: AttributeKey("intensity".into()),
                components: (0..footprint)
                    .map(|offset| ChannelComponent {
                        offset,
                        byte_order: ByteOrder::MsbFirst,
                    })
                    .collect(),
                default: 0.0,
                virtual_dimmer: false,
                metadata: Default::default(),
                capabilities: Vec::new(),
            }],
        }],
        color_calibration: None,
        physical: Default::default(),
        model_asset: None,
        icon_asset: None,
        hazardous: false,
        direct_control_protocols: Vec::new(),
        signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
        safe_values: Default::default(),
        profile_id: None,
        mode_id: None,
        profile_snapshot: None,
    }
}

pub fn mvr_fixture(uuid: Uuid, name: &str, universe: u16, address: u16) -> MvrFixture {
    MvrFixture {
        uuid,
        name: name.into(),
        fixture_id: None,
        gdtf_spec: "MVR Model.gdtf".into(),
        gdtf_mode: "Standard".into(),
        universe: Some(universe),
        address: Some(address),
        matrix: [
            1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 20.0, 30.0,
        ],
        layer: Some("mvr".into()),
        class: None,
    }
}

pub fn stored_fixture(
    fixture_id: FixtureId,
    definition: FixtureDefinition,
    address: u16,
    mib: (bool, u64),
) -> PatchedFixture {
    PatchedFixture {
        fixture_id,
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Stored".into(),
        definition,
        universe: Some(1),
        address: Some(address),
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: Vec::new(),
        move_in_black_enabled: mib.0,
        move_in_black_delay_millis: mib.1,
        highlight_overrides: Default::default(),
        multipatch: Vec::new(),
    }
}

pub fn count(counter: &AtomicUsize) -> usize {
    counter.load(Ordering::Relaxed)
}
