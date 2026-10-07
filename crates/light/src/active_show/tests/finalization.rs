use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Failure {
    None,
    Finalization,
    Backup,
    Commit,
}

#[derive(Clone, Copy, Debug)]
enum PersistenceSite {
    RouteRange,
    Route,
    Objects,
    Transaction,
}

const SITES: [PersistenceSite; 4] = [
    PersistenceSite::RouteRange,
    PersistenceSite::Route,
    PersistenceSite::Objects,
    PersistenceSite::Transaction,
];

fn rig() -> TestRig {
    let directory = PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR")
            .expect("initialize canonical artifact paths before running finalization tests"),
    );
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "active-show-finalization-{}.sqlite",
        Uuid::new_v4()
    ));
    let (store, show_id) = ShowStore::create(&path, "Runtime finalization test").unwrap();
    drop(store);
    TestRig {
        service: ActiveShowService::new(EventBus::new(16)),
        ports: TestPorts {
            path,
            show_id,
            steps: Arc::default(),
            installed: Arc::default(),
        },
        show_id,
    }
}

fn context() -> ActionContext {
    ActionContext::operator(Uuid::from_u128(1), Uuid::from_u128(3), ActionSource::Http)
}

fn injected_failure(failure: Failure) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, format!("injected {failure:?}"))
}

struct FinalizationPorts<'a> {
    inner: &'a TestPorts,
    events: &'a EventBus,
    failure: Failure,
}

struct FinalizationUnit {
    inner: TestUnitOfWork,
    failure: Failure,
}

impl ActiveShowUnitOfWork for FinalizationUnit {
    fn document(&self) -> &PortableShowDocument {
        self.inner.document()
    }

    fn backup(&mut self, identity: &BackupIdentity) -> Result<(), ActionError> {
        if self.failure == Failure::Backup {
            self.inner.steps.lock().push("backup");
            return Err(injected_failure(self.failure));
        }
        self.inner.backup(identity)
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        if self.failure == Failure::Commit {
            self.inner.steps.lock().push("commit");
            return Err(injected_failure(self.failure));
        }
        self.inner.commit(transaction)
    }
}

impl ActiveShowPorts for FinalizationPorts<'_> {
    type UnitOfWork = FinalizationUnit;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        Ok(FinalizationUnit {
            inner: self.inner.begin_active_show(context, show_id)?,
            failure: self.failure,
        })
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.inner
            .prepare_object_undo(&unit.inner, kind, object_id, expected_object_revision)
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.inner.prepare_runtime(snapshot)
    }

    fn finalize_runtime<T>(
        &self,
        context: &ActionContext,
        prepared: Self::PreparedRuntime,
        persist: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError> {
        assert_eq!(*self.inner.steps.lock(), ["begin", "prepare"]);
        assert_eq!(self.events.latest_sequence(), 0);
        assert!(self.inner.installed.lock().is_none());
        self.inner.steps.lock().push("finalize");
        if self.failure == Failure::Finalization {
            return Err(injected_failure(self.failure));
        }

        // Exercise the default implementation as well as the service's override dispatch.
        let result = self.inner.finalize_runtime(context, prepared, persist);
        assert_eq!(self.events.latest_sequence(), 0);
        self.inner.steps.lock().push("release");
        result
    }

    fn install_runtime(&self, context: &ActionContext, prepared: Self::PreparedRuntime) {
        self.inner.install_runtime(context, prepared);
    }

    fn reconcile_object_changes(&self, changes: &[ActiveShowObjectChange]) {
        assert_eq!(self.inner.steps.lock().last(), Some(&"release"));
        self.inner.reconcile_object_changes(changes);
    }
}

fn run_site(
    rig: &TestRig,
    ports: &FinalizationPorts<'_>,
    site: PersistenceSite,
) -> Result<(), ActionError> {
    match site {
        PersistenceSite::RouteRange => rig
            .service
            .create_output_route_range(rig.range_action(2, 102), ports)
            .map(|_| ()),
        PersistenceSite::Route => rig
            .service
            .mutate_output_route(
                rig.action(
                    "main",
                    0,
                    OutputRouteMutation::Put {
                        body: typed_route(json!({
                            "protocol": "art_net",
                            "logical_universe": 1,
                            "destination_universe": 1,
                            "delivery_mode": "broadcast",
                            "destination": null,
                            "enabled": true,
                            "minimum_slots": 512
                        })),
                    },
                ),
                ports,
            )
            .map(|_| ()),
        PersistenceSite::Objects => rig
            .service
            .mutate_objects(
                rig.object_action(vec![ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::Group,
                    object_id: "1".into(),
                    expected_object_revision: 0,
                    mutation: ActiveShowObjectMutationKind::Put {
                        body: Box::new(typed(
                            ActiveShowObjectKind::Group,
                            json!({"id": "1", "name": "Group", "fixtures": []}),
                        )),
                    },
                }]),
                ports,
            )
            .map(|_| ()),
        PersistenceSite::Transaction => rig.service.transact(
            &context(),
            rig.show_id,
            ports,
            "finalization-test",
            |document| {
                let mut transaction = document.transaction();
                transaction.put(
                    "group",
                    "1",
                    json!({"id": "1", "name": "Group", "fixtures": []}),
                );
                Ok(PreparedActiveShowTransaction::PreparedCommit {
                    prepared: Box::new(crate::prepare_show_candidate(document, transaction)?),
                    state: (),
                })
            },
            |events, ports, _, completed| {
                assert!(completed.commit.is_some());
                assert_eq!(events.latest_sequence(), 0);
                assert_eq!(ports.inner.steps.lock().last(), Some(&"release"));
                ports.inner.steps.lock().push("complete");
            },
        ),
    }
}

