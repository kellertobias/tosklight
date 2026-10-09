//! Public-boundary coverage for the versioned command-line adapter.

use super::*;

include!("command_http_support.rs");
include!("command_http_revision_tests.rs");
include!("command_http_event_tests.rs");
include!("command_http_key_tests.rs");
include!("command_http_lifecycle_tests.rs");
include!("not_editable_screen_tests.rs");
include!("command_http_selection_tests.rs");
include!("command_http_values_tests.rs");
include!("command_http_programming_contract_tests.rs");
include!("command_http_preload_values_tests.rs");
include!("command_http_preload_playback_queue_tests.rs");
include!("command_http_preload_lifecycle_tests.rs");
include!("command_http_preset_recording_tests.rs");
include!("command_http_priority_preset_recall_tests.rs");
include!("command_http_group_management_tests.rs");
include!("command_http_group_recording_tests.rs");
include!("command_http_cue_recording_tests.rs");
include!("command_http_cue_transfer_tests.rs");
include!("command_http_cue_navigation_tests.rs");
include!("command_http_cue_deletion_tests.rs");
include!("command_http_cue_convergence_tests.rs");
include!("command_http_speed_group_tests.rs");
include!("live_action_http_tests.rs");

#[path = "command_http_family_values_tests.rs"]
mod family_values;
#[path = "command_http_optics_tests.rs"]
mod optics;
#[path = "command_http_semantic_aim_tests.rs"]
mod semantic_aim;
#[path = "tl560_direct_undo_tests.rs"]
mod tl560_direct_undo;

#[path = "position_intent_authoring_tests.rs"]
mod position_intent_authoring;

mod release_command_regressions;
