//! TL-560 contract-1 startup harness (test-only API; see the handoff's API note).
//!
//! Runs the real `StartupState::load` (desk open, default-show provisioning, Programmer restore,
//! active-show compile and migration commit, Playback and Output runtime restore) at a chosen
//! programming contract. The contract comes from the `#[cfg(test)]` thread-local override in
//! `e2e_semantic_contract::startup_contract_override`; production has no way to reach it.
//!
//! ```ignore
//! let desk = ContractDesk::new("my-test");
//! let show = desk.create_show("Semantic");          // registered in the desk library
//! show.patch(&fixture);                             // profile revision + patch record
//! show.put("preset", "2.1", body);                  // any raw portable object
//! desk.activate(&show);
//! let startup = desk.load_at(SEMANTIC_CONTRACT);    // real StartupState::load at contract 1
//! assert!(startup.active_show_error.is_none());
//! let served = desk.serve(startup).await;            // served.state: AppState for HTTP checks
//! served.close().await;
//! ```
use super::*;
pub(in crate::runtime) use crate::runtime::bootstrap::ServedStartupForTests;
use crate::runtime::e2e_semantic_contract::startup_contract_override;
use light_fixture::{PatchedFixture, PortablePatchedFixtureRecord};
use light_show::{FixtureProfileRevision, ShowEntry};

/// The semantic programming contract (`PROGRAMMING_CONTRACT_VERSION`).
pub(in crate::runtime) const SEMANTIC_CONTRACT: u16 =
    light_core::programming::PROGRAMMING_CONTRACT_VERSION;

/// Runs the real startup path at `contract` on the calling thread.
pub(in crate::runtime) fn load_startup_at(
    contract: u16,
    options: startup_options::StartupOptions,
) -> anyhow::Result<startup_state::StartupState> {
    let _contract = startup_contract_override::at(contract);
    startup_state::StartupState::load(options)
}

/// One desk data directory under `LIGHT_TMP_DIR`, removed on drop.
pub(in crate::runtime) struct ContractDesk {
    pub(in crate::runtime) data_dir: PathBuf,
}

impl Drop for ContractDesk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

impl ContractDesk {
    pub(in crate::runtime) fn new(label: &str) -> Self {
        let root = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let data_dir = root.join(format!("tl560-{label}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(data_dir.join("shows")).unwrap();
        Self { data_dir }
    }

    pub(in crate::runtime) fn options(&self) -> startup_options::StartupOptions {
        startup_options::StartupOptions {
            data_dir: self.data_dir.clone(),
            show_file: None,
            fixture_package_dir: None,
            extensions_dir: Some(self.data_dir.join("extensions")),
            bind: "127.0.0.1:0".parse().unwrap(),
            test_bench: true,
            visualizer_preview: false,
            osc_bind_override: None,
            output_bind_override: None,
        }
    }

    /// The real `StartupState::load` at `contract`. Startup never fails for a rejected show;
    /// it reports `active_show_error` instead.
    pub(in crate::runtime) fn load_at(&self, contract: u16) -> startup_state::StartupState {
        load_startup_at(contract, self.options()).unwrap()
    }

    pub(in crate::runtime) fn desk(&self) -> DeskStore {
        DeskStore::open(self.data_dir.join("desk.sqlite")).unwrap()
    }

    /// Creates `shows/<name>.show`, registers it in the desk library and gives it that identity.
    pub(in crate::runtime) fn create_show(&self, name: &str) -> ContractShow {
        let path = self.data_dir.join("shows").join(format!("{name}.show"));
        let (store, _) = ShowStore::create(&path, name).unwrap();
        let entry = self
            .desk()
            .upsert_show(name, &path.display().to_string(), false)
            .unwrap();
        store.set_identity(entry.id, name, None).unwrap();
        ContractShow { path, entry }
    }

    pub(in crate::runtime) fn activate(&self, show: &ContractShow) {
        self.desk().set_active_show(Some(show.entry.id)).unwrap();
    }

    /// Starts the runtime resources and builds the served `AppState` (release with `close`).
    pub(in crate::runtime) async fn serve(
        &self,
        startup: startup_state::StartupState,
    ) -> ServedStartupForTests {
        ServedStartupForTests::start(startup).await
    }
}

/// One show file in a `ContractDesk`. Every write opens and closes its own connection, so the
/// main file is checkpointed and `bytes` is a stable comparison point.
pub(in crate::runtime) struct ContractShow {
    pub(in crate::runtime) path: PathBuf,
    pub(in crate::runtime) entry: ShowEntry,
}

impl ContractShow {
    pub(in crate::runtime) fn store(&self) -> ShowStore {
        ShowStore::open(&self.path).unwrap()
    }

    pub(in crate::runtime) fn put(&self, kind: &str, id: &str, body: serde_json::Value) {
        let store = self.store();
        let expected = store
            .portable_document()
            .unwrap()
            .object(kind, id)
            .map_or(0, |object| object.revision());
        store.put_object(kind, id, &body, expected).unwrap();
    }

    /// Patches a fixture and stores its immutable profile revision.
    pub(in crate::runtime) fn patch(&self, fixture: &PatchedFixture) {
        let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
        self.store()
            .insert_fixture_profile_revision(
                &FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let body = PortablePatchedFixtureRecord::from_runtime_fixture(fixture)
            .unwrap()
            .into_body();
        self.put("patched_fixture", &fixture.fixture_id.0.to_string(), body);
    }

    /// Exact bytes of the main file (and an empty or absent WAL), for "original preserved".
    pub(in crate::runtime) fn bytes(&self) -> Vec<u8> {
        let wal = PathBuf::from(format!("{}-wal", self.path.display()));
        assert!(
            std::fs::metadata(&wal).map_or(true, |metadata| metadata.len() == 0),
            "no pending WAL frames outside the main file"
        );
        std::fs::read(&self.path).unwrap()
    }
}
