use crate::EngineSnapshot;
use light_output::OutputRoute;
use light_playback::PlaybackEngine;
use light_programmer::GroupDefinition;
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};

/// One internally coherent engine generation.
///
/// A render retains this value for its complete lifetime, so fixture projection, Playback state,
/// Group resolution, and output routing cannot be mixed across show revisions while a new show is
/// installed concurrently.
pub(crate) struct RuntimeGeneration {
    snapshot: Arc<EngineSnapshot>,
    playback: Arc<RwLock<PlaybackEngine>>,
    groups: HashMap<String, GroupDefinition>,
    routes: Arc<[OutputRoute]>,
}

impl RuntimeGeneration {
    pub(crate) fn new(
        snapshot: EngineSnapshot,
        playback: PlaybackEngine,
        groups: HashMap<String, GroupDefinition>,
    ) -> Self {
        let routes = Arc::from(snapshot.routes.clone());
        Self {
            snapshot: Arc::new(snapshot),
            playback: Arc::new(RwLock::new(playback)),
            groups,
            routes,
        }
    }

    pub(crate) fn snapshot(&self) -> &EngineSnapshot {
        &self.snapshot
    }

    pub(crate) fn snapshot_arc(&self) -> Arc<EngineSnapshot> {
        Arc::clone(&self.snapshot)
    }

    pub(crate) fn playback(&self) -> &RwLock<PlaybackEngine> {
        &self.playback
    }

    pub(crate) fn playback_arc(&self) -> Arc<RwLock<PlaybackEngine>> {
        Arc::clone(&self.playback)
    }

    pub(crate) fn groups(&self) -> &HashMap<String, GroupDefinition> {
        &self.groups
    }

    pub(crate) fn routes(&self) -> Arc<[OutputRoute]> {
        Arc::clone(&self.routes)
    }
}
