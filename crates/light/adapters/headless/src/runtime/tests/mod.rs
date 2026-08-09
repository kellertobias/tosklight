#![allow(clippy::items_after_test_module)]

use super::*;

fn v2_show_object_get(
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: Option<&str>,
) -> Request<Body> {
    let mut path = format!("/api/v2/objects/{kind}");
    if let Some(object_id) = object_id {
        path.push('/');
        path.push_str(object_id);
    }
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-tosk-show", show_id)
        .body(Body::empty())
        .unwrap()
}

#[path = "active_show_cue_preload_tests.rs"]
mod active_show_cue_preload_tests;
#[path = "active_show_playback_object_tests.rs"]
mod active_show_playback_object_tests;
#[path = "active_show_programmer_object_tests.rs"]
mod active_show_programmer_object_tests;
#[path = "active_show_route_tests.rs"]
mod active_show_route_tests;
#[path = "attribute_configuration_route_tests.rs"]
mod attribute_configuration_route_tests;
#[path = "command_http_tests.rs"]
mod command_http_tests;
#[path = "control_desk_configuration_v2_tests.rs"]
mod control_desk_configuration_v2_tests;
#[path = "control_mapping_tests.rs"]
mod control_mapping_tests;
#[path = "cue_thumbnail_route_tests.rs"]
mod cue_thumbnail_route_tests;
#[path = "discovery_route_tests.rs"]
mod discovery_route_tests;
#[path = "engine_selection_refresh_tests.rs"]
mod engine_selection_refresh_tests;
#[path = "event_transport_route_tests.rs"]
mod event_transport_route_tests;
#[path = "extension_control_tests.rs"]
mod extension_control_tests;
#[path = "output_runtime_tests.rs"]
mod output_runtime_tests;
#[path = "playback_topology_map_existing_route_tests.rs"]
mod playback_topology_map_existing_route_tests;
#[path = "playback_topology_page_route_tests.rs"]
mod playback_topology_page_route_tests;
#[path = "playback_topology_route_support.rs"]
mod playback_topology_route_support;
#[path = "playback_topology_route_tests.rs"]
mod playback_topology_route_tests;
#[path = "playback_v2_route_tests.rs"]
mod playback_v2_route_tests;
#[path = "playback_ws_action_tests.rs"]
mod playback_ws_action_tests;
#[path = "programmer_values_ws_action_tests.rs"]
mod programmer_values_ws_action_tests;
#[path = "programming_interaction_ws_action_tests.rs"]
mod programming_interaction_ws_action_tests;
#[path = "programming_update_route_tests.rs"]
mod programming_update_route_tests;
#[path = "runtime_v2_route_tests.rs"]
mod runtime_v2_route_tests;
#[path = "schedules_v2_route_tests.rs"]
mod schedules_v2_route_tests;
#[path = "screen_configuration_v2_tests.rs"]
mod screen_configuration_v2_tests;
#[path = "selective_import_route_tests.rs"]
mod selective_import_route_tests;
#[path = "show_library_v2_route_tests.rs"]
mod show_library_v2_route_tests;
#[path = "show_object_intents_v2_route_tests.rs"]
mod show_object_intents_v2_route_tests;
#[path = "show_object_v2_route_tests.rs"]
mod show_object_v2_route_tests;
#[path = "show_patch_route_tests.rs"]
mod show_patch_route_tests;
#[path = "speed_group_v2_tests.rs"]
mod speed_group_v2_tests;
#[path = "stage_layout_route_tests.rs"]
mod stage_layout_route_tests;
#[path = "usb_output_route_tests.rs"]
mod usb_output_route_tests;
#[path = "virtual_playback_zones_route_tests.rs"]
mod virtual_playback_zones_route_tests;
#[path = "visualizer_view_route_tests.rs"]
mod visualizer_view_route_tests;

include!("preload_tests.rs");
include!("command_input_tests.rs");
include!("matter_control_tests.rs");
include!("matter_feedback_tests.rs");
include!("show_migration_tests.rs");
include!("citp_support.rs");
include!("fixture_startup_tests.rs");
include!("runtime_support.rs");
include!("startup_tests.rs");
include!("highlight_schema_tests.rs");
include!("highlight_timing_support.rs");
include!("highlight_timing_tests.rs");
include!("osc_highlight_tests.rs");
include!("highlight_session_tests.rs");
include!("group_command_tests.rs");
include!("spread_recall_tests.rs");
include!("command_contract_tests.rs");
include!("cue_transfer_support.rs");
include!("cue_transfer_tests.rs");
include!("update_command_tests.rs");
include!("cue_selection_tests.rs");
include!("osc_key_tests.rs");
include!("dynamics_command_tests.rs");
include!("dynamics_osc_tests.rs");
include!("update_http_tests.rs");
include!("http_support.rs");
include!("fixture_profile_api_tests.rs");
include!("file_input_tests.rs");
include!("desk_http_tests.rs");
include!("websocket_programmer_tests.rs");
include!("security_event_tests.rs");
include!("preset_api_tests.rs");
include!("show_http_support.rs");
include!("show_rename_tests.rs");
include!("show_revision_tests.rs");
include!("show_overwrite_tests.rs");
include!("show_rest_tests.rs");
include!("active_show_document_cache_tests.rs");
include!("autosave_checkpoint_tests.rs");
include!("pool_presentation_configuration_tests.rs");
include!("operational_flow_support.rs");
include!("operational_flow_tests.rs");
include!("malformed_show_tests.rs");
include!("template_group_support.rs");
include!("template_group_tests.rs");
include!("spread_compatibility_tests.rs");
