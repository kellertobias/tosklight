//! Keeping each layer's session in step with what its address selects.
//!
//! This is where an address becomes media. The reducer says a layer points at `(folder, file)`;
//! the catalog says which asset that is; the loader makes it resident; and a session drives it.
//! Selecting a different address replaces the session, which is what gives each selection its own
//! transport rather than inheriting the last one's position.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use media_domain::catalog::CatalogSnapshot;
use media_domain::{
    AssetId, LayerState, MediaAddress, OutputId, PlayMode, SourceFailure, SourceStatus, Timestamp,
};
use media_library::LibraryStorage;

use crate::MediaLoader;
#[cfg(test)]
use crate::loader::ClipLoader;
use crate::session::PlaybackSession;

/// One layer's live selection.
struct Selected {
    address: MediaAddress,
    asset: AssetId,
    session: PlaybackSession,
    reset_trigger_id: u32,
}

/// What a layer needs from the caller this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSource {
    pub asset: AssetId,
    pub frame: Option<usize>,
    pub status: SourceStatus,
    /// The source's own dimensions, which the compositor scales from.
    pub size: (u32, u32),
    /// The clip this layer showed before its current selection, while that selection is not yet
    /// on screen and the switch hold has not run out.
    pub outgoing: Option<HeldClip>,
}

/// A previous clip a layer keeps playing while its new selection loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldClip {
    pub asset: AssetId,
    pub frame: Option<usize>,
    pub size: (u32, u32),
}

/// A clip that has been replaced but is still showing until its successor is.
struct Outgoing {
    selected: Selected,
    /// When the layer lets go regardless, so a slow load ends in an empty layer rather than a
    /// clip that seems to ignore the desk.
    until: Timestamp,
}

#[derive(Clone, Copy)]
struct FailedSelection {
    address: MediaAddress,
    asset: Option<AssetId>,
    reset_trigger_id: u32,
    failure: SourceFailure,
}

/// Sessions for one output's layers.
pub struct LayerSessions {
    output: OutputId,
    storage: LibraryStorage,
    selected: HashMap<usize, Selected>,
    pending: HashMap<usize, (MediaAddress, AssetId)>,
    sizes: HashMap<AssetId, (u32, u32)>,
    /// Addresses that have already failed, so a layer holding a bad selection does not retry on
    /// every single frame.
    failed: HashMap<usize, FailedSelection>,
    /// Replaced clips still on screen, per layer.
    outgoing: HashMap<usize, Outgoing>,
    /// How long a replaced clip may stay on screen. Zero drops it at once.
    switch_hold: Duration,
}

impl LayerSessions {
    pub fn new(output: OutputId, storage: LibraryStorage) -> Self {
        Self {
            output,
            storage,
            selected: HashMap::new(),
            pending: HashMap::new(),
            sizes: HashMap::new(),
            failed: HashMap::new(),
            outgoing: HashMap::new(),
            switch_hold: Duration::ZERO,
        }
    }

    pub const fn output(&self) -> OutputId {
        self.output
    }

    /// Sets how long a layer keeps its previous clip while a new selection loads. Applies from
    /// the next switch; a hold already running keeps the limit it started with.
    pub fn set_switch_hold(&mut self, hold: Duration) {
        self.switch_hold = hold;
    }

    /// Lets go of a layer's previous clip, because its new selection is on screen or can never
    /// be shown.
    pub fn end_hold(&mut self, layer_index: usize, loader: &mut impl MediaLoader) {
        if let Some(outgoing) = self.outgoing.remove(&layer_index) {
            self.let_go(outgoing.selected.asset, loader);
        }
    }

