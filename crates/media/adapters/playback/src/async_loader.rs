//! Bounded disk workers. The presentation thread only polls results and reads resident bytes.
use crate::{MediaLoader, loader::LoadedClip};
use media_codec::{ClipCache, ResidentClip, container::ClipReader};
use media_domain::{AssetId, SourceFailure};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread::JoinHandle,
};

const WORKERS: usize = 2;
const MAX_REQUESTS: usize = 256;
const RECENT_FRAMES: usize = 4;
type Reader = Arc<Mutex<ClipReader<File>>>;
type RecentFrames = VecDeque<(usize, Arc<[u8]>)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    asset: AssetId,
    generation: u64,
    frame: Option<usize>,
    consumer: u64,
    resident: bool,
}
struct Job {
    key: Key,
    work: Work,
}
enum Work {
    Load(PathBuf),
    Resident(Reader),
    Frame(Option<Reader>, usize),
}
struct PreparedClip {
    loaded: LoadedClip,
    reader: Option<Reader>,
    resident: Option<ResidentClip>,
    bytes: u64,
}
enum Answer {
    Loaded(PreparedClip),
    Resident(ResidentClip),
    Frame(Arc<[u8]>),
}
struct ResultMessage {
    key: Key,
    result: Result<Answer, SourceFailure>,
}
#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    active: HashSet<(AssetId, u64)>,
    shutdown: bool,
    frame_streak: usize,
}

impl Queue {
    fn take_ready(&mut self) -> Option<Job> {
        let ready = |job: &Job| !self.active.contains(&(job.key.asset, job.key.generation));
        let ordinary = self.jobs.iter().position(ready)?;
        // A busy rig must not starve a newly selected source behind endless streaming reads.
        let index = if self.frame_streak >= 8 {
            self.jobs
                .iter()
                .position(|job| job.key.frame.is_none() && ready(job))
                .unwrap_or(ordinary)
        } else {
            ordinary
        };
        let job = self.jobs.remove(index)?;
        self.active.insert((job.key.asset, job.key.generation));
        self.frame_streak = if job.key.frame.is_some() {
            self.frame_streak + 1
        } else {
            0
        };
        Some(job)
    }
}

trait Disk: Send + Sync + 'static {
    fn load(&self, path: &Path, resident_limit: u64) -> Result<PreparedClip, SourceFailure>;
    fn frame(&self, reader: Option<&Reader>, index: usize) -> Result<Arc<[u8]>, SourceFailure>;
}
struct FileDisk;
impl Disk for FileDisk {
    fn load(&self, path: &Path, resident_limit: u64) -> Result<PreparedClip, SourceFailure> {
        let file = File::open(path).map_err(|error| {
            tracing::warn!(path = %path.display(), %error, "cannot open media");
            SourceFailure::MissingFile
        })?;
        let mut reader = ClipReader::open(file).map_err(|error| {
            tracing::warn!(path = %path.display(), %error, "cannot read media container");
            SourceFailure::DecodeFailed
        })?;
        let header = *reader.header();
        let loaded = LoadedClip {
            timing: reader.timing(),
            presentation_micros: reader
                .index()
                .iter()
                .map(|entry| entry.presentation_micros)
                .collect::<Vec<_>>()
                .into(),
            width: header.width,
            height: header.height,
        };
        let bytes: u64 = reader
            .index()
            .iter()
            .map(|entry| u64::from(entry.length))
            .sum();
        let resident = if bytes <= resident_limit {
            Some(
                reader
                    .read_resident()
                    .map_err(|_| SourceFailure::DecodeFailed)?,
            )
        } else {
            None
        };
        Ok(PreparedClip {
            loaded,
            resident,
            bytes,
            reader: Some(Arc::new(Mutex::new(reader))),
        })
    }
    fn frame(&self, reader: Option<&Reader>, index: usize) -> Result<Arc<[u8]>, SourceFailure> {
        let reader = reader.ok_or(SourceFailure::MissingFile)?;
        let frame = reader
            .lock()
            .map_err(|_| SourceFailure::DecodeFailed)?
            .frame(index)
            .map_err(|_| SourceFailure::DecodeFailed)?
            .ok_or(SourceFailure::DecodeFailed)?;
        Ok(frame.into())
    }
}

struct Asset {
    generation: u64,
    references: usize,
    loaded: Option<LoadedClip>,
    reader: Option<Reader>,
    recent: HashMap<u64, RecentFrames>,
    failure: Option<SourceFailure>,
    path: Option<PathBuf>,
    bytes: u64,
    residency_attempted: bool,
}

