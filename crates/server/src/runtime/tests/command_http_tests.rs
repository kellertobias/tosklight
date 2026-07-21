//! Public-boundary coverage for the versioned command-line adapter.

use super::*;

include!("command_http_support.rs");
include!("command_http_revision_tests.rs");
include!("command_http_event_tests.rs");
include!("command_http_key_tests.rs");
include!("command_http_lifecycle_tests.rs");
include!("command_http_selection_tests.rs");
include!("command_http_values_tests.rs");
include!("command_http_preload_values_tests.rs");
include!("command_http_preload_playback_queue_tests.rs");
include!("command_http_preload_lifecycle_tests.rs");
include!("command_http_preset_recording_tests.rs");
include!("command_http_priority_preset_recall_tests.rs");
include!("command_http_group_recording_tests.rs");
include!("command_http_cue_recording_tests.rs");
include!("command_http_cue_transfer_tests.rs");
include!("command_http_cue_navigation_tests.rs");
include!("command_http_cue_convergence_tests.rs");
include!("command_http_speed_group_tests.rs");