    /// Brings one layer's session in line with its state, loading media if the selection changed.
    ///
    /// Returns what to show, or `None` when the layer selects nothing or its source failed. A
    /// failure never clears the address the desk chose: the layer draws transparent while the
    /// status says why.
    pub fn reconcile(
        &mut self,
        layer_index: usize,
        layer: &LayerState,
        catalog: &CatalogSnapshot,
        loader: &mut impl MediaLoader,
        now: Timestamp,
    ) -> Option<LayerSource> {
        if layer.address.is_blank() {
            self.release(layer_index, loader);
            return None;
        }

        let disabled = catalog
            .folder(u16::from(layer.address.folder))
            .and_then(|folder| folder.item(layer.address.file))
            .is_some_and(|item| !item.enabled);
        if disabled {
            self.release(layer_index, loader);
            self.failed.remove(&layer_index);
            return None;
        }

        let catalog_asset = catalog.resolve(layer.address).map(|item| item.id);

        if self
            .outgoing
            .get(&layer_index)
            .is_some_and(|outgoing| now >= outgoing.until)
        {
            self.end_hold(layer_index, loader);
        }

        // A selection that has already failed is reported from memory rather than retried every
        // frame; changing the address clears it, and so does a reset.
        if let Some(failed) = self.failed.get(&layer_index).copied() {
            if failed.address == layer.address
                && failed.asset == catalog_asset
                && failed.reset_trigger_id == layer.reset_trigger_id
            {
                return Some(LayerSource {
                    asset: AssetId::default(),
                    frame: None,
                    status: SourceStatus::Failed {
                        failure: failed.failure,
                    },
                    size: (0, 0),
                    outgoing: None,
                });
            }
            self.failed.remove(&layer_index);
        }

        let retry = self
            .selected
            .get(&layer_index)
            .filter(|selected| {
                selected.reset_trigger_id != layer.reset_trigger_id
                    && (loader.failure(selected.asset).is_some() || layer.source_status.is_failed())
            })
            .map(|selected| selected.asset);
        if let Some(asset) = retry {
            loader.retry(asset);
        }
        let already = self.selected.get(&layer_index).is_some_and(|selected| {
            retry.is_none()
                && selected.address == layer.address
                && Some(selected.asset) == catalog_asset
                && !(selected.reset_trigger_id != layer.reset_trigger_id
                    && (loader.failure(selected.asset).is_some()
                        || layer.source_status.is_failed()))
        });
        if !already {
            let pending_matches = self
                .pending
                .get(&layer_index)
                .is_some_and(|(address, asset)| {
                    *address == layer.address && Some(*asset) == catalog_asset
                });
            if !pending_matches {
                self.make_way(layer_index, loader, now);
            }
            if let Err(failure) = self.select(layer_index, layer, catalog, loader, now) {
                self.release(layer_index, loader);
                self.failed.insert(
                    layer_index,
                    FailedSelection {
                        address: layer.address,
                        asset: catalog_asset,
                        reset_trigger_id: layer.reset_trigger_id,
                        failure,
                    },
                );
                return Some(LayerSource {
                    asset: AssetId::default(),
                    frame: None,
                    status: SourceStatus::Failed { failure },
                    size: (0, 0),
                    outgoing: None,
                });
            }
        }

        let outgoing = self.held(layer_index, layer, now);
        let Some(selected) = self.selected.get_mut(&layer_index) else {
            return Some(LayerSource {
                asset: catalog_asset.unwrap_or_default(),
                frame: None,
                status: SourceStatus::Loading,
                size: (0, 0),
                outgoing,
            });
        };
        selected.reset_trigger_id = layer.reset_trigger_id;
        selected.session.reconcile(layer, now);
        let delivery = selected
            .session
            .deliver(layer, media_domain::ResolvedTempo::None, now);
        let size = self.sizes.get(&selected.asset).copied().unwrap_or((0, 0));

        Some(LayerSource {
            asset: selected.asset,
            frame: delivery.frame,
            status: delivery.status,
            size,
            outgoing,
        })
    }

    /// The frame a layer's previous clip shows now, if it is still being held.
    ///
    /// The previous clip keeps the transport it had: it is only filling in, so a play mode or
    /// reset sent with the new selection belongs to the new selection.
    fn held(&mut self, layer_index: usize, layer: &LayerState, now: Timestamp) -> Option<HeldClip> {
        let outgoing = self.outgoing.get_mut(&layer_index)?;
        let asset = outgoing.selected.asset;
        let delivery =
            outgoing
                .selected
                .session
                .deliver(layer, media_domain::ResolvedTempo::None, now);
        Some(HeldClip {
            asset,
            frame: delivery.frame,
            size: self.sizes.get(&asset).copied().unwrap_or((0, 0)),
        })
    }

