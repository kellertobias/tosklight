//! Fixture deletion compares the current body and never bypasses a newer revision.
use super::show_patch_route_tests::install_patch_route_profile;
use super::show_sync_support::*;
use super::*;

#[tokio::test]
async fn fixture_delete_conflict_contains_current_body_and_retry_keeps_revision_guard() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Delete conflict").await;
    let association = Uuid::new_v4();
    let fixture_id = Uuid::new_v4();
    let fixture = serde_json::json!({
        "fixture_id":fixture_id,"fixture_number":1,"virtual_fixture_number":null,
        "name":"Spot 1","profile_id":profile_id,"profile_revision":1,"mode_id":mode_id,
        "split_patches":[{"split":1,"universe":1,"address":1}],"layer_id":"default",
        "direct_control":null,"location":{"x":0,"y":0,"z":0},
        "rotation":{"x":0.0,"y":0.0,"z":0.0},"multipatch":[],
        "move_in_black_enabled":true,"move_in_black_delay_millis":0,"highlight_overrides":[]
    });
    let create = sync_body(
        "create",
        association,
        &show_id,
        serde_json::json!([
            {"type":"create_patch_fixture","fixture":fixture}
        ]),
    );
    let created = json(post_sync(&app, &token, &create).await).await;
    let first_revision = created["applied"][0]["revision"].as_u64().unwrap();
    let rename = sync_body(
        "rename",
        association,
        &show_id,
        serde_json::json!([
            {"type":"update_patch_fixture","fixture_id":fixture_id,"fields":[{"path":"/name","base":"Spot 1","value":"Control rename"}]}
        ]),
    );
    let renamed = json(post_sync(&app, &token, &rename).await).await;
    let current_revision = renamed["applied"][0]["revision"].as_u64().unwrap();
    let deletion = |request: &str, revision| {
        sync_body(
            request,
            association,
            &show_id,
            serde_json::json!([
                {"type":"remove_patch_fixture","fixture_id":fixture_id,"base_fixture_revision":revision}
            ]),
        )
    };
    let stale = deletion("stale-delete", first_revision);
    let conflict = json(post_sync(&app, &token, &stale).await).await;
    assert_eq!(conflict["status"], "conflicted");
    assert_eq!(conflict["conflicts"][0]["theirs"]["name"], "Control rename");
    assert_eq!(
        conflict["conflicts"][0]["theirs"]["fixture_id"],
        fixture_id.to_string()
    );
    assert_eq!(
        conflict["conflicts"][0]["theirs_revision"],
        current_revision
    );
    // A new change after the operator's decision must conflict again, rather than force delete.
    let newer = sync_body(
        "rename-again",
        association,
        &show_id,
        serde_json::json!([
            {"type":"update_patch_fixture","fixture_id":fixture_id,"fields":[{"path":"/name","base":"Control rename","value":"Newer edit"}]}
        ]),
    );
    let newest = json(post_sync(&app, &token, &newer).await).await;
    let newest_revision = newest["applied"][0]["revision"].as_u64().unwrap();
    let raced =
        json(post_sync(&app, &token, &deletion("raced-delete", current_revision)).await).await;
    assert_eq!(raced["status"], "conflicted");
    assert_eq!(raced["conflicts"][0]["theirs"]["name"], "Newer edit");
    let retry = deletion("deliberate-delete", newest_revision);
    let accepted = json(post_sync(&app, &token, &retry).await).await;
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["applied"][0]["deleted"], true);
    let revision = active_revision(&state);
    let replay = json(post_sync(&app, &token, &retry).await).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(active_revision(&state), revision);
    let missing =
        json(post_sync(&app, &token, &deletion("already-missing", newest_revision)).await).await;
    assert_eq!(missing["status"], "accepted");
    assert_eq!(active_revision(&state), revision);
    let _ = std::fs::remove_dir_all(data_dir);
}
