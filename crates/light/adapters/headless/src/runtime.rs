#![forbid(unsafe_code)]

mod bootstrap;
#[path = "command_http.rs"]
mod command_http;
#[path = "default_show.rs"]
mod default_show;
mod desk_collapse_migration;
mod discovery_http;
mod dynamics_adapter;
mod dynamics_http;
mod event_transport;
mod extensions_runtime;
#[path = "file_manager.rs"]
mod file_manager;
#[path = "file_manager_support.rs"]
mod file_manager_support;
mod fixture_freeze;
#[path = "help.rs"]
mod help;
mod http_router;
#[path = "matter.rs"]
mod matter;
mod media_identity;
mod output_scheduler;
mod playback_service;
mod playback_telemetry;
mod playback_v2;
mod programming_update_adapter;
mod programming_update_http;
mod programming_update_http_error;
mod programming_update_wire;
mod programming_update_wire_output;
mod runtime_wire;
mod startup_options;
mod startup_state;
mod usb_output;
mod visualization_frame;
mod visualization_transport;

use crate::tolerant_json::TolerantJson;
use crate::update;
use action_timing::{
    ActionTimingProjection, ActionTimingReceipt, ActionTimingResource, OscActionFeedback,
};
use axum::{
    Json, Router,
    extract::Request,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use bytes::Bytes;
use light_application::{
    ActiveShowService, EventBus, OutputRuntimeService, PlaybackAction, PlaybackAddress,
    PlaybackExecution, PlaybackService, PlaybackTopologyService, ProgrammingService,
    SelectiveShowImportService, ShowPatchService, SpeedGroupService,
};
use light_control::speed::{
    SoundObservation, SoundToLightConfig, SpeedGroupController, SpeedSnapshot,
};
use light_control::{
    ControlAction, ControlEvent, ControlInput, ExternalTimecodeLossPolicy, FrameRate, OscArgument,
    SmpteTimecode, TimecodeRouter, TimecodeRouterConfig, TimecodeSourceSelection, UdpControlInput,
    UdpInputProtocol, encode_osc_message,
};
use light_core::{ApplicationClock, ManualClock, SessionId};
use light_engine::{
    Engine, EngineError, EnginePlaybackCommand, EnginePlaybackOutcome, EngineSnapshot,
    PoolPlaybackAction, PreparedEngineSnapshot, RenderOptions,
};
use light_media::{CitpClient, LibraryId, MediaCache, PreviewKey, ThumbnailKey, discover_servers};
use light_output::{NetworkOutput, OutputHealth};
use light_programmer::{
    HighlightAction, HighlightFixture, HighlightMode, HighlightRegistry, HighlightSelectionWrite,
    HighlightState, ProgrammerRegistry, is_duplicate_osc_action,
};
#[cfg(test)]
use light_show::ShowStore;
use light_show::{
    AtomicObjectDelete, ControlDesk, DeskStore, DeskUser, PersistedSession, RevisionCopySource,
    ScreenConfiguration, ShowEntry, ShowRevision, initialise_show, validate_show_file,
};
use parking_lot::{Mutex, RwLock};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

mod action_timing;
mod active_show_adapter;
mod active_show_objects;
mod api_error;
mod api_types;
mod attribute_configuration;
mod auth_backup;
mod boundaries;
mod capabilities;
mod capability_resources;
mod command_parse;
mod command_playback_addresses;
mod command_presets;
mod command_timing;
mod configuration;
mod control_desk_configuration_v2;
mod control_inputs;
mod cue_speed_commands;
mod cue_thumbnails_http;
mod desk_management_v2;
mod engine_selection_refresh;
mod event_ws;
mod fixture_api;
mod fixture_api_replay;
mod generated_presets;
mod group_selection;
mod highlight_api;
mod highlight_compatibility;
mod highlight_service_adapter;
mod indexed_presets;
mod internal_audio;
mod internal_audio_http;
mod lifecycle;
mod live_action_http;
pub(crate) mod macros_v2;
mod media_api;
mod media_fixture_heads;
mod mvr_apply;
mod mvr_apply_store;
mod mvr_import;
mod object_api;
mod object_normalization;
mod operator_api;
mod osc_cue_record_suppression;
mod osc_dynamics;
mod osc_feedback;
mod osc_feedback_broadcast;
mod osc_feedback_playbacks;
mod osc_feedback_programmer;
mod osc_highlight;
mod osc_playback;
mod output_api;
mod output_runtime_service;
mod output_runtime_v2;
mod persistence_events;
mod playback_api;
mod playback_dispatch;
mod playback_exclusion_normalization;
mod playback_inputs;
mod playback_layout;
mod playback_layout_mutations;
mod playback_persistence;
mod playback_speed_groups;
mod playback_target_actions;
mod playback_topology_adapter;
mod playback_topology_http;
mod playback_topology_wire;
mod preload;
mod programmer_commands;
mod programmer_dynamic_commands;
mod programmer_fixture_commands;
mod programmer_group_commands;
mod programmer_selection_values;
mod programming_interaction;
mod request_context;
mod schedules_v2;
mod screen_configuration_v2;
mod screens_playback;
mod selective_import_adapter;
mod selective_import_http;
mod selective_import_wire;
mod sessions;
mod set_commands;
mod show_command_dispatch;
mod show_command_presets;
mod show_command_record;
mod show_command_update;
mod show_commands;
mod show_compile;
mod show_compile_migrations;
mod show_library;
mod show_library_mutations;
mod show_library_v2;
mod show_mutation_backup;
mod show_object_intents_v2;
mod show_objects_v2;
mod show_open;
mod show_patch_adapter;
mod show_patch_http;
pub(crate) mod show_patch_wire;
mod speed_group_bindings;
mod speed_group_service;
mod speed_group_v2;
mod speed_groups;
mod stage_layout_http;
mod state;
mod store_api;
mod store_preload_targets;
mod test_bench;
#[cfg(feature = "native-audio-output")]
mod timecode_audio_output;
#[cfg(not(feature = "native-audio-output"))]
#[path = "runtime/timecode_audio_output_disabled.rs"]
mod timecode_audio_output;
mod timecode_v2;
mod timecode_v2_clips;
mod update_api;

pub(crate) use capabilities::active_show::repository::ActiveShowRepository;
mod update_plans;
mod virtual_playback_zones_http;
mod visualizer_view_http;
mod ws_dispatch;
mod ws_output_handlers;
mod ws_playback_handlers;
mod ws_preload_handlers;
mod ws_preset_handlers;
mod ws_programmer_handlers;
mod ws_selection_handlers;

use active_show_adapter::*;
use active_show_objects::*;
use api_error::*;
use api_types::*;
use auth_backup::*;
use boundaries::*;
use capability_resources::*;
use command_parse::*;
use command_playback_addresses::*;
use command_presets::*;
use command_timing::*;
use configuration::*;
use control_inputs::*;
use cue_speed_commands::*;
use engine_selection_refresh::*;
use event_ws::*;
use generated_presets::*;
use group_selection::*;
use highlight_api::*;
use lifecycle::*;
use media_api::*;
use media_fixture_heads::*;
use mvr_apply::*;
use mvr_apply_store::*;
use mvr_import::*;
use object_api::*;
use operator_api::*;
use osc_dynamics::*;
use osc_feedback::*;
use osc_feedback_broadcast::*;
use osc_feedback_playbacks::*;
use osc_feedback_programmer::*;
use osc_highlight::*;
use osc_playback::*;
use output_api::*;
use persistence_events::*;
use playback_api::*;
use playback_dispatch::*;
use playback_exclusion_normalization::normalize_restored_virtual_playback_exclusions;
use playback_inputs::*;
use playback_layout::*;
use playback_speed_groups::*;
use playback_target_actions::*;
use playback_topology_adapter::*;
use preload::*;
use programmer_commands::*;
use programmer_fixture_commands::*;
use programmer_group_commands::*;
use programmer_selection_values::*;
use programming_interaction::*;
use programming_update_adapter::*;
use request_context::*;
use screens_playback::*;
use selective_import_adapter::*;
use sessions::*;
use set_commands::*;
use show_command_dispatch::*;
use show_command_presets::*;
use show_command_record::*;
use show_command_update::*;
use show_commands::*;
use show_compile::*;
use show_compile_migrations::*;
use show_library::*;
use show_library_mutations::*;
use show_mutation_backup::*;
use show_open::*;
use show_patch_adapter::*;
use speed_groups::*;
use state::*;
use store_preload_targets::*;
use test_bench::*;
use update_api::*;
use update_plans::*;
use ws_dispatch::*;
use ws_output_handlers::*;
use ws_playback_handlers::*;
use ws_preload_handlers::*;
use ws_preset_handlers::*;
use ws_programmer_handlers::*;
use ws_selection_handlers::*;

// @tour orientation:30 Compose the server runtime
// Startup binds configuration, persistence, application services, input adapters, output, and
// shutdown at one process edge. Domain crates remain unaware of Axum and operating-system wiring.
pub async fn run() -> anyhow::Result<()> {
    run_server().await
}

#[cfg(test)]
mod tests;