    /// Clears the way for a new selection on a layer.
    ///
    /// With a switch hold the clip on screen becomes the layer's outgoing clip instead of being
    /// dropped. A hold that is already running keeps the clip that was on screen when it began,
    /// so switching again before anything new appeared never shows an intermediate selection.
    fn make_way(&mut self, layer_index: usize, loader: &mut impl MediaLoader, now: Timestamp) {
        self.failed.remove(&layer_index);
        if let Some((_, asset)) = self.pending.remove(&layer_index) {
            loader.release_selection(asset);
        }
        let Some(previous) = self.selected.remove(&layer_index) else {
            return;
        };
        if self.switch_hold.is_zero() || self.outgoing.contains_key(&layer_index) {
            self.let_go(previous.asset, loader);
            return;
        }
        self.outgoing.insert(
            layer_index,
            Outgoing {
                selected: previous,
                until: now.plus(self.switch_hold),
            },
        );
    }

    /// Releases one selection's pin, forgetting its size once nothing on this output shows it.
    fn let_go(&mut self, asset: AssetId, loader: &mut impl MediaLoader) {
        loader.release_selection(asset);
        let still_shown = self
            .selected
            .values()
            .any(|selected| selected.asset == asset)
            || self
                .outgoing
                .values()
                .any(|outgoing| outgoing.selected.asset == asset);
        if !still_shown {
            self.sizes.remove(&asset);
        }
    }

    fn select(
        &mut self,
        layer_index: usize,
        layer: &LayerState,
        catalog: &CatalogSnapshot,
        loader: &mut impl MediaLoader,
        now: Timestamp,
    ) -> Result<(), SourceFailure> {
        let item = catalog
            .resolve(layer.address)
            .ok_or(SourceFailure::MissingFile)?;
        let path: PathBuf = self.storage.item_path(layer.address, &item.name);
        let asset = item.id;

        if let std::collections::hash_map::Entry::Vacant(entry) = self.pending.entry(layer_index) {
            loader.begin_selection(asset);
            entry.insert((layer.address, asset));
        }
        let Some(loaded) = loader.request_load(asset, &path)? else {
            return Ok(());
        };
        self.pending.remove(&layer_index);
        loader.finish_selection(asset);
        self.sizes.insert(asset, (loaded.width, loaded.height));
        self.selected.insert(
            layer_index,
            Selected {
                address: layer.address,
                asset,
                reset_trigger_id: layer.reset_trigger_id,
                session: PlaybackSession::new(
                    asset,
                    loaded.timing,
                    loaded.presentation_micros,
                    now,
                    if layer.play_mode.is_transport_running() {
                        layer.play_mode
                    } else {
                        PlayMode::Loop
                    },
                ),
            },
        );
        Ok(())
    }

