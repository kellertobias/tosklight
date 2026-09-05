//! Making clips resident.
//!
//! Loading is the one point where playback touches storage. Everything after it reads memory, so a
//! clip that is resident cannot stutter because a disk was busy.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use media_codec::container::ClipReader;
use media_codec::{AdmissionError, ClipCache, ContainerError};
use media_domain::AssetId;
use media_domain::timeline::MediaTiming;

/// How far a load has got. Loading a long clip is not instant, so it is reported rather than
/// blocking silently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoadProgress {
    Started { frames: u32, bytes_expected: u64 },
    Finished { frames: u32, bytes: u64 },
}

/// Why a clip could not be made ready.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cannot open {}: {source}", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Container(#[from] ContainerError),
    #[error(transparent)]
    Admission(#[from] AdmissionError),
}

/// What a loaded clip gives playback.
#[derive(Debug, Clone)]
pub struct LoadedClip {
    pub timing: MediaTiming,
    /// Presentation timestamps, one per frame — what a session searches to resolve a position.
    pub presentation_micros: Arc<[u64]>,
    pub width: u32,
    pub height: u32,
}

/// Reads clips from storage into the resident cache.
#[derive(Debug)]
pub struct ClipLoader {
    cache: ClipCache,
    /// Readers kept open for clips too large to be resident, so a streaming frame costs one seek
    /// and one read rather than reopening the file every time.
    streaming: std::collections::HashMap<AssetId, StreamingClip>,
}

/// How many recently read frames a streaming clip keeps.
///
/// Bounded on purpose. A streaming clip is one the cache could not hold, so an unbounded queue
/// here would defeat the budget that sent it down this path in the first place. Four is enough for
/// a hold, a pause, and the turn of a bounce, where the same few frames are asked for repeatedly.
const STREAMING_QUEUE: usize = 4;

/// A clip that did not fit the cache and is read from storage as it plays.
#[derive(Debug)]
struct StreamingClip {
    reader: ClipReader<std::fs::File>,
    /// The frames most recently handed out, newest last. Repeating one — a pause, a hold, a
    /// completed Once, the turn of a bounce — costs no read; anything older is dropped, because a
    /// compositor only ever wants the frame it is about to draw.
    recent: std::collections::VecDeque<(usize, Arc<[u8]>)>,
}

impl StreamingClip {
    fn held(&self, index: usize) -> Option<Arc<[u8]>> {
        self.recent
            .iter()
            .find(|(held, _)| *held == index)
            .map(|(_, frame)| Arc::clone(frame))
    }

    fn hold(&mut self, index: usize, frame: &Arc<[u8]>) {
        self.recent.push_back((index, Arc::clone(frame)));
        while self.recent.len() > STREAMING_QUEUE {
            self.recent.pop_front();
        }
    }
}

impl ClipLoader {
    pub fn new(cache_budget_bytes: u64) -> Self {
        Self {
            cache: ClipCache::new(cache_budget_bytes),
            streaming: std::collections::HashMap::new(),
        }
    }

    /// A frame, wherever it lives.
    ///
    /// Resident clips answer from memory. A clip too large for the budget is read from storage
    /// instead — slower, and still correct, so a big clip plays rather than being refused. The
    /// last streamed frame is held, so a pause or a completed Once costs no read at all.
    pub fn frame(&mut self, asset: AssetId, index: usize) -> Option<Arc<[u8]>> {
        if let Some(frame) = self.cache.frame(asset, index) {
            return Some(frame);
        }
        let streaming = self.streaming.get_mut(&asset)?;
        if let Some(frame) = streaming.held(index) {
            return Some(frame);
        }
        let payload = streaming.reader.frame(index).ok().flatten()?;
        let frame: Arc<[u8]> = Arc::from(payload.into_boxed_slice());
        streaming.hold(index, &frame);
        Some(frame)
    }

