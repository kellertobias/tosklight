//! Running the imports the queue decides on.
//!
//! [`crate::jobs::ImportQueue`] owns the policy — admission, ordering, cancellation, retention —
//! and runs nothing. This is the part that actually works: a small pool of threads that take the
//! next job, call the codec's importer, and report progress back.
//!
//! Three things shape it.
//!
//! **Import is long and must not be silent.** A show's worth of clips takes minutes, so every job
//! publishes frames done against frames total as it goes, and a failure keeps its reason for
//! somebody to read rather than vanishing.
//!
//! **A cancelled import leaves nothing behind.** The codec writes beside the destination and
//! renames at the end; cancelling stops it at the next frame and the staging file goes with it. A
//! library never contains a half-imported clip that looks playable.
//!
//! **The catalog is republished by whoever owns it.** This adapter does not hold the published
//! snapshot — the runtime does — so a finished import calls back rather than reaching for it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use media_domain::MediaAddress;

use crate::jobs::{ImportQueue, Job, JobId, JobState};
use crate::naming;
use crate::storage::LibraryStorage;

/// Told once after each import that published something, so the catalog can be read again.
pub type Published = Arc<dyn Fn() + Send + Sync>;

/// How many finished jobs stay visible. Enough for an operator to look at what a batch did after
/// it has finished, without the list growing all evening.
const KEEP_FINISHED: usize = 50;

/// The pool that runs imports.
///
/// Cloneable: the API holds one and the workers hold one, and they share the same queue.
#[derive(Clone)]
pub struct Importer {
    inner: Arc<Inner>,
}

struct Inner {
    queue: Mutex<ImportQueue>,
    /// The name each job's imported clip keeps. Settled when the job is submitted rather than when
    /// it runs, so two jobs for one address cannot race to different filenames.
    names: Mutex<HashMap<JobId, String>>,
    storage: LibraryStorage,
    published: Published,
    stopping: AtomicBool,
    /// Woken when work arrives or a worker finishes, so an idle pool costs nothing.
    work: std::sync::Condvar,
    /// Guarded by `work`; separate from the queue lock so a waiting worker holds nothing.
    pending: Mutex<bool>,
}

impl std::fmt::Debug for Importer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Importer")
            .field("jobs", &self.jobs().len())
            .finish_non_exhaustive()
    }
}

impl Importer {
    /// Starts a pool of `concurrency` workers.
    ///
    /// Concurrency is bounded because import is CPU-bound compression: running one per clip on a
    /// forty-clip library would leave nothing for the outputs, and a show may be running.
    pub fn start(storage: LibraryStorage, concurrency: usize, published: Published) -> Self {
        let concurrency = concurrency.clamp(1, 8);
        let importer = Self {
            inner: Arc::new(Inner {
                queue: Mutex::new(ImportQueue::new(concurrency)),
                names: Mutex::new(HashMap::new()),
                storage,
                published,
                stopping: AtomicBool::new(false),
                work: std::sync::Condvar::new(),
                pending: Mutex::new(false),
            }),
        };

        for worker in 0..concurrency {
            let inner = Arc::clone(&importer.inner);
            let _ = std::thread::Builder::new()
                .name(format!("media-import-{worker}"))
                .spawn(move || run_worker(&inner));
        }
        importer
    }

    /// Queues one import and returns its identity.
    pub fn submit(&self, source: PathBuf, destination: MediaAddress, name: &str) -> JobId {
        // The destination filename is settled here, not when the job runs, so two jobs for one
        // address cannot race to different names.
        let id = {
            let mut queue = self.inner.queue.lock().expect("the import queue");
            queue.submit(source, destination)
        };
        self.inner
            .names
            .lock()
            .expect("the import names")
            .insert(id, name.to_owned());
        self.wake();
        id
    }

    /// Every job this run has seen, oldest first.
    pub fn jobs(&self) -> Vec<Job> {
        self.inner
            .queue
            .lock()
            .expect("the import queue")
            .jobs()
            .to_vec()
    }

    /// Stops a job. One still queued never starts; one already running stops at its next frame.
    pub fn cancel(&self, id: JobId) -> bool {
        self.inner
            .queue
            .lock()
            .expect("the import queue")
            .cancel(id)
    }

    /// Asks the workers to finish what they are doing and stop.
    pub fn stop(&self) {
        self.inner.stopping.store(true, Ordering::SeqCst);
        self.wake();
    }

