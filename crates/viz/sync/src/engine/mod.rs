//! The synchronized document: one bound Architect document kept in step with its desk show.
//!
//! Every persistent Architect writer runs inside a *gesture* of the host (the editor's session).
//! When the outermost gesture ends the host calls [`SyncEngine::capture`], which reads the
//! document, compares it with the state it had before the gesture, and journals the difference as
//! one [`ShowEditIntent`] — one gesture, one transaction, however many writes it made. A
//! background task sends journal entries in order with their stable request identities, applies
//! the desk's commits from the sync feed to the confirmed mirror, and rebuilds the affected
//! objects of the working document as mirror plus the edits still awaiting the desk.
//!
//! Remote changes reach the working document only through [`DocumentHost::apply_remote`], which
//! the host runs under the same lock as a gesture, so a remote change never lands inside the
//! capture window of a local one and is never journaled as the Architect's own edit.

mod capture;
mod conflicts;
mod rebase;
mod remote;
mod run;

pub use conflicts::{ConflictView, Resolution};

use crate::binding::{SyncBinding, SyncBindingStore};
use crate::desk::DeskClient;
use crate::journal::Journal;
use crate::mirror::Mirror;
use crate::state::{DocumentReader, ObjectKey, VersionedState};
use crate::status::{Connection, StatusFacts, SyncStatus};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use viz_document::PlanningDocument;

type Answer<T> = Result<T, String>;

/// What the engine needs from the application holding the document.
pub trait DocumentHost: Send + Sync + 'static {
    /// Runs `edit` against the bound document under the lock gestures hold. `edit` answers whether
    /// it changed the document; when it did, the host tells its views. Errs when the bound
    /// document is no longer the open one.
    fn apply_remote(&self, edit: &mut dyn FnMut(&PlanningDocument) -> Answer<bool>) -> Answer<()>;

    /// The status changed.
    fn status_changed(&self, status: &SyncStatus);
}

/// The working document's synchronized content as the engine last saw it.
pub(crate) struct Shadow {
    pub versioned: VersionedState,
    pub reader: DocumentReader,
}

pub(crate) struct Shared {
    pub binding: Mutex<SyncBinding>,
    pub store: SyncBindingStore,
    pub directory: PathBuf,
    pub journal: Mutex<Journal>,
    pub mirror: Mutex<Mirror>,
    pub shadow: Mutex<Shadow>,
    pub facts: Mutex<StatusFacts>,
    pub last_status: Mutex<Option<SyncStatus>>,
    pub host: Arc<dyn DocumentHost>,
    pub client: DeskClient,
    pub wake: tokio::sync::Notify,
    pub stopped: AtomicBool,
    pub paused: AtomicBool,
    pub lose_next_reply: AtomicBool,
    /// Whole-show snapshot reads since the engine started: catch-ups after a gap or a reconnect.
    pub snapshot_reads: std::sync::atomic::AtomicU64,
    /// The mirror was damaged: the next snapshot of the desk turns every difference between the
    /// document and the desk into a recoverable conflict instead of trusting either side.
    pub rebuild_mirror: AtomicBool,
    /// Desk revisions the desk reported in outcomes before their commit reached the mirror.
    pub known_revisions: Mutex<BTreeMap<ObjectKey, u64>>,
}

/// One bound document's synchronization. Cloning shares it.
#[derive(Clone)]
pub struct SyncEngine {
    shared: Arc<Shared>,
    task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// How a binding starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Start {
    /// The document was just copied from the desk: it *is* the desk's show at the binding's
    /// acknowledged revision, so it seeds the confirmed mirror.
    FreshCopy,
    /// The document was bound before; the journal and mirror on disk carry its state.
    Reopen,
}

