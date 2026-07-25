use crate::{ActionContext, ActionError, ActiveShowObjectChange, ActiveShowPorts};

/// Adapter hooks for authenticated portable Playback-topology mutations.
pub trait PlaybackTopologyPorts: ActiveShowPorts {
    fn authorize_playback_topology(&self, context: &ActionContext) -> Result<(), ActionError>;

    fn reconcile_playback_topology(&self, _changes: &[ActiveShowObjectChange]) {}
}