    fn wake(&self) {
        let mut pending = self.inner.pending.lock().expect("the import signal");
        *pending = true;
        self.inner.work.notify_all();
    }
}

/// One worker: take a job, run it, record what happened, repeat.
fn run_worker(inner: &Arc<Inner>) {
    while !inner.stopping.load(Ordering::SeqCst) {
        let next = {
            let mut queue = inner.queue.lock().expect("the import queue");
            queue.next_to_start()
        };
        let Some(job) = next else {
            // Nothing to do. Wait to be woken rather than spinning a core the outputs need.
            let mut pending = inner.pending.lock().expect("the import signal");
            while !*pending && !inner.stopping.load(Ordering::SeqCst) {
                let (guard, timeout) = inner
                    .work
                    .wait_timeout(pending, std::time::Duration::from_millis(250))
                    .expect("the import signal");
                pending = guard;
                if timeout.timed_out() {
                    break;
                }
            }
            *pending = false;
            continue;
        };

        let outcome = run_one(inner, &job);
        let published = matches!(outcome, JobState::Succeeded { .. });
        {
            let mut queue = inner.queue.lock().expect("the import queue");
            queue.finish(job.id, outcome);
            queue.forget_completed(KEEP_FINISHED);
        }
        // The catalog is read again only when something actually arrived in it.
        if published {
            (inner.published)();
        }
        // A freed slot may let a waiting job start.
        let mut pending = inner.pending.lock().expect("the import signal");
        *pending = true;
        inner.work.notify_all();
    }
}

