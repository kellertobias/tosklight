//! CPU-only immutable latest pictures; no renderer, atlas, device or redraw dependency.
use super::*;
use std::time::Instant;
use viz_render::media::{FrozenMedia, FrozenMediaSource, LAST_GOOD_HOLD, MAX_SOURCES};
use viz_scene::uuid::Uuid;

#[derive(Default)]
pub(super) struct CaptureCache {
    frames: HashMap<Uuid, (Arc<MediaFrame>, Instant)>,
    overflow: bool,
}
impl CaptureCache {
    pub fn retain(&mut self, wanted: &HashMap<Uuid, &MediaSourceBinding>) {
        self.frames.retain(|id, _| wanted.contains_key(id));
        self.overflow = wanted.len() > (MAX_SOURCES - 1) as usize;
    }
    pub fn remove(&mut self, id: Uuid) {
        self.frames.remove(&id);
    }
    pub fn matches_fallback(&self, id: Uuid, rgba: &[u8]) -> bool {
        self.frames
            .get(&id)
            .is_some_and(|(frame, _)| frame.persistent && frame.rgba.as_slice() == rgba)
    }
    pub fn adopt(&mut self, frame: MediaFrame, now: Instant) {
        if !self.frames.contains_key(&frame.source_id)
            && self.frames.len() >= (MAX_SOURCES - 1) as usize
        {
            self.overflow = true;
            return;
        }
        self.frames.insert(frame.source_id, (Arc::new(frame), now));
    }
    pub fn snapshot(&self, now: Instant) -> Result<FrozenMedia, String> {
        if self.overflow {
            return Err("Media capture exceeds the native source budget".into());
        }
        FrozenMedia::from_sources(
            self.frames
                .values()
                .map(|(frame, received)| FrozenMediaSource {
                    frame: Arc::clone(frame),
                    available: frame.persistent
                        || now.saturating_duration_since(*received) <= LAST_GOOD_HOLD,
                    flicker: 0.0,
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(id: u128, pixel: u8) -> MediaFrame {
        MediaFrame {
            source_id: Uuid::from_u128(id),
            sequence: 1,
            width: EDGE,
            height: EDGE,
            rgba: vec![pixel; (EDGE * EDGE * 4) as usize],
            persistent: false,
        }
    }
    #[test]
    fn freezes_pixels_and_availability_independently_of_later_frames_and_expiry() {
        let mut cache = CaptureCache::default();
        let now = Instant::now();
        cache.adopt(frame(1, 12), now);
        let frozen = cache.snapshot(now).unwrap();
        let edge = cache.snapshot(now + LAST_GOOD_HOLD).unwrap();
        assert!(edge.sources()[0].available);
        let expired = cache
            .snapshot(now + LAST_GOOD_HOLD + Duration::from_millis(1))
            .unwrap();
        assert!(!expired.sources()[0].available);
        cache.adopt(frame(1, 99), now + Duration::from_secs(3));
        assert_eq!(frozen.sources()[0].frame.rgba[0], 12);
        assert!(frozen.sources()[0].available);
        assert_eq!(
            cache
                .snapshot(now + Duration::from_secs(3))
                .unwrap()
                .sources()[0]
                .frame
                .rgba[0],
            99
        );
    }
    #[test]
    fn before_initial_readiness_is_empty_and_persistent_frames_do_not_expire() {
        let mut cache = CaptureCache::default();
        let now = Instant::now();
        assert!(cache.snapshot(now).unwrap().sources().is_empty());
        let mut fallback = frame(1, 30);
        fallback.persistent = true;
        cache.adopt(fallback, now);
        assert!(
            cache
                .snapshot(now + Duration::from_secs(100))
                .unwrap()
                .sources()[0]
                .available
        );
    }
    #[test]
    fn bounded_cache_fails_explicitly_and_invalid_pixels_never_become_a_valid_snapshot() {
        let now = Instant::now();
        let mut cache = CaptureCache::default();
        for id in 1..=MAX_SOURCES {
            cache.adopt(frame(id as u128, 1), now);
        }
        assert_eq!(cache.frames.len(), (MAX_SOURCES - 1) as usize);
        assert!(cache.snapshot(now).is_err());
        let mut cache = CaptureCache::default();
        let mut invalid = frame(1, 1);
        invalid.width = 1;
        cache.adopt(invalid, now);
        assert!(cache.snapshot(now).is_err());
    }
    #[test]
    fn frozen_constructor_rejects_duplicate_identity_nonfinite_flicker_and_budget_overflow() {
        let source = FrozenMediaSource {
            frame: Arc::new(frame(1, 1)),
            available: true,
            flicker: 0.0,
        };
        assert!(FrozenMedia::from_sources(vec![source.clone(), source.clone()]).is_err());
        let mut invalid = source.clone();
        invalid.flicker = f32::NAN;
        assert!(FrozenMedia::from_sources(vec![invalid]).is_err());
        assert!(FrozenMedia::from_sources(vec![source; MAX_SOURCES as usize]).is_err());
    }
}
