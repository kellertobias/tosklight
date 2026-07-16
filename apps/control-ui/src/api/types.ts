export interface DeskUser {
  id: string;
  name: string;
  enabled: boolean;
}

export interface ShowEntry {
  id: string;
  name: string;
  path: string;
  revision: number;
  updated_at: string;
}

export interface ShowRevision {
  show_id: string;
  revision: number;
  name: string;
  created_at: string;
}

export interface HelpCatalogEntry {
  id: string | null;
  title: string;
  kind: "folder" | "topic";
  children: HelpCatalogEntry[];
}
export interface HelpCatalog {
  topics: HelpCatalogEntry[];
  errors: string[];
  live: boolean;
}
export interface HelpTopic {
  id: string;
  title: string;
  markdown: string;
  live: boolean;
}
export interface FileRoot {
  id: string;
  label: string;
  icon: string;
  removable: boolean;
  writable: boolean;
}
export interface FileEntry {
  name: string;
  path: string;
  kind: "folder" | "file";
  size: number;
  modified_millis: number | null;
  created_millis: number | null;
  hidden: boolean;
  writable: boolean;
}
export interface FileDirectory {
  root_id: string;
  path: string;
  entries: FileEntry[];
}
export interface TextDocument {
  root_id: string;
  path: string;
  text: string;
  revision: string;
  read_only: boolean;
}

export interface MvrImportPreview {
  token: string;
  fixtures: Array<{
    uuid: string;
    name: string;
    gdtf_spec: string;
    gdtf_mode: string;
    universe: number | null;
    address: number | null;
    matched: boolean;
  }>;
  scenery: number;
  missing_profiles: string[];
  warnings: string[];
  address_conflicts: string[];
}
export interface MvrExportPreview {
  fixtures: number;
  scenery: number;
  embedded_profiles: number;
  missing_profiles: string[];
  omitted: string[];
  warnings: string[];
}
export interface MvrApplyResult {
  show: ShowEntry;
  imported_fixtures: number;
  unresolved_fixtures: number;
  imported_scenery: number;
  opened: boolean;
  warnings: string[];
}

export interface OutputHealth {
  frames_sent: number;
  packets_sent: number;
  send_errors: number;
  deadline_misses: number;
  maximum_lateness_micros: number;
  frame_hz: number;
  last_tick_micros: number;
  maximum_tick_micros: number;
  scheduler_utilization: number;
}

export interface ProgrammerState {
  session_id: string;
  user_id: string;
  selected: string[];
  command_line: string;
  connected: boolean;
  blind: boolean;
  preview: boolean;
  highlight: boolean;
  values: unknown[];
  group_values?: Record<
    string,
    Record<
      string,
      {
        value: unknown;
        changed_at: string;
        fade?: boolean;
        fade_millis?: number;
        delay_millis?: number;
      }
    >
  >;
  selection_expression?: {
    type: string;
    group_id?: string;
    source_revision?: number;
    rule?: { type: string; n?: number; offset?: number };
  };
}

export interface BootstrapSnapshot {
  api_version: string;
  users: DeskUser[];
  desks: ControlDesk[];
  active_show: ShowEntry | null;
  active_programmers: ProgrammerState[];
  frame_rate_hz: number;
  output_health: OutputHealth;
  active_timecode_source: string | null;
  active_timecode: string | null;
  active_show_error: string | null;
  hardware_connected: boolean;
}

export interface SessionResponse {
  session_id: string;
  token: string;
  user: DeskUser;
  desk: ControlDesk;
}
export interface DeskLockState {
  locked: boolean;
  message: string;
  wallpaper: string | null;
  unlock_mode: "button" | "pin";
}
export interface ControlDesk {
  id: string;
  name: string;
  osc_alias: string;
  columns: number;
  rows: number;
  buttons: number;
}
export interface ScreenConfiguration {
  id: string;
  name: string;
  layout: { desks: import("../types").DeskModel[]; activeDeskId: string };
  show_dock: boolean;
  show_playbacks: boolean;
  playback_count: number;
  playback_rows: number;
  first_playback_slot: number;
  page_mode: "follow_main" | "independent";
  show_page_controls: boolean;
  desired_open: boolean;
  display_id: string | null;
  bounds: { x: number; y: number; width: number; height: number } | null;
  fullscreen: boolean;
}
export interface ScreenSnapshot {
  screens: ScreenConfiguration[];
  active_pages: Record<string, number>;
}

