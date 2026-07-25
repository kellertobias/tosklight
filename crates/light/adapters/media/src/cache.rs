use crate::{CachedImage, MediaError, MediaImage, PreviewKey, ThumbnailKey};
use std::{
    collections::{HashMap, VecDeque},
    time::SystemTime,
};

pub struct MediaCache {
    thumbnails: HashMap<ThumbnailKey, CachedImage>,
    previews: HashMap<PreviewKey, CachedImage>,
    thumbnail_order: VecDeque<ThumbnailKey>,
    preview_order: VecDeque<PreviewKey>,
    thumbnail_limit: usize,
    preview_limit: usize,
}

impl Default for MediaCache {
    fn default() -> Self {
        Self::new(512, 32)
    }
}

impl MediaCache {
    pub fn new(thumbnail_limit: usize, preview_limit: usize) -> Self {
        Self {
            thumbnails: HashMap::new(),
            previews: HashMap::new(),
            thumbnail_order: VecDeque::new(),
            preview_order: VecDeque::new(),
            thumbnail_limit: thumbnail_limit.max(1),
            preview_limit: preview_limit.max(1),
        }
    }

    pub fn put_thumbnail(
        &mut self,
        key: ThumbnailKey,
        image: MediaImage,
    ) -> Result<(), MediaError> {
        image.validate()?;
        touch(&mut self.thumbnail_order, &key);
        self.thumbnails.insert(key, cached(image));
        evict(
            &mut self.thumbnails,
            &mut self.thumbnail_order,
            self.thumbnail_limit,
        );
        Ok(())
    }

    pub fn put_preview(&mut self, key: PreviewKey, image: MediaImage) -> Result<(), MediaError> {
        image.validate()?;
        touch(&mut self.preview_order, &key);
        self.previews.insert(key, cached(image));
        evict(
            &mut self.previews,
            &mut self.preview_order,
            self.preview_limit,
        );
        Ok(())
    }

    pub fn thumbnail(&mut self, key: &ThumbnailKey) -> Option<CachedImage> {
        let value = self.thumbnails.get(key)?.clone();
        touch(&mut self.thumbnail_order, key);
        Some(value)
    }

    pub fn preview(&mut self, key: &PreviewKey) -> Option<CachedImage> {
        let value = self.previews.get(key)?.clone();
        touch(&mut self.preview_order, key);
        Some(value)
    }

    pub fn clear_fixture(&mut self, fixture: &str) {
        self.thumbnails.retain(|key, _| key.fixture != fixture);
        self.previews.retain(|key, _| key.fixture != fixture);
        self.thumbnail_order.retain(|key| key.fixture != fixture);
        self.preview_order.retain(|key| key.fixture != fixture);
    }

    pub fn retain_fixtures(&mut self, fixtures: &std::collections::HashSet<String>) {
        self.thumbnails
            .retain(|key, _| fixtures.contains(&key.fixture));
        self.previews
            .retain(|key, _| fixtures.contains(&key.fixture));
        self.thumbnail_order
            .retain(|key| fixtures.contains(&key.fixture));
        self.preview_order
            .retain(|key| fixtures.contains(&key.fixture));
    }
}

fn cached(image: MediaImage) -> CachedImage {
    CachedImage {
        image,
        received_at: SystemTime::now(),
    }
}

fn touch<K: Eq + Clone>(order: &mut VecDeque<K>, key: &K) {
    if let Some(index) = order.iter().position(|candidate| candidate == key) {
        order.remove(index);
    }
    order.push_back(key.clone());
}

fn evict<K: Eq + std::hash::Hash + Clone, V>(
    values: &mut HashMap<K, V>,
    order: &mut VecDeque<K>,
    limit: usize,
) {
    while values.len() > limit {
        let Some(key) = order.pop_front() else {
            break;
        };
        values.remove(&key);
    }
}
