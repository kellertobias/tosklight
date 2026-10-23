use super::{Embedding, HelperSource, PaneState};

pub(super) fn prepare(
    state: &mut PaneState,
    source: &mut HelperSource,
    embedding: &Embedding,
) -> Result<(), String> {
    // Publish the framed camera after picture settings resolve, including before any gesture.
    if let Some(camera) = state.take_camera_update() {
        source.send_camera(&camera);
    }
    state.resize_for(embedding)
}
