#![allow(clippy::items_after_test_module)]

use super::*;

#[path = "active_show_cue_preload_tests.rs"]
mod active_show_cue_preload_tests;
#[path = "active_show_playback_object_tests.rs"]
mod active_show_playback_object_tests;
#[path = "active_show_programmer_object_tests.rs"]
mod active_show_programmer_object_tests;
#[path = "active_show_route_tests.rs"]
mod active_show_route_tests;
#[path = "command_http_tests.rs"]
mod command_http_tests;
#[path = "control_mapping_tests.rs"]
mod control_mapping_tests;
#[path = "engine_selection_refresh_tests.rs"]
mod engine_selection_refresh_tests;
#[path = "event_transport_route_tests.rs"]
mod event_transport_route_tests;
#[path = "output_runtime_tests.rs"]
mod output_runtime_tests;
#[path = "playback_topology_route_support.rs"]
mod playback_topology_route_support;
#[path = "playback_topology_route_tests.rs"]
mod playback_topology_route_tests;
#[path = "playback_v2_route_tests.rs"]
mod playback_v2_route_tests;
#[path = "programming_interaction_adapter_tests.rs"]
mod programming_interaction_adapter_tests;
#[path = "programming_update_route_tests.rs"]
mod programming_update_route_tests;
#[path = "selective_import_route_tests.rs"]
mod selective_import_route_tests;
#[path = "show_patch_route_tests.rs"]
mod show_patch_route_tests;
#[path = "virtual_playback_zones_route_tests.rs"]
mod virtual_playback_zones_route_tests;

include!("preload_tests.rs");
include!("command_input_tests.rs");
include!("matter_control_tests.rs");
include!("matter_feedback_tests.rs");
include!("show_migration_tests.rs");
include!("citp_support.rs");
include!("fixture_startup_tests.rs");
include!("runtime_support.rs");
include!("startup_tests.rs");
include!("alignment_tests.rs");
include!("highlight_schema_tests.rs");
include!("highlight_timing_support.rs");
include!("highlight_timing_tests.rs");
include!("osc_highlight_tests.rs");
include!("highlight_session_tests.rs");
include!("group_command_tests.rs");
include!("command_contract_tests.rs");
include!("cue_transfer_support.rs");
include!("cue_transfer_tests.rs");
include!("update_command_tests.rs");
include!("cue_selection_tests.rs");
include!("osc_key_tests.rs");
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
include!("operational_flow_support.rs");
include!("operational_flow_tests.rs");
include!("malformed_show_tests.rs");
include!("template_group_support.rs");
include!("template_group_tests.rs");