pub struct AsyncClipLoader {
    cache: ClipCache,
    assets: HashMap<AssetId, Asset>,
    next_generation: u64,
    pending: HashSet<Key>,
    reservations: HashMap<Key, u64>,
    queue: Arc<(Mutex<Queue>, Condvar)>,
    results: Option<mpsc::Receiver<ResultMessage>>,
    workers: Vec<JoinHandle<()>>,
    #[cfg(test)]
    discarded_results: usize,
}

impl AsyncClipLoader {
    pub fn new(budget: u64) -> Self {
        Self::with_disk(budget, Arc::new(FileDisk))
    }

    fn with_disk(budget: u64, disk: Arc<dyn Disk>) -> Self {
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        // At most two completed loads wait for presentation, and each resident load is bounded.
        let (sender, results) = mpsc::sync_channel(WORKERS);
        let mut workers = Vec::new();
        for index in 0..WORKERS {
            let queue = Arc::clone(&queue);
            let sender = sender.clone();
            let disk = Arc::clone(&disk);
            match std::thread::Builder::new()
                .name(format!("media-disk-{index}"))
                .spawn(move || {
                    loop {
                        let job = {
                            let (lock, wake) = &*queue;
                            let Ok(mut queue) = lock.lock() else { return };
                            loop {
                                if queue.shutdown {
                                    return;
                                }
                                if let Some(job) = queue.take_ready() {
                                    break job;
                                }
                                let Ok(next) = wake.wait(queue) else { return };
                                queue = next;
                            }
                        };
                        let result = match job.work {
                            Work::Load(path) => disk.load(&path, 0).map(Answer::Loaded),
                            Work::Resident(reader) => reader
                                .lock()
                                .map_err(|_| SourceFailure::DecodeFailed)
                                .and_then(|mut reader| {
                                    reader
                                        .read_resident()
                                        .map_err(|_| SourceFailure::DecodeFailed)
                                })
                                .map(Answer::Resident),
                            Work::Frame(reader, index) => {
                                disk.frame(reader.as_ref(), index).map(Answer::Frame)
                            }
                        };
                        if let Ok(mut queued) = queue.0.lock() {
                            queued.active.remove(&(job.key.asset, job.key.generation));
                            queue.1.notify_all();
                        }
                        if sender
                            .send(ResultMessage {
                                key: job.key,
                                result,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }) {
                Ok(worker) => workers.push(worker),
                Err(error) => tracing::error!(%error, "cannot start media disk worker"),
            }
        }
        Self {
            cache: ClipCache::new(budget),
            assets: HashMap::new(),
            next_generation: 0,
            pending: HashSet::new(),
            reservations: HashMap::new(),
            queue,
            results: Some(results),
            workers,
            #[cfg(test)]
            discarded_results: 0,
        }
    }

    fn poll(&mut self) {
        while let Some(result) = self
            .results
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.pending.remove(&result.key);
            self.reservations.remove(&result.key);
            let Some(asset) = self
                .assets
                .get_mut(&result.key.asset)
                .filter(|asset| asset.generation == result.key.generation)
            else {
                #[cfg(test)]
                {
                    self.discarded_results += 1;
                }
                continue;
            };
            match result.result {
                Ok(Answer::Loaded(prepared)) => {
                    asset.loaded = Some(prepared.loaded);
                    asset.reader = prepared.reader;
                    asset.bytes = prepared.bytes;
                    if let Some(resident) = prepared.resident
                        && self.cache.admit(result.key.asset, resident).is_ok()
                    {
                        for _ in 0..asset.references {
                            self.cache.pin(result.key.asset);
                        }
                    }
                }
                Ok(Answer::Resident(resident)) => {
                    if self.cache.admit(result.key.asset, resident).is_ok() {
                        for _ in 0..asset.references {
                            self.cache.pin(result.key.asset);
                        }
                    }
                }
                Ok(Answer::Frame(frame)) => {
                    let index = result.key.frame.expect("frame result has index");
                    let Some(recent) = asset.recent.get_mut(&result.key.consumer) else {
                        continue;
                    };
                    recent.retain(|(held, _)| *held != index);
                    recent.push_back((index, frame));
                    while recent.len() > RECENT_FRAMES {
                        recent.pop_front();
                    }
                }
                Err(failure) => {
                    tracing::warn!(asset = %result.key.asset, ?failure, "media disk request failed");
                    asset.failure = Some(failure);
                }
            }
        }
    }

    fn cancel_queued(&mut self, asset: AssetId, consumer: Option<u64>) {
        if let Ok(mut queue) = self.queue.0.try_lock() {
            queue.jobs.retain(|job| {
                let remove = job.key.asset == asset
                    && consumer.is_none_or(|consumer| {
                        job.key.consumer == consumer && job.key.frame.is_some()
                    });
                if remove {
                    self.reservations.remove(&job.key);
                }
                !remove
            });
        }
    }

    fn warm(&mut self, asset: AssetId) {
        let Some(entry) = self.assets.get(&asset) else {
            return;
        };
        if entry.residency_attempted || entry.loaded.is_none() || entry.failure.is_some() {
            return;
        }
        let Some(reader) = entry.reader.clone() else {
            return;
        };
        let bytes = entry.bytes;
        if bytes > self.cache.budget() {
            return;
        }
        let reserved: u64 = self.reservations.values().sum();
        if !self.cache.make_room(reserved.saturating_add(bytes)) {
            return;
        }
        let key = Key {
            asset,
            generation: entry.generation,
            frame: None,
            consumer: 0,
            resident: true,
        };
        self.enqueue(key, Work::Resident(reader));
        if self.pending.contains(&key) {
            self.reservations.insert(key, bytes);
            self.assets.get_mut(&asset).unwrap().residency_attempted = true;
        }
    }

    fn enqueue(&mut self, key: Key, work: Work) {
        if self.pending.contains(&key) || self.pending.len() >= MAX_REQUESTS {
            return;
        }
        // A disk worker never holds this lock during I/O; still use try_lock so presentation
        // has no dependency on worker scheduling, even for the short queue operation.
        let (lock, wake) = &*self.queue;
        let Ok(mut queue) = lock.try_lock() else {
            return;
        };
        if queue.jobs.len() >= MAX_REQUESTS || queue.shutdown {
            return;
        }
        if key.frame.is_some() {
            // Keep a bounded recent window per asset, replacing obsolete queued seeks rather
            // than letting a slow disk accumulate seconds of frames nobody still wants.
            let same_asset = |job: &Job| {
                job.key.asset == key.asset
                    && job.key.generation == key.generation
                    && job.key.frame.is_some()
                    && job.key.consumer == key.consumer
            };
            if queue.jobs.iter().filter(|job| same_asset(job)).count() >= RECENT_FRAMES
                && let Some(index) = queue.jobs.iter().position(same_asset)
                && let Some(stale) = queue.jobs.remove(index)
            {
                self.pending.remove(&stale.key);
            }
        }
        let job = Job { key, work };
        // Frame reads take priority over cold loads, keeping already-playing siblings moving.
        if key.frame.is_some() {
            let before_load = queue
                .jobs
                .iter()
                .position(|queued| queued.key.frame.is_none())
                .unwrap_or(queue.jobs.len());
            queue.jobs.insert(before_load, job);
        } else {
            queue.jobs.push_back(job);
        }
        self.pending.insert(key);
        wake.notify_one();
    }
}

impl MediaLoader for AsyncClipLoader {
    fn begin_selection(&mut self, asset: AssetId) {
        self.poll();
        self.next_generation = self.next_generation.wrapping_add(1);
        let entry = self.assets.entry(asset).or_insert_with(|| Asset {
            generation: self.next_generation,
            references: 0,
            loaded: None,
            reader: None,
            recent: HashMap::new(),
            failure: None,
            path: None,
            bytes: 0,
            residency_attempted: false,
        });
        entry.references += 1;
        if matches!(
            self.cache.residency(asset),
            media_codec::Residency::Resident { .. }
        ) {
            self.cache.pin(asset);
        }
    }
    fn request_load(
        &mut self,
        asset: AssetId,
        path: &Path,
    ) -> Result<Option<LoadedClip>, SourceFailure> {
        self.poll();
        if self.workers.is_empty() {
            return Err(SourceFailure::MissingFile);
        }
        let Some(entry) = self.assets.get_mut(&asset) else {
            return Err(SourceFailure::MissingFile);
        };
        entry.path = Some(path.to_path_buf());
        if let Some(failure) = entry.failure {
            return Err(failure);
        }
        if let Some(loaded) = &entry.loaded {
            let loaded = loaded.clone();
            self.warm(asset);
            return Ok(Some(loaded));
        }
        let key = Key {
            asset,
            generation: entry.generation,
            frame: None,
            consumer: 0,
            resident: false,
        };
        self.enqueue(key, Work::Load(path.to_path_buf()));
        Ok(None)
    }
    fn finish_selection(&mut self, _asset: AssetId) {}
    fn release_selection(&mut self, asset: AssetId) {
        let Some(entry) = self.assets.get_mut(&asset) else {
            return;
        };
        entry.references = entry.references.saturating_sub(1);
        self.cache.unpin(asset);
        if entry.references != 0 {
            return;
        }
        self.assets.remove(&asset);
        // Discard queued work and ignore any in-flight response using the generation above.
        self.pending.retain(|key| key.asset != asset);
        self.cancel_queued(asset, None);
    }
    fn request_frame(
        &mut self,
        asset: AssetId,
        frame: usize,
        consumer: u64,
    ) -> Option<(usize, Arc<[u8]>)> {
        self.poll();
        if let Some(payload) = self.cache.frame(asset, frame) {
            return Some((frame, payload));
        }
        let entry = self.assets.get(&asset)?;
        if entry.loaded.is_none() {
            let path = entry.path.clone()?;
            let _ = self.request_load(asset, &path);
            return None;
        }
        self.warm(asset);
        let entry = self.assets.get_mut(&asset)?;
        if entry.failure.is_some() {
            return None;
        }
        let recent = entry.recent.entry(consumer).or_default();
        if let Some((_, payload)) = recent.iter().find(|(held, _)| *held == frame) {
            return Some((frame, Arc::clone(payload)));
        }
        let key = Key {
            asset,
            generation: entry.generation,
            frame: Some(frame),
            consumer,
            resident: false,
        };
        let reader = entry.reader.clone();
        let fallback = recent
            .iter()
            .min_by_key(|(index, _)| index.abs_diff(frame))
            .map(|(index, payload)| (*index, Arc::clone(payload)));
        self.enqueue(key, Work::Frame(reader, frame));
        fallback
    }
    fn release_consumer(&mut self, consumer: u64) {
        for asset in self.assets.values_mut() {
            asset.recent.remove(&consumer);
        }
        self.pending
            .retain(|key| key.consumer != consumer || key.frame.is_none());
        if let Ok(mut queue) = self.queue.0.try_lock() {
            queue
                .jobs
                .retain(|job| job.key.consumer != consumer || job.key.frame.is_none());
        }
    }
    fn retry(&mut self, asset: AssetId) {
        if let Some(entry) = self.assets.get_mut(&asset) {
            self.next_generation = self.next_generation.wrapping_add(1);
            entry.generation = self.next_generation;
            entry.failure = None;
            entry.loaded = None;
            entry.reader = None;
            entry.recent.clear();
            entry.bytes = 0;
            entry.residency_attempted = false;
            self.cache.remove(asset);
            self.pending.retain(|key| key.asset != asset);
            self.cancel_queued(asset, None);
        }
    }
    fn failure(&self, asset: AssetId) -> Option<SourceFailure> {
        self.assets.get(&asset).and_then(|entry| entry.failure)
    }
}

impl Drop for AsyncClipLoader {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.queue.0.lock() {
            queue.shutdown = true;
            queue.jobs.clear();
        }
        self.queue.1.notify_all();
        // Unblock workers publishing a result before joining them. In-flight OS reads finish;
        // queued requests are cancelled and no result can reach a stopped presentation loop.
        self.results.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::timeline::MediaTiming;
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        wake: Condvar,
    }
    impl Gate {
        fn wait(&self) {
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.wake.wait(open).unwrap();
            }
        }
        fn release(&self) {
            *self.open.lock().unwrap() = true;
            self.wake.notify_all();
        }
    }
    struct Release(Arc<Gate>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.release();
        }
    }
    struct TestDisk {
        gate: Arc<Gate>,
        started: mpsc::Sender<&'static str>,
    }
    impl Disk for TestDisk {
        fn load(&self, path: &Path, _limit: u64) -> Result<PreparedClip, SourceFailure> {
            let slow = path == Path::new("slow");
            if slow {
                self.started.send("load").unwrap();
                self.gate.wait();
            }
            Ok(PreparedClip {
                loaded: LoadedClip {
                    timing: MediaTiming::from_frames(10, 10.0),
                    presentation_micros: (0..10)
                        .map(|frame| frame * 100_000)
                        .collect::<Vec<_>>()
                        .into(),
                    width: 16,
                    height: 16,
                },
                reader: None,
                bytes: 4,
                resident: (path == Path::new("resident"))
                    .then(|| ResidentClip::new(vec![Arc::from([42u8; 4])])),
            })
        }
        fn frame(
            &self,
            _reader: Option<&Reader>,
            index: usize,
        ) -> Result<Arc<[u8]>, SourceFailure> {
            if index == 7 {
                self.started.send("frame").unwrap();
                self.gate.wait();
            }
            Ok(Arc::from([index as u8; 4]))
        }
    }
    fn await_loaded(loader: &mut AsyncClipLoader, asset: AssetId, path: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while loader
            .request_load(asset, Path::new(path))
            .unwrap()
            .is_none()
        {
            assert!(Instant::now() < deadline, "load worker must answer");
            std::thread::yield_now();
        }
    }
    fn await_started(
        loader: &mut AsyncClipLoader,
        asset: AssetId,
        path: &str,
        started: &mpsc::Receiver<&'static str>,
    ) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            loader.request_load(asset, Path::new(path)).unwrap();
            if started.try_recv().is_ok() {
                return;
            }
            assert!(Instant::now() < deadline, "worker starts gated load");
            std::thread::yield_now();
        }
    }

    fn bench() -> (AsyncClipLoader, Arc<Gate>, mpsc::Receiver<&'static str>) {
        let gate = Arc::new(Gate::default());
        let (started, receiver) = mpsc::channel();
        let loader = AsyncClipLoader::with_disk(
            1024,
            Arc::new(TestDisk {
                gate: gate.clone(),
                started,
            }),
        );
        (loader, gate, receiver)
    }

    fn await_frame(
        loader: &mut AsyncClipLoader,
        asset: AssetId,
        frame: usize,
        consumer: u64,
    ) -> Arc<[u8]> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some((actual, payload)) = loader.request_frame(asset, frame, consumer)
                && actual == frame
            {
                return payload;
            }
            assert!(Instant::now() < deadline, "frame worker answers");
            std::thread::yield_now();
        }
    }

    #[test]
    fn streaming_fallbacks_are_independent_for_two_layers_on_one_asset() {
        let (mut loader, gate, _) = bench();
        let _release = Release(gate);
        let asset = AssetId::new();
        loader.begin_selection(asset);
        await_loaded(&mut loader, asset, "stream");
        await_frame(&mut loader, asset, 3, 11);
        await_frame(&mut loader, asset, 9, 22);
        assert_eq!(loader.request_frame(asset, 4, 11).unwrap().0, 3);
        assert_eq!(loader.request_frame(asset, 10, 22).unwrap().0, 9);
        loader.release_consumer(11);
        assert!(
            loader.request_frame(asset, 5, 33).is_none(),
            "a new consumer never inherits another layer's frame"
        );
    }

    #[test]
    fn retry_recovers_a_failed_stream_while_another_layer_retains_it() {
        struct FailingDisk {
            fail: Arc<std::sync::atomic::AtomicBool>,
        }
        impl Disk for FailingDisk {
            fn load(&self, _: &Path, _: u64) -> Result<PreparedClip, SourceFailure> {
                Ok(PreparedClip {
                    loaded: LoadedClip {
                        timing: MediaTiming::from_frames(1, 30.0),
                        presentation_micros: Arc::from([0]),
                        width: 16,
                        height: 16,
                    },
                    reader: None,
                    resident: None,
                    bytes: 4,
                })
            }
            fn frame(&self, _: Option<&Reader>, _: usize) -> Result<Arc<[u8]>, SourceFailure> {
                if self.fail.load(std::sync::atomic::Ordering::Relaxed) {
                    Err(SourceFailure::DecodeFailed)
                } else {
                    Ok(Arc::from([42; 4]))
                }
            }
        }
        let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut loader =
            AsyncClipLoader::with_disk(1, Arc::new(FailingDisk { fail: fail.clone() }));
        let asset = AssetId::new();
        loader.begin_selection(asset);
        loader.begin_selection(asset);
        await_loaded(&mut loader, asset, "stream");
        let deadline = Instant::now() + Duration::from_secs(2);
        while loader.failure(asset).is_none() {
            loader.request_frame(asset, 0, 11);
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        fail.store(false, std::sync::atomic::Ordering::Relaxed);
        loader.retry(asset);
        assert_eq!(loader.assets[&asset].references, 2);
        await_loaded(&mut loader, asset, "stream");
        assert_eq!(await_frame(&mut loader, asset, 0, 11).as_ref(), &[42; 4]);
    }

    #[test]
    fn continuous_frame_traffic_does_not_starve_a_cold_selection() {
        let mut queue = Queue::default();
        let cold = AssetId::new();
        for index in 0..16 {
            queue.jobs.push_back(Job {
                key: Key {
                    asset: AssetId::new(),
                    generation: 1,
                    frame: Some(index),
                    consumer: 1,
                    resident: false,
                },
                work: Work::Frame(None, index),
            });
        }
        queue.jobs.push_back(Job {
            key: Key {
                asset: cold,
                generation: 1,
                frame: None,
                consumer: 0,
                resident: false,
            },
            work: Work::Load(PathBuf::from("cold")),
        });
        let mut seen_cold = false;
        for _ in 0..9 {
            let job = queue.take_ready().unwrap();
            seen_cold |= job.key.asset == cold;
        }
        assert!(
            seen_cold,
            "cold work is selected after at most eight preferred frame jobs"
        );
    }

    #[test]
    fn cold_io_never_blocks_selection_or_a_resident_sibling() {
        let (mut loader, gate, started) = bench();
        let _release = Release(gate);
        let healthy = AssetId::new();
        loader.begin_selection(healthy);
        await_loaded(&mut loader, healthy, "resident");
        let slow = AssetId::new();
        loader.begin_selection(slow);
        assert!(
            loader
                .request_load(slow, Path::new("slow"))
                .unwrap()
                .is_none()
        );
        await_started(&mut loader, slow, "slow", &started);
        let before = Instant::now();
        for _ in 0..100 {
            assert!(
                loader
                    .request_load(slow, Path::new("slow"))
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                loader.request_frame(healthy, 0, 0).unwrap().1.as_ref(),
                &[42; 4]
            );
        }
        assert!(
            before.elapsed() < Duration::from_millis(100),
            "render calls must not wait on the closed disk gate"
        );
    }

    #[test]
    fn a_streaming_read_is_pending_while_a_healthy_sibling_keeps_rendering() {
        let (mut loader, gate, started) = bench();
        let _release = Release(gate);
        let stream = AssetId::new();
        let healthy = AssetId::new();
        loader.begin_selection(stream);
        loader.begin_selection(healthy);
        await_loaded(&mut loader, stream, "stream");
        await_loaded(&mut loader, healthy, "resident");
        assert!(loader.request_frame(stream, 7, 0).is_none());
        let deadline = Instant::now() + Duration::from_secs(2);
        while started.try_recv().is_err() {
            loader.request_frame(stream, 7, 0);
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(loader.request_frame(stream, 7, 0).is_none());
        assert_eq!(
            loader.request_frame(healthy, 0, 0).unwrap().1.as_ref(),
            &[42; 4]
        );
    }

    #[test]
    fn deselection_ignores_an_in_flight_old_generation_and_releases_shared_pins() {
        let (mut loader, gate, started) = bench();
        let _release = Release(gate.clone());
        let asset = AssetId::new();
        loader.begin_selection(asset);
        loader.request_load(asset, Path::new("slow")).unwrap();
        await_started(&mut loader, asset, "slow", &started);
        loader.release_selection(asset);
        loader.begin_selection(asset);
        await_loaded(&mut loader, asset, "resident");
        gate.release();
        let deadline = Instant::now() + Duration::from_secs(2);
        while loader.discarded_results == 0 {
            loader.poll();
            assert!(
                Instant::now() < deadline,
                "obsolete worker reply arrives and is discarded"
            );
            std::thread::yield_now();
        }
        assert_eq!(
            loader.request_frame(asset, 0, 0).unwrap().1.as_ref(),
            &[42; 4]
        );
        loader.begin_selection(asset);
        loader.release_selection(asset);
        assert!(loader.cache.is_pinned(asset));
        loader.release_selection(asset);
        assert!(!loader.cache.is_pinned(asset));
        assert!(!loader.assets.contains_key(&asset));
    }

    #[test]
    fn outstanding_work_is_bounded_when_storage_is_slow() {
        let (mut loader, gate, started) = bench();
        let _release = Release(gate);
        for _ in 0..2 {
            let asset = AssetId::new();
            loader.begin_selection(asset);
            await_started(&mut loader, asset, "slow", &started);
        }
        for _ in 0..MAX_REQUESTS * 2 {
            let asset = AssetId::new();
            loader.begin_selection(asset);
            loader.request_load(asset, Path::new("resident")).unwrap();
        }
        assert!(loader.pending.len() <= MAX_REQUESTS);
        assert!(loader.queue.0.lock().unwrap().jobs.len() <= MAX_REQUESTS);
    }
}
