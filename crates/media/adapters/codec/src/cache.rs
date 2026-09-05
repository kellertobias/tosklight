//! The resident-clip cache.
//!
//! A show must not depend on how fast a disk feels tonight. Once a clip is resident, playback
//! reads frames out of memory and never touches storage again — no stutter from a slow SD card, a
//! spinning disk waking up, or another process saturating the bus.
//!
//! Frames are cached **still Snappy-compressed**, exactly as they sit on disk. Measured over real
//! footage that is 6–50% of the fixed BC3 size, so the same memory holds two to sixteen times more
//! clip, and Snappy decompresses at gigabytes per second — a fraction of one core even for two
//! 1080p60 layers. Trading a little CPU for that much residency is the right way round.

use std::collections::HashMap;
use std::sync::Arc;

use media_domain::AssetId;

/// One clip's frames, held in memory.
#[derive(Debug, Clone)]
pub struct ResidentClip {
    frames: Vec<Arc<[u8]>>,
    bytes: u64,
}

impl ResidentClip {
    pub fn new(frames: Vec<Arc<[u8]>>) -> Self {
        let bytes = frames.iter().map(|frame| frame.len() as u64).sum();
        Self { frames, bytes }
    }

    /// A frame by index. Random access is the whole point: reverse, bounce, a speed change, and a
    /// seek are all just a different index.
    pub fn frame(&self, index: usize) -> Option<&Arc<[u8]>> {
        self.frames.get(index)
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Why a clip could not be made resident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionError {
    #[error("the clip needs {needed} bytes but the whole cache budget is {budget}")]
    LargerThanBudget { needed: u64, budget: u64 },
    #[error(
        "the clip needs {needed} bytes and only {available} can be freed; {pinned} bytes are \
         pinned by outputs currently using them"
    )]
    PinnedClipsFillTheBudget {
        needed: u64,
        available: u64,
        pinned: u64,
    },
}

/// Whether a clip is in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Residency {
    Absent,
    Resident { frames: usize, bytes: u64 },
}

/// An in-memory store of decoded-ready clips, bounded by a byte budget.
///
/// Eviction is least-recently-used, and a clip a live layer has selected is pinned and never
/// evicted — dropping the clip that is currently on screen to make room for one that is not would
/// be exactly the wrong trade.
#[derive(Debug)]
pub struct ClipCache {
    budget: u64,
    used: u64,
    clips: HashMap<AssetId, ResidentClip>,
    /// Least recently used first.
    recency: Vec<AssetId>,
    pinned: HashMap<AssetId, u32>,
}

impl ClipCache {
    pub fn new(budget_bytes: u64) -> Self {
        Self {
            budget: budget_bytes,
            used: 0,
            clips: HashMap::new(),
            recency: Vec::new(),
            pinned: HashMap::new(),
        }
    }

    pub const fn budget(&self) -> u64 {
        self.budget
    }

    pub const fn used(&self) -> u64 {
        self.used
    }

    pub const fn available(&self) -> u64 {
        self.budget.saturating_sub(self.used)
    }

    pub fn residency(&self, asset: AssetId) -> Residency {
        match self.clips.get(&asset) {
            Some(clip) => Residency::Resident {
                frames: clip.frame_count(),
                bytes: clip.bytes(),
            },
            None => Residency::Absent,
        }
    }

    /// Makes a clip resident, evicting unpinned clips by least-recent use if it does not fit.
    ///
    /// A clip larger than the whole budget is refused rather than admitted and immediately thrashed
    /// — playback streams that one from disk instead of evicting everything else for a clip that
    /// still will not fit.
    pub fn admit(&mut self, asset: AssetId, clip: ResidentClip) -> Result<(), AdmissionError> {
        let needed = clip.bytes();
        if needed > self.budget {
            return Err(AdmissionError::LargerThanBudget {
                needed,
                budget: self.budget,
            });
        }

        // Re-admitting replaces, so its old bytes are not counted against the new copy.
        self.remove(asset);

        while self.used + needed > self.budget {
            if !self.evict_least_recent() {
                let pinned = self.pinned_bytes();
                return Err(AdmissionError::PinnedClipsFillTheBudget {
                    needed,
                    available: self.budget.saturating_sub(pinned),
                    pinned,
                });
            }
        }

        self.used += needed;
        self.clips.insert(asset, clip);
        self.recency.push(asset);
        Ok(())
    }

    /// Makes room for an in-flight resident load without evicting any playing clip.
    pub fn make_room(&mut self, needed: u64) -> bool {
        if needed > self.budget {
            return false;
        }
        while self.available() < needed {
            if !self.evict_least_recent() {
                return false;
            }
        }
        true
    }

