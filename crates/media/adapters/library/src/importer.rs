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
pub type Published = Arc<dyn Fn(MediaAddress) + Send + Sync>;

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
    names: Mutex<HashMap<JobId, ImportDetails>>,
    storage: LibraryStorage,
    published: Published,
    stopping: AtomicBool,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    startup_failure: Mutex<Option<String>>,
    /// Woken when work arrives or a worker finishes, so an idle pool costs nothing.
    work: std::sync::Condvar,
    /// Guarded by `work`; separate from the queue lock so a waiting worker holds nothing.
    pending: Mutex<bool>,
}

struct ImportDetails {
    name: String,
    // A finished browser upload owns the address lease until its job has actually stopped.
    _upload: Option<crate::Upload>,
    _reservation: Option<crate::uploads::Reservation>,
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
        Self::start_with_spawner(storage, concurrency, published, |worker, inner| {
            std::thread::Builder::new()
                .name(format!("media-import-{worker}"))
                .spawn(move || run_worker(&inner))
        })
    }

    fn start_with_spawner(
        storage: LibraryStorage,
        concurrency: usize,
        published: Published,
        mut spawn: impl FnMut(usize, Arc<Inner>) -> std::io::Result<std::thread::JoinHandle<()>>,
    ) -> Self {
        let concurrency = concurrency.clamp(1, 8);
        let importer = Self {
            inner: Arc::new(Inner {
                queue: Mutex::new(ImportQueue::new(concurrency)),
                names: Mutex::new(HashMap::new()),
                storage,
                published,
                stopping: AtomicBool::new(false),
                workers: Mutex::new(Vec::new()),
                startup_failure: Mutex::new(None),
                work: std::sync::Condvar::new(),
                pending: Mutex::new(false),
            }),
        };

        for worker in 0..concurrency {
            match spawn(worker, Arc::clone(&importer.inner)) {
                Ok(handle) => importer
                    .inner
                    .workers
                    .lock()
                    .expect("import workers")
                    .push(handle),
                Err(error) => {
                    let reason = format!("cannot start media import worker: {error}");
                    tracing::error!(%reason);
                    *importer
                        .inner
                        .startup_failure
                        .lock()
                        .expect("import startup failure") = Some(reason);
                    importer.stop();
                    break;
                }
            }
        }
        importer
    }

    /// Queues one import and returns its identity.
    pub fn submit(&self, source: PathBuf, destination: MediaAddress, name: &str) -> JobId {
        let reservation = self
            .inner
            .storage
            .ensure_folder(destination.folder)
            .map_err(|error| error.to_string())
            .and_then(|()| {
                crate::uploads::Reservation::acquire(
                    &self
                        .inner
                        .storage
                        .root()
                        .join(naming::folder_directory(destination.folder)),
                    destination,
                )
                .map_err(|error| error.to_string())
            });
        let reservation = match reservation {
            Ok(reservation) => reservation,
            Err(reason) => {
                let mut queue = self.inner.queue.lock().expect("the import queue");
                let id = queue.submit(source, destination);
                queue.finish(id, JobState::Failed { reason });
                return id;
            }
        };
        self.submit_with_details(
            source,
            destination,
            ImportDetails {
                name: name.to_owned(),
                _upload: None,
                _reservation: Some(reservation),
            },
        )
    }

    pub(crate) fn submit_upload(
        &self,
        source: PathBuf,
        destination: MediaAddress,
        name: &str,
        upload: crate::Upload,
    ) -> JobId {
        self.submit_with_details(
            source,
            destination,
            ImportDetails {
                name: name.to_owned(),
                _upload: Some(upload),
                _reservation: None,
            },
        )
    }

    fn submit_with_details(
        &self,
        source: PathBuf,
        destination: MediaAddress,
        details: ImportDetails,
    ) -> JobId {
        // The destination filename is settled here, not when the job runs, so two jobs for one
        // address cannot race to different names.
        let id = {
            let mut queue = self.inner.queue.lock().expect("the import queue");
            let id = queue.submit(source, destination);
            let unavailable = self
                .inner
                .startup_failure
                .lock()
                .expect("import startup failure")
                .clone()
                .or_else(|| {
                    self.inner
                        .stopping
                        .load(Ordering::SeqCst)
                        .then(|| "the media importer has stopped".to_owned())
                });
            if let Some(reason) = unavailable {
                queue.finish(id, JobState::Failed { reason });
                return id;
            }
            // Publish the name before releasing the queue: an already awake worker may claim
            // this job immediately, without waiting for our wake notification.
            self.inner
                .names
                .lock()
                .expect("the import names")
                .insert(id, details);
            id
        };
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
        let mut queue = self.inner.queue.lock().expect("the import queue");
        let was_queued = queue
            .job(id)
            .is_some_and(|job| job.state == JobState::Queued);
        let cancelled = queue.cancel(id);
        if cancelled && was_queued {
            self.inner
                .names
                .lock()
                .expect("the import names")
                .remove(&id);
            queue.forget_completed(KEEP_FINISHED);
        }
        cancelled
    }

    /// Cancels queued/active work and joins workers after their decoder and staging cleanup.
    /// Decoder subprocess stalls are supervised; a kernel filesystem stall can still delay exit.
    pub fn stop(&self) {
        self.inner.stopping.store(true, Ordering::SeqCst);
        // Queued uploads will never get a worker after shutdown, so release their leases now.
        for job in self.jobs() {
            self.cancel(job.id);
        }
        self.wake();
        let mut workers = self.inner.workers.lock().expect("import workers");
        for worker in workers.drain(..) {
            // A user-supplied publication callback cannot synchronously join its own thread.
            if worker.thread().id() == std::thread::current().id() {
                continue;
            }
            if worker.join().is_err() {
                tracing::error!("a media import worker panicked during shutdown");
            }
        }
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

        let details = inner
            .names
            .lock()
            .expect("the import names")
            .remove(&job.id)
            .expect("a submitted import has its details");
        let outcome = run_one(inner, &job, &details);
        // Keep the address reserved until the catalog reflects the published clip as well.
        if matches!(outcome, JobState::Succeeded { .. }) {
            (inner.published)(job.destination);
        }
        drop(details);
        {
            let mut queue = inner.queue.lock().expect("the import queue");
            queue.finish(job.id, outcome);
            queue.forget_completed(KEEP_FINISHED);
        }
        // A freed slot may let a waiting job start.
        let mut pending = inner.pending.lock().expect("the import signal");
        *pending = true;
        inner.work.notify_all();
    }
}