/// Imports one job, reporting progress and honouring cancellation.
fn run_one(inner: &Arc<Inner>, job: &Job) -> JobState {
    let name = inner
        .names
        .lock()
        .expect("the import names")
        .get(&job.id)
        .cloned()
        .unwrap_or_default();
    let destination = inner
        .storage
        .item_path(job.destination, &naming::safe_name(&name));

    if let Err(error) = inner.storage.ensure_folder(job.destination.folder) {
        return JobState::Failed {
            reason: error.to_string(),
        };
    }

    let mut report = |progress: media_codec::import::Progress| {
        let mut queue = inner.queue.lock().expect("the import queue");
        // A job cancelled while it ran is no longer in the queue's running set, and that is the
        // signal to stop: the codec deletes its staging file and leaves the library untouched.
        let still_running = queue
            .job(job.id)
            .is_some_and(|current| matches!(current.state, JobState::Running { .. }));
        if still_running {
            match progress {
                media_codec::import::Progress::Started { frames, .. } => {
                    queue.progress(job.id, 0, frames);
                }
                media_codec::import::Progress::Encoded { frame, total } => {
                    queue.progress(job.id, frame, total);
                }
                media_codec::import::Progress::Finished { .. } => {}
            }
        }
        still_running && !inner.stopping.load(Ordering::SeqCst)
    };

    match media_codec::import::import(&job.source, &destination, &mut report) {
        Ok(0) => JobState::Cancelled,
        Ok(frames) => {
            tracing::info!(
                source = %job.source.display(),
                destination = %destination.display(),
                frames,
                "imported"
            );
            JobState::Succeeded { frames }
        }
        Err(error) => {
            tracing::error!(source = %job.source.display(), %error, "an import failed");
            JobState::Failed {
                reason: error.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library nobody else is using, removed and recreated so a rerun starts clean.
    fn library(name: &str) -> LibraryStorage {
        let root = std::env::temp_dir().join(format!("media-importer-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a library root");
        LibraryStorage::new(root)
    }

    fn importer(storage: LibraryStorage, concurrency: usize) -> (Importer, Arc<AtomicBool>) {
        let published = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&published);
        let importer = Importer::start(
            storage,
            concurrency,
            Arc::new(move || flag.store(true, Ordering::SeqCst)),
        );
        (importer, published)
    }

    /// Waits for every job to reach a terminal state, or gives up.
    fn settle(importer: &Importer) -> Vec<Job> {
        for _ in 0..600 {
            let jobs = importer.jobs();
            if !jobs.is_empty() && jobs.iter().all(|job| job.state.is_terminal()) {
                return jobs;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        importer.jobs()
    }

    /// Whether this machine can actually transcode. Import shells out to FFmpeg, so a machine
    /// without it can prove the queueing and the failure path but not a real conversion.
    fn has_ffmpeg() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Two seconds of colour bars, made by FFmpeg because a real import needs a real source.
    fn source(storage: &LibraryStorage, filename: &str) -> PathBuf {
        let path = storage.root().join(filename);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("a folder");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=64x64:rate=10:duration=1",
            ])
            .arg(&path)
            .status()
            .expect("ffmpeg runs");
        assert!(made.success(), "ffmpeg made a source");
        path
    }

    #[test]
    fn a_submitted_job_is_visible_before_anything_has_run() {
        let storage = library("queued");
        // No workers can start it, because nothing has been given to them that exists.
        let (importer, _) = importer(storage, 1);
        let id = importer.submit(
            PathBuf::from("/does/not/exist/001.mp4"),
            MediaAddress::new(1, 1),
            "Missing",
        );

        let jobs = settle(&importer);
        let job = jobs.iter().find(|job| job.id == id).expect("the job");
        assert!(
            matches!(job.state, JobState::Failed { .. }),
            "a source that is not there fails with a reason rather than vanishing: {:?}",
            job.state
        );
        importer.stop();
    }

    #[test]
    fn a_queued_job_that_is_cancelled_never_runs() {
        let storage = library("cancelled");
        let (importer, published) = importer(storage, 1);
        // Submitted against a stopped pool, so it cannot be picked up before it is cancelled.
        importer.stop();
        let id = importer.submit(
            PathBuf::from("/does/not/exist/001.mp4"),
            MediaAddress::new(1, 1),
            "Missing",
        );

        assert!(importer.cancel(id), "a queued job can be stopped");
        assert!(
            !importer.cancel(id),
            "and stopping it twice is not a second stop"
        );
        let job = importer
            .jobs()
            .into_iter()
            .find(|job| job.id == id)
            .expect("the job");
        assert_eq!(job.state, JobState::Cancelled);
        assert!(
            !published.load(Ordering::SeqCst),
            "nothing was published, so the catalog was not re-read"
        );
    }

    #[test]
    fn a_real_source_becomes_a_playable_clip_at_its_address() {
        if !has_ffmpeg() {
            eprintln!("skipped: this machine has no FFmpeg, so nothing can be transcoded");
            return;
        }
        let storage = library("imported");
        let source = source(&storage, "001/001-Bars.mp4");
        let (importer, published) = importer(storage.clone(), 1);

        importer.submit(source, MediaAddress::new(1, 1), "Bars");
        let jobs = settle(&importer);
        importer.stop();

        assert!(
            matches!(jobs[0].state, JobState::Succeeded { frames } if frames > 0),
            "{:?}",
            jobs[0].state
        );
        assert!(
            published.load(Ordering::SeqCst),
            "the catalog is re-read once something has arrived in it"
        );

        // The clip is where the address says, and discovery can read it.
        let catalog = crate::discover(storage.root()).expect("a library");
        let item = catalog
            .resolve(MediaAddress::new(1, 1))
            .expect("the imported clip is addressable");
        assert_eq!(item.name, "Bars", "the name in the filename is kept");
        assert!(
            crate::pending_imports(storage.root()).is_empty(),
            "and it is no longer waiting to be imported"
        );
    }

    #[test]
    fn a_batch_drains_and_leaves_nothing_waiting() {
        if !has_ffmpeg() {
            eprintln!("skipped: this machine has no FFmpeg, so nothing can be transcoded");
            return;
        }
        let storage = library("batch");
        for (folder, file) in [(1u8, 1u8), (1, 2), (2, 1)] {
            source(&storage, &format!("{folder:03}/{file:03}-Clip.mp4"));
        }
        let pending = crate::pending_imports(storage.root());
        assert_eq!(pending.len(), 3);

        let (importer, _) = importer(storage.clone(), 2);
        for item in pending {
            importer.submit(item.source, item.destination, &item.name);
        }
        let jobs = settle(&importer);
        importer.stop();

        assert_eq!(jobs.len(), 3);
        assert!(
            jobs.iter()
                .all(|job| matches!(job.state, JobState::Succeeded { .. })),
            "{jobs:?}"
        );
        assert_eq!(crate::discover(storage.root()).unwrap().item_count(), 3);
        assert!(crate::pending_imports(storage.root()).is_empty());
    }
}