    /// How many frames a streaming clip is holding. `None` when it is not streaming.
    ///
    /// Exposed so a test can prove the queue stays bounded; a clip the cache could not hold must
    /// not grow an unbounded one here instead.
    pub fn streaming_frames_held(&self, asset: AssetId) -> Option<usize> {
        self.streaming.get(&asset).map(|clip| clip.recent.len())
    }

    /// Whether a streaming clip is still holding one particular frame.
    pub fn streaming_holds(&self, asset: AssetId, index: usize) -> bool {
        self.streaming
            .get(&asset)
            .is_some_and(|clip| clip.held(index).is_some())
    }

    /// Forgets a clip entirely, resident or streaming.
    pub fn release(&mut self, asset: AssetId) {
        self.cache.remove(asset);
        self.streaming.remove(&asset);
    }

    pub const fn cache(&self) -> &ClipCache {
        &self.cache
    }

    pub const fn cache_mut(&mut self) -> &mut ClipCache {
        &mut self.cache
    }

    /// Loads a clip and makes it resident.
    ///
    /// A clip too large for the whole budget still loads and still plays — it is simply not
    /// cached, and its frames are read from storage as they are needed. Refusing to play a big
    /// clip would be worse than playing it from disk.
    pub fn load(
        &mut self,
        asset: AssetId,
        path: &Path,
        report: &mut dyn FnMut(LoadProgress),
    ) -> Result<LoadedClip, LoadError> {
        let file = std::fs::File::open(path).map_err(|source| LoadError::Unreadable {
            path: path.to_path_buf(),
            source,
        })?;
        let expected = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        let mut reader = ClipReader::open(file)?;

        let header = *reader.header();
        report(LoadProgress::Started {
            frames: header.frame_count,
            bytes_expected: expected,
        });

        let loaded = LoadedClip {
            timing: reader.timing(),
            presentation_micros: Arc::from(
                reader
                    .index()
                    .iter()
                    .map(|entry| entry.presentation_micros)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            width: header.width,
            height: header.height,
        };

        // Decide before reading the payloads: a multi-gigabyte clip must not be allocated
        // in full just to discover that it cannot fit in the resident cache.
        let bytes = reader
            .index()
            .iter()
            .map(|entry| u64::from(entry.length))
            .sum();
        let stream = if bytes > self.cache.budget() {
            true
        } else {
            match self.cache.admit(asset, reader.read_resident()?) {
                Ok(()) => false,
                // Other playing layers are pinned. Keep them intact and stream this selection.
                Err(
                    AdmissionError::LargerThanBudget { .. }
                    | AdmissionError::PinnedClipsFillTheBudget { .. },
                ) => true,
            }
        };
        if stream {
            self.streaming.insert(
                asset,
                StreamingClip {
                    reader,
                    recent: std::collections::VecDeque::with_capacity(STREAMING_QUEUE),
                },
            );
        } else {
            self.streaming.remove(&asset);
        }

        report(LoadProgress::Finished {
            frames: header.frame_count,
            bytes,
        });
        Ok(loaded)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use media_codec::container::{ClipHeader, ClipWriter};

    use super::*;

    fn clip_bytes(frames: usize) -> Vec<u8> {
        let mut writer = ClipWriter::new(
            Cursor::new(Vec::new()),
            ClipHeader {
                width: 16,
                height: 16,
                frame_count: 0,
                frame_rate: (10, 1),
                intrinsic_bpm: None,
            },
        )
        .unwrap();
        for index in 0..frames {
            writer
                .write_frame(&[index as u8; 32], index as u64 * 100_000)
                .unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
        let directory = std::env::temp_dir().join("media-playback-loader-tests");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_loaded_clip_is_resident_and_reports_its_progress() {
        let path = write_temp("resident.toskclip", &clip_bytes(10));
        let mut loader = ClipLoader::new(1_000_000);
        let asset = AssetId::new();

        let mut seen = Vec::new();
        let loaded = loader
            .load(asset, &path, &mut |progress| seen.push(progress))
            .unwrap();

        assert_eq!(loaded.width, 16);
        assert_eq!(loaded.presentation_micros.len(), 10);
        assert_eq!(
            loaded.timing.last_frame,
            std::time::Duration::from_millis(900)
        );
        assert!(matches!(
            seen.first(),
            Some(LoadProgress::Started { frames: 10, .. })
        ));
        assert!(matches!(
            seen.last(),
            Some(LoadProgress::Finished {
                frames: 10,
                bytes: 320
            })
        ));
        assert_eq!(
            loader.cache().residency(asset),
            media_codec::Residency::Resident {
                frames: 10,
                bytes: 320
            }
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_second_clip_streams_when_the_resident_budget_is_pinned() {
        let path = write_temp("pinned-budget-stream.toskclip", &clip_bytes(10));
        let mut loader = ClipLoader::new(320);
        let first = AssetId::new();
        let second = AssetId::new();
        loader.load(first, &path, &mut |_| {}).unwrap();
        loader.cache_mut().pin(first);
        loader.load(second, &path, &mut |_| {}).unwrap();
        assert!(loader.cache().is_pinned(first));
        assert_eq!(loader.cache().used(), 320);
        assert_eq!(loader.frame(second, 7).unwrap().as_ref(), &[7; 32]);
        loader.release(second);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_clip_too_large_for_the_cache_still_loads_and_still_plays() {
        let path = write_temp("oversized.toskclip", &clip_bytes(10));
        // A budget smaller than the clip: it cannot be resident, but it must still be usable.
        let mut loader = ClipLoader::new(64);
        let asset = AssetId::new();

        let loaded = loader.load(asset, &path, &mut |_| {}).unwrap();
        assert_eq!(
            loaded.presentation_micros.len(),
            10,
            "the clip is still playable"
        );
        assert_eq!(
            loader.cache().residency(asset),
            media_codec::Residency::Absent
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn every_frame_of_a_streamed_clip_is_readable_in_any_order() {
        let path = write_temp("streamed-order.toskclip", &clip_bytes(10));
        // A budget far smaller than the clip, so nothing is resident.
        let mut loader = ClipLoader::new(8);
        let asset = AssetId::new();
        loader.load(asset, &path, &mut |_| {}).unwrap();
        assert_eq!(
            loader.cache().residency(asset),
            media_codec::Residency::Absent
        );

        for index in [0usize, 9, 4, 9, 0, 7] {
            assert!(loader.frame(asset, index).is_some(), "frame {index}");
        }
        assert!(loader.frame(asset, 10).is_none(), "past the end");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_streaming_clip_holds_a_bounded_number_of_recent_frames() {
        let path = write_temp("bounded-queue.toskclip", &clip_bytes(32));
        let mut loader = ClipLoader::new(8);
        let asset = AssetId::new();
        loader.load(asset, &path, &mut |_| {}).unwrap();

        // Play well past the queue's length, then ask for the frames around where playback is.
        for index in 0..20 {
            assert!(loader.frame(asset, index).is_some(), "frame {index}");
        }
        let held = loader
            .streaming_frames_held(asset)
            .expect("it is streaming");
        assert_eq!(
            held, STREAMING_QUEUE,
            "a clip too large to cache must not grow an unbounded queue instead"
        );

        // The turn of a bounce asks for the frames just played, which are the ones kept.
        for index in (16..20).rev() {
            assert!(
                loader.frame(asset, index).is_some(),
                "frame {index} on the way back"
            );
        }
        assert_eq!(
            loader.streaming_frames_held(asset),
            Some(STREAMING_QUEUE),
            "and it is still bounded after a reversal"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn an_obsolete_streamed_frame_is_dropped_rather_than_kept_forever() {
        let path = write_temp("obsolete-frame.toskclip", &clip_bytes(32));
        let mut loader = ClipLoader::new(8);
        let asset = AssetId::new();
        loader.load(asset, &path, &mut |_| {}).unwrap();

        loader.frame(asset, 0);
        for index in 1..=STREAMING_QUEUE {
            loader.frame(asset, index);
        }
        assert!(
            !loader.streaming_holds(asset, 0),
            "the frame playback has left behind is gone, not held for the rest of the show"
        );
        assert!(loader.streaming_holds(asset, STREAMING_QUEUE));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repeating_a_streamed_frame_does_not_read_it_again() {
        let path = write_temp("held-frame.toskclip", &clip_bytes(4));
        let mut loader = ClipLoader::new(8);
        let asset = AssetId::new();
        loader.load(asset, &path, &mut |_| {}).unwrap();

        // A pause, a hold, or a completed Once asks for the same frame on every tick.
        let first = loader.frame(asset, 1).unwrap();
        let again = loader.frame(asset, 1).unwrap();
        assert!(
            Arc::ptr_eq(&first, &again),
            "the held frame was reused, not re-read"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_resident_clip_still_answers_from_memory() {
        let path = write_temp("resident-frames.toskclip", &clip_bytes(4));
        let mut loader = ClipLoader::new(1_000_000);
        let asset = AssetId::new();
        loader.load(asset, &path, &mut |_| {}).unwrap();

        assert_ne!(
            loader.cache().residency(asset),
            media_codec::Residency::Absent
        );
        assert_eq!(loader.frame(asset, 2).unwrap().len(), 32);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn releasing_forgets_a_clip_whether_it_was_resident_or_streaming() {
        for budget in [8u64, 1_000_000] {
            let path = write_temp(&format!("released-{budget}.toskclip"), &clip_bytes(4));
            let mut loader = ClipLoader::new(budget);
            let asset = AssetId::new();
            loader.load(asset, &path, &mut |_| {}).unwrap();
            assert!(loader.frame(asset, 0).is_some(), "budget {budget}");

            loader.release(asset);
            assert!(loader.frame(asset, 0).is_none(), "budget {budget}");
            let _ = std::fs::remove_file(path);
        }
    }

    /// The long-running layer-switch stress the contract asks for: a clip is selected, played,
    /// deselected, and reselected many times over, both resident and streaming, and every frame
    /// it hands out stays valid.
    #[test]
    fn switching_between_clips_repeatedly_stays_correct() {
        let paths: Vec<PathBuf> = (0..4)
            .map(|index| write_temp(&format!("stress-{index}.toskclip"), &clip_bytes(10 + index)))
            .collect();
        // Room for two of the four, so eviction runs constantly.
        let mut loader = ClipLoader::new(700);
        let assets: Vec<AssetId> = (0..4).map(|_| AssetId::new()).collect();

        for round in 0..200usize {
            let which = round % assets.len();
            let loaded = loader
                .load(assets[which], &paths[which], &mut |_| {})
                .unwrap();
            let frames = loaded.presentation_micros.len();

            for step in 0..frames {
                let frame = loader.frame(assets[which], step);
                assert!(frame.is_some(), "round {round}, clip {which}, frame {step}");
                assert_eq!(frame.unwrap().len(), 32);
            }
            assert!(
                loader.frame(assets[which], frames).is_none(),
                "round {round}: nothing past the end"
            );
            assert!(
                loader.cache().used() <= loader.cache().budget(),
                "round {round}: over budget"
            );
        }

        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn a_missing_file_reports_where_it_looked() {
        let mut loader = ClipLoader::new(1_000);
        let error = loader
            .load(
                AssetId::new(),
                Path::new("/does/not/exist.toskclip"),
                &mut |_| {},
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("/does/not/exist.toskclip"),
            "{error}"
        );
    }

    #[test]
    fn something_that_is_not_a_clip_reports_rather_than_loading_rubbish() {
        let path = write_temp("rubbish.toskclip", b"definitely not a clip");
        let mut loader = ClipLoader::new(1_000);
        let error = loader.load(AssetId::new(), &path, &mut |_| {}).unwrap_err();
        assert!(matches!(error, LoadError::Container(_)), "{error}");
        let _ = std::fs::remove_file(path);
    }
}
