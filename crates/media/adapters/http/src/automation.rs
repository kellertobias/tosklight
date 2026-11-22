//! The runtime bridge used by autonomous-playback routes and telemetry.

use crate::wire::AutomationRuntimeView;
use arc_swap::ArcSwap;
use media_domain::OutputId;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AutomationAccess {
    /// Temporary output previews and bounded closed-session sequencing.
    pub previews: Arc<Mutex<crate::playlist_preview::PreviewSessions>>,
    pub snapshot: Arc<ArcSwap<AutomationRuntimeView>>,
    /// Explicit live actions, consumed once by the process runner; never replayed on reconnect.
    pub apply_now: Arc<Mutex<HashSet<OutputId>>>,
}

impl Default for AutomationAccess {
    fn default() -> Self {
        Self {
            previews: Arc::new(Mutex::new(Default::default())),
            snapshot: Arc::new(ArcSwap::from_pointee(AutomationRuntimeView {
                time_zone: "Computer local time".into(),
                ..Default::default()
            })),
            apply_now: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}