    /// A frame from a resident clip, marking the clip as recently used.
    ///
    /// Returns `None` when the clip is not resident; the caller streams from storage instead.
    pub fn frame(&mut self, asset: AssetId, index: usize) -> Option<Arc<[u8]>> {
        let frame = self.clips.get(&asset)?.frame(index).cloned();
        if frame.is_some() {
            self.touch(asset);
        }
        frame
    }

    /// Marks a clip as in use by an output. Pinned clips are never evicted.
    pub fn pin(&mut self, asset: AssetId) {
        *self.pinned.entry(asset).or_insert(0) += 1;
        self.touch(asset);
    }

    /// Releases one pin. A clip is evictable again once every output has released it.
    pub fn unpin(&mut self, asset: AssetId) {
        if let Some(count) = self.pinned.get_mut(&asset) {
            *count -= 1;
            if *count == 0 {
                self.pinned.remove(&asset);
            }
        }
    }

    pub fn is_pinned(&self, asset: AssetId) -> bool {
        self.pinned.contains_key(&asset)
    }

    /// Drops a clip whether or not it is pinned. For a clip that has been deleted or replaced in
    /// the library, where keeping the old bytes would be worse than a reload.
    pub fn remove(&mut self, asset: AssetId) -> bool {
        let Some(clip) = self.clips.remove(&asset) else {
            return false;
        };
        self.used -= clip.bytes();
        self.recency.retain(|candidate| *candidate != asset);
        self.pinned.remove(&asset);
        true
    }

    fn touch(&mut self, asset: AssetId) {
        if let Some(position) = self
            .recency
            .iter()
            .position(|candidate| *candidate == asset)
        {
            let entry = self.recency.remove(position);
            self.recency.push(entry);
        }
    }

    fn evict_least_recent(&mut self) -> bool {
        let Some(asset) = self
            .recency
            .iter()
            .copied()
            .find(|candidate| !self.pinned.contains_key(candidate))
        else {
            return false;
        };
        self.remove(asset)
    }

