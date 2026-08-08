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
}

impl ClipLoader {
    pub fn new(cache_budget_bytes: u64) -> Self {
        Self {
            cache: ClipCache::new(cache_budget_bytes),
        }
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

        let resident = reader.read_resident()?;
        let bytes = resident.bytes();
        match self.cache.admit(asset, resident) {
            Ok(()) => {}
            Err(AdmissionError::LargerThanBudget { needed, budget }) => {
                tracing::warn!(
                    %asset,
                    needed,
                    budget,
                    "the clip is larger than the whole cache budget; it will stream from storage"
                );
            }
            Err(error) => return Err(error.into()),
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