    /// Lets go of a layer's selection, releasing its pin so the clip can be evicted once nothing
    /// else is showing it.
    pub fn release(&mut self, layer_index: usize, loader: &mut impl MediaLoader) {
        self.failed.remove(&layer_index);
        self.end_hold(layer_index, loader);
        if let Some((_, asset)) = self.pending.remove(&layer_index) {
            loader.release_selection(asset);
        }
        if let Some(previous) = self.selected.remove(&layer_index) {
            self.let_go(previous.asset, loader);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use media_codec::container::{ClipHeader, ClipWriter};
    use media_domain::catalog::{CatalogItem, CatalogLocation, ItemKind};

    use super::*;

    fn clip(frames: usize) -> Vec<u8> {
        let mut writer = ClipWriter::new(
            Cursor::new(Vec::new()),
            ClipHeader {
                width: 320,
                height: 180,
                frame_count: 0,
                frame_rate: (10, 1),
                intrinsic_bpm: None,
            },
        )
        .unwrap();
        for index in 0..frames {
            writer
                .write_frame(&[1u8; 24], index as u64 * 100_000)
                .unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    struct Bench {
        root: PathBuf,
        catalog: CatalogSnapshot,
        sessions: LayerSessions,
        loader: ClipLoader,
    }

    impl Bench {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join("media-layer-sessions").join(name);
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self {
                sessions: LayerSessions::new(OutputId::new(), LibraryStorage::new(root.clone())),
                root,
                catalog: CatalogSnapshot::default(),
                loader: ClipLoader::new(10_000_000),
            }
        }

        fn add(&mut self, folder: u16, file: u8, name: &str, frames: usize) -> AssetId {
            let id = AssetId::new();
            self.catalog
                .insert(
                    folder,
                    CatalogItem {
                        id,
                        file,
                        name: name.to_owned(),
                        kind: ItemKind::Video,
                        width: 320,
                        height: 180,
                        frames: Some(frames as u32),
                        intrinsic_bpm: None,
                        note: None,
                        enabled: true,
                    },
                )
                .unwrap();
            let storage = LibraryStorage::new(self.root.clone());
            let path = storage.item_path(CatalogLocation::new(folder, file), name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, clip(frames)).unwrap();
            id
        }

        fn reconcile(&mut self, layer: &LayerState, millis: u64) -> Option<LayerSource> {
            self.sessions.reconcile(
                0,
                layer,
                &self.catalog,
                &mut self.loader,
                Timestamp::from_millis(millis),
            )
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn pointing_at(folder: u8, file: u8) -> LayerState {
        LayerState {
            address: MediaAddress::new(folder, file),
            ..Default::default()
        }
    }

    #[test]
    fn an_address_becomes_playing_media() {
        let mut bench = Bench::new("selects");
        let asset = bench.add(1, 4, "Clip", 10);

        let source = bench.reconcile(&pointing_at(1, 4), 0).unwrap();
        assert_eq!(source.asset, asset);
        assert_eq!(source.status, SourceStatus::Ready);
        assert_eq!(source.frame, Some(0));
        assert_eq!(source.size, (320, 180));

        let later = bench.reconcile(&pointing_at(1, 4), 500).unwrap();
        assert_eq!(later.frame, Some(5), "and it advances");
    }

    #[test]
    fn a_blank_address_selects_nothing() {
        let mut bench = Bench::new("blank");
        bench.add(1, 4, "Clip", 10);
        assert!(bench.reconcile(&LayerState::default(), 0).is_none());
        assert!(bench.reconcile(&pointing_at(1, 0), 0).is_none());
        assert!(bench.reconcile(&pointing_at(1, 255), 0).is_none());
    }

    #[test]
    fn disabling_an_active_clip_releases_it_and_draws_transparent_until_reenabled() {
        let mut bench = Bench::new("disabled-active");
        let layer = pointing_at(1, 4);
        let asset = bench.add(1, 4, "Clip", 10);

        assert!(bench.reconcile(&layer, 0).is_some());
        assert!(bench.loader.cache().is_pinned(asset));
        bench.catalog.set_item_enabled(asset, false).unwrap();

        assert!(bench.reconcile(&layer, 500).is_none());
        assert!(!bench.loader.cache().is_pinned(asset));

        bench.catalog.set_item_enabled(asset, true).unwrap();
        let source = bench.reconcile(&layer, 700).unwrap();
        assert_eq!(source.asset, asset);
        assert_eq!(source.frame, Some(0), "re-enable starts a fresh session");
    }

    #[test]
    fn deleting_an_active_clip_releases_it_without_a_stale_frame() {
        let mut bench = Bench::new("delete-active");
        let layer = pointing_at(1, 4);
        let asset = bench.add(1, 4, "Clip", 10);
        assert!(bench.reconcile(&layer, 0).is_some());

        assert!(bench.catalog.remove_item(asset));
        let source = bench.reconcile(&layer, 500).unwrap();
        assert_eq!(source.frame, None);
        assert_eq!(
            source.status,
            SourceStatus::Failed {
                failure: SourceFailure::MissingFile,
            }
        );
        assert!(!bench.loader.cache().is_pinned(asset));
    }

    #[test]
    fn an_address_with_no_media_fails_without_losing_the_selection() {
        let mut bench = Bench::new("missing");
        let source = bench.reconcile(&pointing_at(9, 9), 0).unwrap();
        assert_eq!(
            source.status,
            SourceStatus::Failed {
                failure: SourceFailure::MissingFile
            }
        );
        assert_eq!(source.frame, None, "the layer draws transparent");
    }

    #[test]
    fn a_failed_selection_is_not_retried_on_every_frame() {
        let mut bench = Bench::new("no-retry-storm");
        for millis in [0, 16, 32] {
            let source = bench.reconcile(&pointing_at(9, 9), millis).unwrap();
            assert!(source.status.is_failed(), "at {millis}ms");
        }
    }

    #[test]
    fn a_failed_selection_retries_once_per_new_reset() {
        let mut bench = Bench::new("reset-failure-edge");
        let mut layer = pointing_at(1, 1);
        layer.reset_trigger_id = 7;
        bench.add(1, 1, "Repaired", 10);
        let path = LibraryStorage::new(bench.root.clone()).item_path(layer.address, "Repaired");
        std::fs::remove_file(&path).unwrap();
        assert!(bench.reconcile(&layer, 0).unwrap().status.is_failed());
        std::fs::write(path, clip(10)).unwrap();
        assert!(
            bench.reconcile(&layer, 16).unwrap().status.is_failed(),
            "an old reset must not trigger a disk retry every frame"
        );
        layer.reset_trigger_id += 1;
        assert_eq!(
            bench.reconcile(&layer, 32).unwrap().status,
            SourceStatus::Ready
        );
    }

    #[test]
    fn deselection_clears_failure_and_allows_a_repaired_address() {
        let mut bench = Bench::new("reselect-failure");
        let layer = pointing_at(1, 1);
        assert!(bench.reconcile(&layer, 0).unwrap().status.is_failed());
        bench.reconcile(&LayerState::default(), 16);
        bench.add(1, 1, "Repaired", 10);
        assert_eq!(
            bench.reconcile(&layer, 32).unwrap().status,
            SourceStatus::Ready
        );
    }

    #[test]
    fn one_layers_failed_selection_does_not_poison_another_layer() {
        let mut bench = Bench::new("layer-failure-isolation");
        let layer = pointing_at(1, 1);
        assert!(bench.reconcile(&layer, 0).unwrap().status.is_failed());
        bench.add(1, 1, "Repaired", 10);
        let source = bench
            .sessions
            .reconcile(
                1,
                &layer,
                &bench.catalog,
                &mut bench.loader,
                Timestamp::ZERO,
            )
            .unwrap();
        assert_eq!(source.status, SourceStatus::Ready);
    }

    #[test]
    fn importing_a_previously_missing_address_recovers_without_a_reset() {
        let mut bench = Bench::new("import-recovers");
        let layer = pointing_at(1, 1);
        assert!(bench.reconcile(&layer, 0).unwrap().status.is_failed());
        let asset = bench.add(1, 1, "Imported", 10);
        let source = bench.reconcile(&layer, 16).unwrap();
        assert_eq!(source.status, SourceStatus::Ready);
        assert_eq!(source.asset, asset);
    }

    #[test]
    fn replacing_the_catalog_asset_reloads_the_active_address() {
        let mut bench = Bench::new("replace-active");
        let layer = pointing_at(1, 1);
        let first = bench.add(1, 1, "Clip", 10);
        bench.reconcile(&layer, 0);
        let mut replacement = bench.catalog.resolve(layer.address).unwrap().clone();
        replacement.id = AssetId::new();
        let next_asset = replacement.id;
        bench.catalog = CatalogSnapshot::default();
        bench.catalog.insert(1, replacement).unwrap();
        let source = bench.reconcile(&layer, 500).unwrap();
        assert_eq!(source.asset, next_asset);
        assert_eq!(source.frame, Some(0));
        assert!(!bench.loader.cache().is_pinned(first));
        assert!(bench.loader.cache().is_pinned(next_asset));
    }

    #[test]
    fn changing_the_address_starts_a_fresh_transport() {
        let mut bench = Bench::new("reselect");
        bench.add(1, 1, "First", 10);
        bench.add(1, 2, "Second", 10);

        bench.reconcile(&pointing_at(1, 1), 0);
        let advanced = bench.reconcile(&pointing_at(1, 1), 500).unwrap();
        assert_eq!(advanced.frame, Some(5));

        let switched = bench.reconcile(&pointing_at(1, 2), 500).unwrap();
        assert_eq!(
            switched.frame,
            Some(0),
            "the new selection starts at its own beginning, not the last one's position"
        );
    }

    #[test]
    fn a_switch_hold_keeps_the_previous_clip_playing_until_it_runs_out() {
        let mut bench = Bench::new("switch-hold-expires");
        let first = bench.add(1, 1, "First", 10);
        let second = bench.add(1, 2, "Second", 10);
        bench.sessions.set_switch_hold(Duration::from_millis(300));

        bench.reconcile(&pointing_at(1, 1), 0);
        let switched = bench.reconcile(&pointing_at(1, 2), 500).unwrap();
        assert_eq!(switched.asset, second);
        assert_eq!(switched.frame, Some(0));
        assert_eq!(
            switched.outgoing,
            Some(HeldClip {
                asset: first,
                frame: Some(5),
                size: (320, 180),
            })
        );
        assert!(
            bench.loader.cache().is_pinned(first),
            "a held clip cannot be evicted"
        );

        let still = bench.reconcile(&pointing_at(1, 2), 700).unwrap();
        assert_eq!(
            still.outgoing.and_then(|held| held.frame),
            Some(7),
            "the previous clip keeps playing rather than freezing"
        );

        let expired = bench.reconcile(&pointing_at(1, 2), 800).unwrap();
        assert_eq!(expired.outgoing, None, "the hold runs out");
        assert!(!bench.loader.cache().is_pinned(first));
    }

    #[test]
    fn a_switch_hold_ends_when_the_caller_shows_the_new_clip() {
        let mut bench = Bench::new("switch-hold-ends");
        let first = bench.add(1, 1, "First", 10);
        bench.add(1, 2, "Second", 10);
        bench.sessions.set_switch_hold(Duration::from_millis(300));

        bench.reconcile(&pointing_at(1, 1), 0);
        assert!(
            bench
                .reconcile(&pointing_at(1, 2), 100)
                .unwrap()
                .outgoing
                .is_some()
        );
        bench.sessions.end_hold(0, &mut bench.loader);
        assert_eq!(
            bench.reconcile(&pointing_at(1, 2), 150).unwrap().outgoing,
            None
        );
        assert!(!bench.loader.cache().is_pinned(first));
    }

    #[test]
    fn blanking_a_layer_drops_a_held_clip_at_once() {
        let mut bench = Bench::new("switch-hold-blank");
        let first = bench.add(1, 1, "First", 10);
        bench.add(1, 2, "Second", 10);
        bench.sessions.set_switch_hold(Duration::from_millis(300));

        bench.reconcile(&pointing_at(1, 1), 0);
        bench.reconcile(&pointing_at(1, 2), 100);
        assert!(bench.reconcile(&LayerState::default(), 120).is_none());
        assert!(!bench.loader.cache().is_pinned(first));
        assert_eq!(
            bench.reconcile(&pointing_at(1, 2), 140).unwrap().outgoing,
            None
        );
    }

    #[test]
    fn switching_again_during_a_hold_keeps_the_clip_that_was_on_screen() {
        let mut bench = Bench::new("switch-hold-again");
        let first = bench.add(1, 1, "First", 10);
        let second = bench.add(1, 2, "Second", 10);
        bench.add(1, 3, "Third", 10);
        bench.sessions.set_switch_hold(Duration::from_millis(300));

        bench.reconcile(&pointing_at(1, 1), 0);
        bench.reconcile(&pointing_at(1, 2), 100);
        let third = bench.reconcile(&pointing_at(1, 3), 150).unwrap();
        assert_eq!(third.outgoing.map(|held| held.asset), Some(first));
        assert!(
            !bench.loader.cache().is_pinned(second),
            "the never-shown selection is released"
        );
    }

    #[test]
    fn without_a_switch_hold_the_previous_clip_is_released_at_once() {
        let mut bench = Bench::new("switch-hold-off");
        let first = bench.add(1, 1, "First", 10);
        bench.add(1, 2, "Second", 10);

        bench.reconcile(&pointing_at(1, 1), 0);
        assert_eq!(
            bench.reconcile(&pointing_at(1, 2), 100).unwrap().outgoing,
            None
        );
        assert!(!bench.loader.cache().is_pinned(first));
    }

    #[test]
    fn a_clip_on_screen_is_pinned_and_released_when_deselected() {
        let mut bench = Bench::new("pinning");
        let asset = bench.add(1, 4, "Clip", 10);

        bench.reconcile(&pointing_at(1, 4), 0);
        assert!(
            bench.loader.cache().is_pinned(asset),
            "what is showing cannot be evicted"
        );

        bench.reconcile(&LayerState::default(), 10);
        assert!(
            !bench.loader.cache().is_pinned(asset),
            "and is released when it stops showing"
        );
    }

    #[test]
    fn re_selecting_the_same_address_does_not_restart_playback() {
        let mut bench = Bench::new("stable");
        bench.add(1, 4, "Clip", 10);
        bench.reconcile(&pointing_at(1, 4), 0);

        let mut layer = pointing_at(1, 4);
        layer.dimmer = 0.5;
        let source = bench.reconcile(&layer, 700).unwrap();
        assert_eq!(
            source.frame,
            Some(7),
            "dimming does not reload or restart the clip"
        );
    }
}