    fn pinned_bytes(&self) -> u64 {
        self.pinned
            .keys()
            .filter_map(|asset| self.clips.get(asset))
            .map(ResidentClip::bytes)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(frames: usize, frame_bytes: usize) -> ResidentClip {
        ResidentClip::new(
            (0..frames)
                .map(|index| Arc::from(vec![index as u8; frame_bytes].into_boxed_slice()))
                .collect(),
        )
    }

    #[test]
    fn a_resident_clip_serves_frames_from_memory_in_any_order() {
        let mut cache = ClipCache::new(1_000);
        let asset = AssetId::new();
        cache.admit(asset, clip(10, 10)).unwrap();

        // Forward, backward, and a jump — all the same cost, which is why reverse and bounce are
        // ordinary here.
        for index in [0, 9, 4, 9, 0] {
            assert!(cache.frame(asset, index).is_some(), "frame {index}");
        }
        assert!(cache.frame(asset, 10).is_none(), "past the end");
    }

    #[test]
    fn residency_reports_what_is_in_memory() {
        let mut cache = ClipCache::new(1_000);
        let asset = AssetId::new();
        assert_eq!(cache.residency(asset), Residency::Absent);

        cache.admit(asset, clip(4, 25)).unwrap();
        assert_eq!(
            cache.residency(asset),
            Residency::Resident {
                frames: 4,
                bytes: 100
            }
        );
        assert_eq!(cache.used(), 100);
        assert_eq!(cache.available(), 900);
    }

    #[test]
    fn a_clip_larger_than_the_budget_is_refused_rather_than_thrashed() {
        let mut cache = ClipCache::new(100);
        let small = AssetId::new();
        cache.admit(small, clip(1, 50)).unwrap();

        let huge = AssetId::new();
        let error = cache.admit(huge, clip(10, 50)).unwrap_err();
        assert_eq!(
            error,
            AdmissionError::LargerThanBudget {
                needed: 500,
                budget: 100
            }
        );
        assert_eq!(
            cache.residency(small),
            Residency::Resident {
                frames: 1,
                bytes: 50
            },
            "refusing must not have evicted anything"
        );
    }

    #[test]
    fn the_least_recently_used_clip_is_evicted_first() {
        let mut cache = ClipCache::new(100);
        let (first, second, third) = (AssetId::new(), AssetId::new(), AssetId::new());
        cache.admit(first, clip(1, 40)).unwrap();
        cache.admit(second, clip(1, 40)).unwrap();

        // Touching the first makes the second the least recent.
        cache.frame(first, 0).unwrap();

        cache.admit(third, clip(1, 40)).unwrap();
        assert_eq!(
            cache.residency(second),
            Residency::Absent,
            "the least recent went"
        );
        assert_ne!(cache.residency(first), Residency::Absent);
        assert_ne!(cache.residency(third), Residency::Absent);
    }

    #[test]
    fn a_clip_an_output_is_using_is_never_evicted() {
        let mut cache = ClipCache::new(100);
        let live = AssetId::new();
        let idle = AssetId::new();
        cache.admit(live, clip(1, 40)).unwrap();
        cache.admit(idle, clip(1, 40)).unwrap();
        cache.pin(live);

        // Even though `live` was admitted first and so is the least recent.
        cache.admit(AssetId::new(), clip(1, 40)).unwrap();
        assert_ne!(
            cache.residency(live),
            Residency::Absent,
            "the clip on screen stayed"
        );
        assert_eq!(cache.residency(idle), Residency::Absent);
    }

    #[test]
    fn a_budget_full_of_pinned_clips_reports_rather_than_evicting_a_live_one() {
        let mut cache = ClipCache::new(100);
        let first = AssetId::new();
        let second = AssetId::new();
        cache.admit(first, clip(1, 50)).unwrap();
        cache.admit(second, clip(1, 50)).unwrap();
        cache.pin(first);
        cache.pin(second);

        let error = cache.admit(AssetId::new(), clip(1, 50)).unwrap_err();
        assert_eq!(
            error,
            AdmissionError::PinnedClipsFillTheBudget {
                needed: 50,
                available: 0,
                pinned: 100
            }
        );
    }

    #[test]
    fn unpinning_makes_a_clip_evictable_again() {
        let mut cache = ClipCache::new(100);
        let asset = AssetId::new();
        cache.admit(asset, clip(1, 60)).unwrap();
        cache.pin(asset);
        assert!(cache.is_pinned(asset));

        cache.unpin(asset);
        assert!(!cache.is_pinned(asset));
        cache.admit(AssetId::new(), clip(1, 60)).unwrap();
        assert_eq!(cache.residency(asset), Residency::Absent);
    }

    #[test]
    fn two_outputs_using_one_clip_both_have_to_release_it() {
        let mut cache = ClipCache::new(100);
        let asset = AssetId::new();
        cache.admit(asset, clip(1, 60)).unwrap();
        cache.pin(asset);
        cache.pin(asset);

        cache.unpin(asset);
        assert!(cache.is_pinned(asset), "one output is still showing it");
        cache.unpin(asset);
        assert!(!cache.is_pinned(asset));
    }

    #[test]
    fn re_admitting_a_clip_replaces_it_without_double_counting() {
        let mut cache = ClipCache::new(1_000);
        let asset = AssetId::new();
        cache.admit(asset, clip(2, 50)).unwrap();
        cache.admit(asset, clip(4, 50)).unwrap();

        assert_eq!(cache.used(), 200, "the old copy's bytes were released");
        assert_eq!(
            cache.residency(asset),
            Residency::Resident {
                frames: 4,
                bytes: 200
            }
        );
    }

    #[test]
    fn removing_a_replaced_asset_frees_its_memory_even_while_pinned() {
        let mut cache = ClipCache::new(100);
        let asset = AssetId::new();
        cache.admit(asset, clip(1, 80)).unwrap();
        cache.pin(asset);

        assert!(cache.remove(asset));
        assert_eq!(cache.used(), 0);
        assert!(!cache.is_pinned(asset));
        assert!(
            !cache.remove(asset),
            "removing twice is not an error, just nothing to do"
        );
    }

    #[test]
    fn a_miss_returns_nothing_so_the_caller_can_stream_instead() {
        let mut cache = ClipCache::new(100);
        assert!(cache.frame(AssetId::new(), 0).is_none());
    }

    #[test]
    fn admission_and_eviction_keep_the_accounting_exact() {
        let mut cache = ClipCache::new(500);
        let mut assets = Vec::new();
        for _ in 0..20 {
            let asset = AssetId::new();
            assets.push(asset);
            cache.admit(asset, clip(2, 60)).unwrap();
            assert!(cache.used() <= cache.budget(), "the budget was exceeded");
        }
        let counted: u64 = assets
            .iter()
            .filter_map(|asset| match cache.residency(*asset) {
                Residency::Resident { bytes, .. } => Some(bytes),
                Residency::Absent => None,
            })
            .sum();
        assert_eq!(
            counted,
            cache.used(),
            "used bytes match what is actually resident"
        );
    }
}