#[test]
fn rejection_precedes_backup_commit_install_and_events_at_all_persistence_sites() {
    for site in SITES {
        let rig = rig();
        let before = rig.document();
        let ports = FinalizationPorts {
            inner: &rig.ports,
            events: rig.service.events(),
            failure: Failure::Finalization,
        };
        let error = run_site(&rig, &ports, site).unwrap_err();
        assert_eq!(error.message, "injected Finalization", "{site:?}");
        assert_eq!(rig.steps(), ["begin", "prepare", "finalize"], "{site:?}");
        assert_eq!(rig.document(), before, "{site:?}");
        assert!(rig.installed_snapshot().is_none(), "{site:?}");
        assert_eq!(rig.service.events().latest_sequence(), 0, "{site:?}");
    }
}

#[test]
fn persistence_failures_release_finalizer_without_install_or_events() {
    for site in SITES {
        for failure in [Failure::Backup, Failure::Commit] {
            let rig = rig();
            let before = rig.document();
            let ports = FinalizationPorts {
                inner: &rig.ports,
                events: rig.service.events(),
                failure,
            };
            let error = run_site(&rig, &ports, site).unwrap_err();
            assert_eq!(error.message, format!("injected {failure:?}"), "{site:?}");
            let mut expected = vec!["begin", "prepare", "finalize", "backup"];
            if failure == Failure::Commit {
                expected.push("commit");
            }
            expected.push("release");
            assert_eq!(rig.steps(), expected, "{site:?}/{failure:?}");
            assert_eq!(rig.document(), before, "{site:?}/{failure:?}");
            assert!(rig.installed_snapshot().is_none(), "{site:?}/{failure:?}");
            assert_eq!(
                rig.service.events().latest_sequence(),
                0,
                "{site:?}/{failure:?}"
            );
        }
    }
}

#[test]
fn success_installs_once_before_finalizer_returns_and_application_completion_runs() {
    for site in SITES {
        let rig = rig();
        let ports = FinalizationPorts {
            inner: &rig.ports,
            events: rig.service.events(),
            failure: Failure::None,
        };
        run_site(&rig, &ports, site).unwrap();
        let mut expected = vec![
            "begin", "prepare", "finalize", "backup", "commit", "install", "release",
        ];
        match site {
            PersistenceSite::Objects => expected.push("reconcile"),
            PersistenceSite::Transaction => expected.push("complete"),
            _ => (),
        }
        assert_eq!(rig.steps(), expected, "{site:?}");
        assert_eq!(
            rig.installed_revision(),
            Some(rig.document().revision().value())
        );
        assert!(rig.installed_revision().unwrap() > 0, "{site:?}");
        if !matches!(site, PersistenceSite::Transaction) {
            assert!(rig.service.events().latest_sequence() > 0, "{site:?}");
        }
    }
}

#[test]
fn no_change_completes_without_preparation_finalization_or_persistence() {
    let rig = rig();
    let before = rig.document();
    let ports = FinalizationPorts {
        inner: &rig.ports,
        events: rig.service.events(),
        failure: Failure::Finalization,
    };
    let result = rig
        .service
        .transact(
            &context(),
            rig.show_id,
            &ports,
            "unchanged-test",
            |_| Ok(PreparedActiveShowTransaction::NoChange("unchanged")),
            |events, ports, _, completed| {
                assert!(completed.commit.is_none());
                assert_eq!(events.latest_sequence(), 0);
                ports.inner.steps.lock().push("complete");
                completed.state
            },
        )
        .unwrap();
    assert_eq!(result, "unchanged");
    assert_eq!(rig.steps(), ["begin", "complete"]);
    assert_eq!(rig.document(), before);
    assert!(rig.installed_snapshot().is_none());
    assert_eq!(rig.service.events().latest_sequence(), 0);
}