export interface PatchedFixture {
  fixture_id: string;
  fixture_number?: number | null;
  name?: string;
  universe: number | null;
  address: number | null;
  layer_id?: string;
  direct_control?: {
    protocol: "citp";
    ip_address: string;
    port: number;
  } | null;
  definition: FixtureDefinition;
  logical_heads: Array<{ fixture_id: string; head_index: number }>;
  location?: { x: number; y: number; z: number };
  rotation?: { x: number; y: number; z: number };
  multipatch?: MultiPatchInstance[];
}

export interface MultiPatchInstance {
  id: string;
  name: string;
  universe: number | null;
  address: number | null;
  location: { x: number; y: number; z: number };
  rotation: { x: number; y: number; z: number };
}

export interface FixtureDefinition {
  schema_version: number;
  id: string;
  revision: number;
  manufacturer: string;
  device_type: string;
  name: string;
  model: string;
  mode: string;
  footprint: number;
  heads: Array<{
    index: number;
    name: string;
    shared: boolean;
    parameters: Array<{
      attribute: string;
      components: Array<{
        offset: number;
        byte_order: "msb_first" | "lsb_first";
      }>;
      default: number;
      virtual_dimmer: boolean;
      metadata?: {
        physical_min: number;
        physical_max: number;
        unit: string | null;
        invert: boolean;
        wrap: boolean;
        curve: string;
      };
      capabilities: Array<{
        name: string;
        dmx_from: number;
        dmx_to: number;
        preset_family?: string | null;
      }>;
    }>;
  }>;
  color_calibration: unknown | null;
  physical: {
    pan_range_degrees?: number | null;
    tilt_range_degrees?: number | null;
    width_millimetres?: number | null;
    height_millimetres?: number | null;
    depth_millimetres?: number | null;
    weight_kilograms?: number | null;
  };
  model_asset?: string | null;
  icon_asset?: string | null;
  hazardous: boolean;
  direct_control_protocols: Array<"citp">;
  signal_loss_policy: { type: string; duration_millis?: number };
  safe_values: Record<string, unknown>;
}

export interface MediaServerFixture {
  fixture_id: string;
  name: string;
  endpoint: { protocol: "citp"; ip_address: string; port: number };
  layers: Array<{ fixture_id: string; head_index: number }>;
  status: {
    online: boolean;
    last_success: string | null;
    last_error: string | null;
  };
}

export interface PatchSnapshot {
  revision: number;
  fixtures: PatchedFixture[];
  routes: Array<{
    protocol: "art_net" | "sacn";
    logical_universe: number;
    destination_universe: number;
    destination: string | null;
    enabled: boolean;
  }>;
}

export interface PatchLayer {
  id: string;
  name: string;
  order: number;
}

export interface Cue {
  number: number;
  name: string;
  fade_millis: number;
  delay_millis: number;
  trigger: { type: string; [key: string]: unknown };
  changes: Array<{
    fixture_id: string;
    attribute: string;
    value: AttributeValue | null;
    fade_millis?: number;
    delay_millis?: number;
  }>;
  group_changes?: Array<{
    group_id: string;
    attribute: string;
    value: AttributeValue | null;
    fade_millis?: number;
    delay_millis?: number;
  }>;
  phasers?: unknown[];
}

export type AttributeValue = { kind: "normalized"; value: number } | { kind: "discrete"; value: string } | { kind: "color_xyz"; value: { x: number; y: number; z: number } } | { kind: "raw_dmx"; value: number };

export interface VisualizationSnapshot {
  revision: number;
  generated_at: string;
  grand_master: number;
  blackout: boolean;
  preload?: boolean;
  values: Array<{
    fixture_id: string;
    attribute: string;
    value: AttributeValue;
  }>;
}

