#![allow(clippy::items_after_test_module)]

use super::*;

#[path = "command_http_tests.rs"]
mod command_http_tests;
#[path = "event_transport_route_tests.rs"]
mod event_transport_route_tests;

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
