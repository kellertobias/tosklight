use super::*;

pub(super) fn authoritative_playback_controls(state: &AppState) -> serde_json::Value {
    let now = application_millis(state);
    let speed_groups = state.output.speed_group_snapshots(now);
    let snapshot = state.output.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .filter_map(|group| {
            state.output.group_master(&group.id).map(|master| {
                serde_json::json!({
                    "id":group.id,
                    "master":master,
                    "flash_level":state.output.group_master_flash(&group.id)
                })
            })
        })
        .collect::<Vec<_>>();
    let control = state.output.control_projection();
    let timing = state.installation.configuration();
    serde_json::json!({
        "speed_groups":speed_groups,
        "groups":groups,
        "grand_master":{
            "level":control.grand_master,
            "effective_level":if control.grand_master_flash {1.0} else {control.grand_master},
            "blackout":control.blackout,
            "flash_active":control.grand_master_flash,
            "dynamics_paused":state.output.playback_dynamics().paused
        },
        "programmer_fade_millis":timing.programmer_fade_millis,
        "cue_fade_millis":timing.sequence_master_fade_millis,
        "release_fade_millis":timing.release_fade_millis
    })
}

pub(super) use super::playback_policy::{
    PlaybackSoloRegion, read_playback_solo, update_playback_solo, validate_playback_solo_regions,
};