impl SyncEngine {
    /// Starts synchronizing `document` with the show `binding` names. Call with the host's
    /// gesture lock held, so nothing changes the document while its starting state is read.
    pub fn start(
        document: &PlanningDocument,
        binding: SyncBinding,
        store: SyncBindingStore,
        host: Arc<dyn DocumentHost>,
        runtime: &tokio::runtime::Handle,
        start: Start,
    ) -> Answer<Self> {
        let directory = store.association_dir(binding.association_id);
        let mut errors = Vec::new();
        let (journal, damaged_journal) = Journal::open(&directory.join("journal.sqlite"))?;
        if let Some(damage) = damaged_journal {
            errors.push(format!(
                "The sync journal was damaged ({}) and was set aside{}; unconfirmed edits were recovered from the document.",
                damage.reason,
                damage
                    .set_aside
                    .map(|path| format!(" as {}", path.display()))
                    .unwrap_or_default()
            ));
        }
        let (mut mirror, damaged_mirror) = Mirror::open(&directory.join("mirror.sqlite"))?;
        let mut reader = DocumentReader::default();
        let working = reader.read(document)?;
        let rebuild = match start {
            Start::FreshCopy => {
                let mut seed = working.clone();
                seed.show_revision = binding.acknowledged_show_revision;
                mirror.replace(seed)?;
                false
            }
            Start::Reopen => damaged_mirror.is_some() || !mirror.trusted(),
        };
        if let Some(damage) = damaged_mirror {
            errors.push(format!(
                "The confirmed copy of {}'s show was damaged ({}) and is rebuilt from Control on the next connection.",
                binding.desk_name, damage.reason
            ));
        }
        let client = DeskClient::new(binding.base_url().unwrap_or_default())?;
        let shared = Arc::new(Shared {
            binding: Mutex::new(binding),
            store,
            directory,
            journal: Mutex::new(journal),
            mirror: Mutex::new(mirror),
            shadow: Mutex::new(Shadow {
                versioned: working,
                reader,
            }),
            facts: Mutex::new(StatusFacts::default()),
            last_status: Mutex::new(None),
            host,
            client,
            wake: tokio::sync::Notify::new(),
            stopped: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            lose_next_reply: AtomicBool::new(false),
            snapshot_reads: std::sync::atomic::AtomicU64::new(0),
            rebuild_mirror: AtomicBool::new(rebuild),
            known_revisions: Mutex::new(BTreeMap::new()),
        });
        if !rebuild {
            shared.recover_unjournaled(document)?;
        }
        if !errors.is_empty() {
            shared.facts.lock().error = Some(errors.join(" "));
        }
        shared.publish_status();
        let task = runtime.spawn(run::run(shared.clone()));
        Ok(Self {
            shared,
            task: Arc::new(Mutex::new(Some(task))),
        })
    }

    /// Journals what the gesture that just ended changed. Call with the host's gesture lock still
    /// held, after every write of the gesture.
    pub fn capture(&self, document: &PlanningDocument) -> Answer<()> {
        let result = self.shared.capture(document);
        if let Err(error) = &result {
            self.shared.facts.lock().error = Some(format!(
                "An edit could not be recorded for {}: {error}",
                self.shared.binding.lock().desk_name
            ));
        }
        self.shared.publish_status();
        self.shared.wake.notify_one();
        result
    }

    pub fn status(&self) -> SyncStatus {
        self.shared.compute_status()
    }

    pub fn binding(&self) -> SyncBinding {
        self.shared.binding.lock().clone()
    }

    pub fn conflicts(&self) -> Answer<Vec<ConflictView>> {
        self.shared.conflict_views()
    }

    /// Resolves one conflicted or refused entry. Never call inside a gesture.
    pub fn resolve(&self, entry: i64, resolution: Resolution) -> Answer<()> {
        self.shared.resolve(entry, resolution)?;
        self.shared.publish_status();
        self.shared.wake.notify_one();
        Ok(())
    }

    /// Clears a reported error once the operator has read it. Conflicts and refused drafts stay
    /// until they are resolved.
    pub fn dismiss_error(&self) {
        self.shared.facts.lock().error = None;
        self.shared.publish_status();
    }

    /// Works offline, or reconnects. Edits keep being journaled either way.
    pub fn set_online(&self, online: bool) {
        self.shared.paused.store(!online, Ordering::SeqCst);
        if !online {
            self.shared.facts.lock().connection = Connection::Paused;
            self.shared.publish_status();
        }
        self.shared.wake.notify_one();
    }

    /// Stops synchronizing. The journal and mirror stay on disk for the next start.
    pub fn stop(&self) {
        self.shared.stopped.store(true, Ordering::SeqCst);
        self.shared.wake.notify_one();
        if let Some(task) = self.task.lock().take() {
            task.abort();
        }
    }

    /// How many times the engine re-read the whole show from the desk since it started.
    pub fn snapshot_reads(&self) -> u64 {
        self.shared.snapshot_reads.load(Ordering::SeqCst)
    }

    /// Discards the desk's reply to the next transaction after the desk committed it, exactly
    /// as a dropped connection would. For end-to-end tests of the exactly-once retry.
    #[doc(hidden)]
    pub fn lose_next_reply(&self) {
        self.shared.lose_next_reply.store(true, Ordering::SeqCst);
    }
}

impl Shared {
    pub(crate) fn compute_status(&self) -> SyncStatus {
        let counts = self.journal.lock().counts().unwrap_or_default();
        let desk_name = self.binding.lock().desk_name.clone();
        let mut facts = self.facts.lock();
        facts.pending = counts.pending;
        facts.conflicts = counts.conflicts;
        facts.rejected = counts.rejected;
        facts.status(&desk_name)
    }

    pub(crate) fn publish_status(&self) {
        let status = self.compute_status();
        let mut last = self.last_status.lock();
        if last.as_ref() != Some(&status) {
            *last = Some(status.clone());
            drop(last);
            self.host.status_changed(&status);
        }
    }

    pub(crate) fn set_connection(&self, connection: Connection) {
        self.facts.lock().connection = connection;
        self.publish_status();
    }
}

impl Drop for SyncEngine {
    fn drop(&mut self) {
        // The last handle going away stops the task; clones share it.
        if Arc::strong_count(&self.task) == 1 {
            self.stop();
        }
    }
}
