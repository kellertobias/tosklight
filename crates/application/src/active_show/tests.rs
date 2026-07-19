use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ApplicationEvent,
    EventBus, EventFilter, EventReplay, ShowEvent,
};
use light_core::ShowId;
use light_engine::EngineSnapshot;
use light_show::{PortableShowCommit, PortableShowDocument, PortableShowTransaction, ShowStore};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::Arc};
use uuid::Uuid;

#[test]
fn route_update_prepares_before_one_backup_and_preserves_raw_extensions() {
    let rig = TestRig::new();
    rig.seed_route(
        "main",
        json!({
            "protocol": "art_net",
            "logical_universe": 1,
            "destination_universe": 1,
            "delivery_mode": "broadcast",
            "destination": null,
            "enabled": true,
            "minimum_slots": 512,
            "future_server_field": {"kept": true}
        }),
    );
    let result = rig
        .service
        .mutate_output_route(
            rig.action(
                "main",
                1,
                OutputRouteMutation::Put {
                    body: json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 2,
                        "delivery_mode": "broadcast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 128,
                        "future_client_field": "accepted"
                    }),
                },
            ),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(
        rig.steps(),
        ["begin", "prepare", "backup", "commit", "install"]
    );
    assert_eq!(result.change.show_revision.value(), 2);
    assert_eq!(result.change.object_revision, 2);
    assert_eq!(result.event_sequence, 1);
    assert_eq!(
        result
            .route_to_terminate
            .as_ref()
            .map(|route| route.destination_universe),
        Some(1)
    );
    let stored = rig.route_body("main");
    assert_eq!(stored["future_server_field"], json!({"kept": true}));
    assert_eq!(stored["future_client_field"], "accepted");
    assert_eq!(stored["destination_universe"], 2);
    assert_eq!(rig.installed_routes(), 1);

    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained route event");
    };
    let event = events.first().unwrap();
    assert_eq!(
        event.object.as_ref().unwrap().id,
        format!("route:{}:main", rig.show_id.0)
    );
    assert!(matches!(
        &event.payload,
        ApplicationEvent::Show(ShowEvent::OutputRouteChanged(change))
            if change.route_id == "main" && !change.deleted
    ));
}

#[test]
fn invalid_route_stops_before_backup_commit_install_and_event() {
    let rig = TestRig::new();
    let error = rig
        .service
        .mutate_output_route(
            rig.action(
                "broken",
                0,
                OutputRouteMutation::Put {
                    body: json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 1,
                        "delivery_mode": "unicast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 512
                    }),
                },
            ),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.steps(), ["begin"]);
    assert!(rig.document().object("route", "broken").is_none());
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn stale_object_revision_stops_before_candidate_preparation_or_side_effects() {
    let rig = TestRig::new();
    rig.seed_route(
        "main",
        json!({
            "protocol": "art_net",
            "logical_universe": 1,
            "destination_universe": 1,
            "delivery_mode": "broadcast",
            "destination": null,
            "enabled": true,
            "minimum_slots": 512
        }),
    );

    let error = rig
        .service
        .mutate_output_route(
            rig.action("main", 0, OutputRouteMutation::Delete),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
    assert_eq!(rig.steps(), ["begin"]);
    assert!(rig.document().object("route", "main").is_some());
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn route_delete_uses_the_same_prepared_atomic_boundary() {
    let rig = TestRig::new();
    rig.seed_route(
        "main",
        json!({
            "protocol": "sacn",
            "logical_universe": 1,
            "destination_universe": 1,
            "delivery_mode": "multicast",
            "destination": null,
            "enabled": true,
            "minimum_slots": 512,
            "future": true
        }),
    );

    let result = rig
        .service
        .mutate_output_route(
            rig.action("main", 1, OutputRouteMutation::Delete),
            &rig.ports,
        )
        .unwrap();

    assert!(result.change.deleted);
    assert!(result.change.route.is_none());
    assert!(result.route_to_terminate.is_some());
    assert_eq!(result.change.object_revision, 2);
    assert!(rig.document().object("route", "main").is_none());
    assert_eq!(rig.installed_routes(), 0);
    assert_eq!(
        rig.steps(),
        ["begin", "prepare", "backup", "commit", "install"]
    );
}

struct TestRig {
    service: ActiveShowService,
    ports: TestPorts,
    show_id: ShowId,
}

impl TestRig {
    fn new() -> Self {
        let path = temporary_show_path();
        let (store, show_id) = ShowStore::create(&path, "Active show route test").unwrap();
        drop(store);
        let events = EventBus::new(16);
        Self {
            service: ActiveShowService::new(events),
            ports: TestPorts {
                path,
                show_id,
                steps: Arc::default(),
                installed: Arc::default(),
            },
            show_id,
        }
    }

    fn seed_route(&self, id: &str, body: Value) {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object("route", id, &body, 0)
            .unwrap();
    }

    fn action(
        &self,
        route_id: &str,
        expected_object_revision: u64,
        mutation: OutputRouteMutation,
    ) -> ActionEnvelope<MutateOutputRouteCommand> {
        ActionEnvelope {
            context: ActionContext::operator(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                Uuid::from_u128(3),
                ActionSource::Http,
            ),
            command: MutateOutputRouteCommand {
                show_id: self.show_id,
                route_id: route_id.into(),
                expected_object_revision,
                mutation,
            },
        }
    }

    fn route_body(&self, id: &str) -> Value {
        self.document().object("route", id).unwrap().body().clone()
    }

    fn document(&self) -> PortableShowDocument {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }

    fn installed_routes(&self) -> usize {
        self.ports
            .installed
            .lock()
            .as_ref()
            .map_or(0, |snapshot| snapshot.routes.len())
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        remove_sqlite_files(&self.ports.path);
    }
}

struct TestPorts {
    path: PathBuf,
    show_id: ShowId,
    steps: Arc<Mutex<Vec<&'static str>>>,
    installed: Arc<Mutex<Option<EngineSnapshot>>>,
}

struct TestUnitOfWork {
    store: ShowStore,
    document: PortableShowDocument,
    steps: Arc<Mutex<Vec<&'static str>>>,
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
        self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.steps.lock().push("commit");
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))
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
        if show_id != self.show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not active",
            ));
        }
        let store = ShowStore::open(&self.path)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        let document = store
            .portable_document()
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        Ok(TestUnitOfWork {
            store,
            document,
            steps: Arc::clone(&self.steps),
        })
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.steps.lock().push("prepare");
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, prepared: Self::PreparedRuntime) {
        self.steps.lock().push("install");
        *self.installed.lock() = Some(prepared);
    }
}

fn temporary_show_path() -> PathBuf {
    std::env::temp_dir().join(format!("light-active-show-route-{}.sqlite", Uuid::new_v4()))
}

fn remove_sqlite_files(path: &PathBuf) {
    let _ = fs::remove_file(path);
    for suffix in ["-shm", "-wal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}