export interface CueList {
  id: string;
  name: string;
  cues: Cue[];
  mode: "sequence" | "chaser";
  priority: number;
  looped: boolean;
  intensity_priority_mode?: "htp" | "ltp";
  wrap_mode?: "off" | "tracking" | "reset";
  restart_mode?: "first_cue" | "continue_current_cue";
  force_cue_timing?: boolean;
  disable_cue_timing?: boolean;
  chaser_step_millis?: number;
  chaser_xfade_millis?: number;
  speed_group?: "A" | "B" | "C" | "D" | "E" | null;
  speed_multiplier?: number;
}

export interface PlaybackSnapshot {
  cue_lists: CueList[];
  pool: PlaybackDefinition[];
  pages: PlaybackPage[];
  active: Array<{
    playback_number?: number | null;
    cue_list_id: string;
    cue_index: number;
    previous_index?: number | null;
    paused: boolean;
    activated_at?: string;
    master: number;
    flash: boolean;
  }>;
  desk: ControlDesk;
  active_page: number;
}

export type PlaybackButtonAction = "on" | "off" | "toggle" | "go" | "go_minus" | "fast_forward" | "fast_rewind" | "flash" | "temp" | "swap" | "select" | "select_contents" | "learn" | "double" | "half" | "pause" | "blackout" | "pause_dynamics" | "none";
export interface PlaybackDefinition {
  number: number;
  name: string;
  target: { type: "cue_list"; cue_list_id: string } | { type: "group"; group_id: string } | { type: "speed_group"; group: string } | { type: "programmer_fade" } | { type: "cue_fade" } | { type: "grand_master" };
  buttons: [PlaybackButtonAction, PlaybackButtonAction, PlaybackButtonAction];
  fader: "master" | "temp" | "speed" | "x_fade" | "direct_bpm" | "centered_relative" | "learned_percentage";
  go_activates: boolean;
  auto_off: boolean;
  xfade_millis: number;
  color?: string;
  flash_release?: "release_all" | "release_intensity_only";
  protect_from_swap?: boolean;
}
export interface PlaybackPage {
  number: number;
  name: string;
  slots: Record<string, number>;
}

export interface DmxSnapshot {
  revision: number;
  universes: Array<{ universe: number; slots: number[] }>;
  overrides: Array<{ universe: number; address: number; value: number }>;
}

export interface VersionedObject<T = Record<string, unknown>> {
  kind: string;
  id: string;
  body: T;
  revision: number;
  updated_at: string;
}

export interface DeskConfiguration {
  frame_rate_hz: number;
  output_bind_ip: string;
  osc_bind: string | null;
  art_timecode_bind: string | null;
  midi_inputs: string[];
  rtp_midi_bind: string | null;
  timecode_sources: Array<{
    source_prefix: string;
    priority: number;
    fallback: boolean;
    loss_timeout_millis: number;
  }>;
  osc_timecode: { address: string; rate: string } | null;
  backup_retention: number;
  speed_groups_bpm: [number, number, number, number, number];
  programmer_fade_millis: number;
  sequence_master_fade_millis: number;
  preload_programmer_changes: boolean;
  preload_physical_playback_actions: boolean;
  preload_virtual_playback_actions: boolean;
  file_manager_roots: Array<{
    id: string;
    label: string;
    path: string;
    icon?: string;
  }>;
}

export interface StoredGroup {
  name?: string;
  fixtures: string[];
  master?: number;
  playback_fader?: number | null;
  programming?: Record<string, unknown>;
  derived_from?: {
    source_group_id: string;
    rule: { type: string; n?: number; offset?: number };
  } | null;
  frozen_from?: {
    source_group_id: string;
    source_revision: number;
    captured_at: string;
  } | null;
}

export interface StoredPreset {
  name: string;
  values: Record<string, Record<string, unknown>>;
  group_values?: Record<string, Record<string, unknown>>;
  family?: string;
  color?: string;
  icon?: string;
}

export interface ServerEvent {
  revision: number;
  kind: string;
  payload: Record<string, unknown>;
}

export type ConnectionStatus = "connecting" | "connected" | "offline" | "error";