/// Imports one job, reporting progress and honouring cancellation.
fn run_one(inner: &Arc<Inner>, job: &Job, details: &ImportDetails) -> JobState {
    let name = &details.name;
    // Replacements keep the occupied slot's filename so discovery cannot pick an older duplicate.
    let existing_name = match crate::discover(inner.storage.root()) {
        Ok(catalog) => catalog
            .resolve(job.destination)
            .map(|item| item.name.clone()),
        Err(error) => {
            return JobState::Failed {
                reason: error.to_string(),
            };
        }
    };
    let destination = inner.storage.item_path(
        job.destination,
        &naming::safe_name(existing_name.as_deref().unwrap_or(name)),
    );

    if let Err(error) = inner.storage.ensure_folder(job.destination.folder) {
        return JobState::Failed {
            reason: error.to_string(),
        };
    }

    let mut report = |progress: media_codec::import::Progress| {
        let mut queue = inner.queue.lock().expect("the import queue");
        // Cancellation changes the visible state but reserves the worker/address until this
        // import returns and releases its staging file.
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

    let cancellation_inner = Arc::clone(inner);
    let id = job.id;
    let cancelled: media_codec::import::Cancellation = Arc::new(move || {
        cancellation_inner.stopping.load(Ordering::SeqCst)
            || cancellation_inner
                .queue
                .lock()
                .expect("the import queue")
                .job(id)
                .is_none_or(|job| !matches!(job.state, JobState::Running { .. }))
    });
    match media_codec::import::import_cancellable(
        &job.source,
        &destination,
        &mut report,
        Arc::clone(&cancelled),
    ) {
        Ok(0) => JobState::Cancelled,
        Ok(frames) => {
            let extension = job
                .source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let is_video = !["png", "jpg", "jpeg", "tif"].contains(&extension.as_str());
            if let Err(error) = crate::thumbnails::generate_cancellable(
                &inner.storage,
                job.destination,
                &job.source,
                is_video,
                cancelled,
            ) {
                // The clip is already complete and playable. Keep that successful import and
                // expose the UI's explicit missing-thumbnail state rather than lying that the
                // whole conversion failed after publication.
                tracing::warn!(
                    source = %job.source.display(),
                    destination = %destination.display(),
                    %error,
                    "the imported clip has no thumbnail"
                );
            }
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
        configured_importer(storage, concurrency, false)
    }

    fn idle_importer(storage: LibraryStorage, concurrency: usize) -> (Importer, Arc<AtomicBool>) {
        configured_importer(storage, concurrency, true)
    }

    fn configured_importer(
        storage: LibraryStorage,
        concurrency: usize,
        idle: bool,
    ) -> (Importer, Arc<AtomicBool>) {
        let published = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&published);
        let library_root = storage.root().to_owned();
        let published_callback: Published = Arc::new(move |address| {
            assert!(
                crate::uploads::guard_idle_addresses(&library_root, &[address]).is_err(),
                "publication still owns the address reservation"
            );
            flag.store(true, Ordering::SeqCst);
        });
        let importer = if idle {
            Importer::start_with_spawner(storage, concurrency, published_callback, |_, _| {
                std::thread::Builder::new().spawn(|| {})
            })
        } else {
            Importer::start(storage, concurrency, published_callback)
        };
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
    fn worker_start_failure_is_visible_and_releases_the_submission_lease() {
        let storage = library("spawn-failure");
        let importer =
            Importer::start_with_spawner(storage.clone(), 1, Arc::new(|_| {}), |_, _| {
                Err(std::io::Error::other("simulated worker exhaustion"))
            });
        let address = MediaAddress::new(1, 1);
        let id = importer.submit(PathBuf::from("source.mp4"), address, "Clip");
        assert!(
            matches!(&importer.jobs().iter().find(|job| job.id == id).unwrap().state,
            JobState::Failed { reason } if reason.contains("simulated worker exhaustion"))
        );
        assert!(crate::uploads::guard_idle_addresses(storage.root(), &[address]).is_ok());
        assert!(importer.inner.names.lock().unwrap().is_empty());
    }

    #[test]
    fn stopping_joins_active_conversion_and_releases_its_files_and_address() {
        if !has_ffmpeg() {
            return;
        }
        let storage = library("joined-stop");
        let source = storage.root().join("long.mp4");
        assert!(
            std::process::Command::new("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=256x256:rate=30:duration=20"
                ])
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        let (importer, _) = importer(storage.clone(), 1);
        let address = MediaAddress::new(1, 1);
        let id = importer.submit(source, address, "Long");
        let started = std::time::Instant::now();
        loop {
            let jobs = importer.jobs();
            let job = jobs.iter().find(|job| job.id == id).unwrap();
            if matches!(job.state, JobState::Running { frames_done, .. } if frames_done > 0) {
                break;
            }
            assert!(
                !job.state.is_terminal(),
                "conversion must still be active: {:?}",
                job.state
            );
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        importer.stop();
        assert!(importer.inner.workers.lock().unwrap().is_empty());
        assert_eq!(
            importer
                .jobs()
                .iter()
                .find(|job| job.id == id)
                .unwrap()
                .state,
            JobState::Cancelled
        );
        assert!(crate::uploads::guard_idle_addresses(storage.root(), &[address]).is_ok());
        let destination = storage.item_path(address, "Long");
        assert!(!destination.exists());
        assert!(
            !destination
                .with_file_name("001-Long.toskclip.importing")
                .exists()
        );
        let rejected = importer.submit(PathBuf::from("another.mp4"), address, "Another");
        assert!(
            matches!(&importer.jobs().iter().find(|job| job.id == rejected).unwrap().state,
            JobState::Failed { reason } if reason.contains("stopped"))
        );
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
        assert!(
            importer.inner.names.lock().unwrap().is_empty(),
            "finished names are released"
        );
        importer.stop();
    }

    #[test]
    fn a_queued_job_that_is_cancelled_never_runs() {
        let storage = library("cancelled");
        let (importer, published) = idle_importer(storage, 1);
        // Submitted against an idle test pool, so it cannot be picked up before it is cancelled.
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
            importer.inner.names.lock().unwrap().is_empty(),
            "cancelled queued names are released"
        );
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

        let address = MediaAddress::new(1, 1);
        let mut upload =
            crate::Upload::begin_replacement(storage.root(), address, "Bars", "bars.mp4").unwrap();
        upload.write(&std::fs::read(source).unwrap()).unwrap();
        upload
            .finish_and_import(&importer, address, "Bars")
            .unwrap();
        let jobs = settle(&importer);
        assert!(
            crate::Upload::begin_replacement(storage.root(), address, "Bars", "bars.mp4").is_ok(),
            "successful conversion releases the upload lease"
        );
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
            crate::thumbnails::exists(&storage, MediaAddress::new(1, 1)),
            "the browser and CITP can identify the imported clip visually"
        );
        assert!(
            crate::pending_imports(storage.root()).is_empty(),
            "and it is no longer waiting to be imported"
        );
    }

    #[test]
    fn local_source_jobs_exclude_uploads_and_edits_until_cancelled() {
        let storage = library("local-source-lease");
        let (importer, _) = idle_importer(storage.clone(), 1);
        let address = MediaAddress::new(1, 1);
        let first = importer.submit(PathBuf::from("source.mp4"), address, "Clip");
        let duplicate = importer.submit(PathBuf::from("other.mp4"), address, "Other");
        assert!(matches!(
            importer
                .jobs()
                .iter()
                .find(|job| job.id == duplicate)
                .unwrap()
                .state,
            JobState::Failed { .. }
        ));
        assert!(
            crate::Upload::begin_replacement(storage.root(), address, "Clip", "clip.mp4").is_err()
        );
        assert!(crate::uploads::guard_idle_addresses(storage.root(), &[address]).is_err());
        assert!(importer.cancel(first));
        assert!(
            importer
                .inner
                .queue
                .lock()
                .unwrap()
                .next_to_start()
                .is_none(),
            "an admission failure never remains runnable"
        );
        assert!(crate::uploads::guard_idle_addresses(storage.root(), &[address]).is_ok());
        assert!(
            crate::Upload::begin_replacement(storage.root(), address, "Clip", "clip.mp4").is_ok()
        );
    }

    #[test]
    fn queued_uploaded_sources_are_reserved_until_cancelled() {
        let storage = library("queued-upload-lease");
        let (importer, _) = idle_importer(storage.clone(), 1);
        let address = MediaAddress::new(1, 1);
        let mut upload = crate::Upload::begin(storage.root(), address, "Clip", "clip.mp4").unwrap();
        upload.write(b"original uploaded source").unwrap();
        let id = upload
            .finish_and_import(&importer, address, "Clip")
            .unwrap();
        assert!(
            crate::Upload::begin_replacement(storage.root(), address, "Clip", "clip.mp4").is_err()
        );
        assert!(importer.cancel(id));
        assert!(
            crate::Upload::begin_replacement(storage.root(), address, "Clip", "clip.mp4").is_ok()
        );
    }

    #[test]
    fn running_uploaded_sources_keep_the_lease_until_the_worker_returns() {
        for cancelled in [false, true] {
            let storage = library(if cancelled {
                "cancelled-upload-lease"
            } else {
                "failed-upload-lease"
            });
            let (importer, _) = idle_importer(storage.clone(), 1);
            let address = MediaAddress::new(1, 1);
            let mut upload =
                crate::Upload::begin(storage.root(), address, "Clip", "clip.mp4").unwrap();
            upload
                .write(b"invalid source that will fail conversion")
                .unwrap();
            let id = upload
                .finish_and_import(&importer, address, "Clip")
                .unwrap();
            let job = importer
                .inner
                .queue
                .lock()
                .unwrap()
                .next_to_start()
                .unwrap();
            if cancelled {
                assert!(importer.cancel(id));
            }
            assert!(
                crate::Upload::begin_replacement(storage.root(), address, "Clip", "clip.mp4")
                    .is_err()
            );
            let details = importer
                .inner
                .names
                .lock()
                .unwrap()
                .remove(&job.id)
                .unwrap();
            let outcome = run_one(&importer.inner, &job, &details);
            if cancelled {
                assert_eq!(outcome, JobState::Cancelled);
            } else {
                assert!(matches!(outcome, JobState::Failed { .. }));
            }
            drop(details);
            assert!(
                crate::Upload::begin_replacement(storage.root(), address, "Clip", "clip.mp4")
                    .is_ok()
            );
        }
    }

    #[test]
    fn a_replacement_with_a_new_name_replaces_the_existing_clip() {
        if !has_ffmpeg() {
            return;
        }
        let storage = library("replacement");
        let source = source(&storage, "source.mp4");
        let (importer, _) = importer(storage.clone(), 2);
        importer.submit(source.clone(), MediaAddress::new(1, 1), "Original");
        settle(&importer);
        importer.submit(source, MediaAddress::new(1, 1), "Replacement");
        let jobs = settle(&importer);
        importer.stop();
        assert!(
            jobs.iter()
                .all(|job| matches!(job.state, JobState::Succeeded { .. })),
            "{jobs:?}"
        );
        let catalog = crate::discover(storage.root()).unwrap();
        assert_eq!(catalog.item_count(), 1);
        assert_eq!(
            catalog.resolve(MediaAddress::new(1, 1)).unwrap().name,
            "Original"
        );
        assert!(
            !storage
                .item_path(MediaAddress::new(1, 1), "Replacement")
                .exists()
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
