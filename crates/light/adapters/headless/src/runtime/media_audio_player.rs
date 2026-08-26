//! How the Media pane reads an Internal Audio Player.

use super::media_fixture_heads::{master_head_attributes, media_layers};
use super::*;

/// One Internal Audio Player as the Media pane reads it.
///
/// A player advertises nothing over CITP, so its library, transport, and level come from the desk
/// rather than from a conversation with a server.
pub(super) fn audio_player_projection(
    state: &AppState,
    fixture: &light_fixture::PatchedFixture,
    name: &str,
) -> serde_json::Value {
    let player = state.internal_audio.player(fixture);
    serde_json::json!({
        "fixture_id": fixture.fixture_id,
        "fixture_number": fixture.fixture_number,
        "name": name,
        "kind": "audio_player",
        "endpoint": serde_json::Value::Null,
        "native_action": serde_json::Value::Null,
        "layers": media_layers(fixture),
        "master_attributes": master_head_attributes(fixture),
        "status": {
            "online": player.diagnostic.is_none(),
            "last_success": serde_json::Value::Null,
            "last_error": player.diagnostic,
        },
        "audio": {
            "folder": player.folder,
            "file": player.file,
            "volume_percent": player.volume_percent,
            "transport": player.transport,
            "repeat": player.repeat,
            "source": player.source,
            "library": state
                .internal_audio
                .library_entries(fixture)
                .into_iter()
                .map(|entry| {
                    serde_json::json!({
                        "folder": entry.folder,
                        "file": entry.file,
                        "name": entry.relative_path,
                    })
                })
                .collect::<Vec<_>>(),
        },
    })
}
