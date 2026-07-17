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
  revision_copy?: RevisionCopySource;
}

export interface RevisionCopySource {
  show_id: string;
  show_name: string;
  revision: number;
  revision_name: string;
  copied_at: string;
}

export type CueUpdateMode = "existing_only" | "existing_in_current_cue" | "add_to_current_cue" | "add_new";
export type ExistingContentMode = "update_existing" | "add_new";
export type UpdateTargetFilter = "eligible_for_update_existing" | "show_all_active";

export type UpdateTargetFamily =
  | { type: "cue" }
  | { type: "preset" }
  | { type: "group" }
  | { type: "other"; kind: string };

export type UpdateMode =
  | { target_type: "cue"; mode: CueUpdateMode }
  | { target_type: "existing_content"; mode: ExistingContentMode };

/** Transport target resolved authoritatively by the server before preview or apply. */
export interface UpdateTargetRequest {
  family: UpdateTargetFamily;
  object_id: string;
  playback_number?: number;
  cue_id?: string;
  cue_number?: number;
  validate_active_context?: boolean;
}

export interface UpdateSettings {
  cue_mode: CueUpdateMode;
  preset_mode: ExistingContentMode;
  group_mode: ExistingContentMode;
  other_target_modes: Record<string, ExistingContentMode>;
  show_update_modal_on_touch: boolean;
}

export interface UpdateCueIdentity {
  id: string;
  number: number;
}

export interface UpdateTargetIdentity {
  family: UpdateTargetFamily;
  object_id: string;
  name: string;
  playback_number?: number;
  cue?: UpdateCueIdentity;
}

export type UpdateAddress =
  | { type: "fixture_attribute"; fixture_id: string; attribute: string }
  | { type: "group_attribute"; group_id: string; attribute: string }
  | { type: "group_membership"; fixture_id: string };

export interface UpdateCueSource {
  cue_id: string;
  cue_number: number;
  cue_index: number;
}

export type UpdateIgnoreReason = "new_address" | "not_in_current_cue" | "not_in_active_tracked_state" | "new_group_member";

export type UpdateItemOutcome =
  | { outcome: "change_at_source"; source: UpdateCueSource }
  | { outcome: "change_in_current_cue"; cue: UpdateCueSource }
  | { outcome: "add_to_current_cue"; cue: UpdateCueSource }
  | { outcome: "add_new_to_current_cue"; cue: UpdateCueSource }
  | { outcome: "update_existing" }
  | { outcome: "add_new" }
  | { outcome: "unchanged"; source?: UpdateCueSource }
  | { outcome: "ignored"; reason: UpdateIgnoreReason };

export interface UpdatePreviewItem {
  address: UpdateAddress;
  outcome: UpdateItemOutcome;
}

export interface UpdatePreview {
  /** Exact target object revision used to calculate this preview. */
  revision: number;
  /** Fingerprint of the exact normal-programmer content shown in this preview. */
  programmer_revision: string;
  target: UpdateTargetIdentity;
  mode: UpdateMode;
  items: UpdatePreviewItem[];
}

export interface UpdateMenuEntry {
  /** Exact target object revision shared by the menu previews. */
  revision: number;
  target: UpdateTargetIdentity;
  active_or_referenced: boolean;
  existing_preview: UpdatePreview;
  add_new_preview?: UpdatePreview;
}

export interface UpdateResult {
  target: UpdateTargetIdentity;
  revision_before: number;
  revision_after: number;
  eligible_count: number;
  changed_count: number;
  added_count: number;
  ignored_count: number;
  changed_cues: UpdateCueSource[];
  programmer_values_retained: boolean;
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
  capabilities?: FileSystemCapabilities;
}
export interface FileSystemCapabilities {
  created_time: boolean;
  hidden_attributes: boolean;
  native_notes: boolean;
  trash: boolean;
  range_streaming: boolean;
  thumbnails: boolean;
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
  mime?: string;
  note_supported?: boolean;
  trash_supported?: boolean;
}
export interface FileDirectory {
  root_id: string;
  path: string;
  entries: FileEntry[];
}
export interface FileMetadata extends FileEntry {
  root_id: string;
  capabilities: FileSystemCapabilities;
}
export interface FileNativeNote {
  root_id: string;
  path: string;
  supported: boolean;
  note: string | null;
}
export type FileConflictChoice = "replace" | "keep_both" | "skip";
export interface FileOperationResult {
  paths: string[];
  complete: boolean;
  items: Array<{
    source_root_id: string;
    source: string;
    destination_root_id: string | null;
    destination: string | null;
    status: "completed" | "skipped" | "failed";
    error: string | null;
  }>;
}
export type FileInputAction = "rename" | "copy" | "move" | "delete";
export interface FileInputContext {
  instance_id: string;
  action: FileInputAction;
  session_id: string;
  desk_id: string;
  expires_in_millis: number;
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
  preload_capture_programmer?: boolean;
  preview: boolean;
  highlight: boolean;
  values: unknown[];
  preload_pending?: Array<{
    fixture_id: string;
    attribute: string;
    value: unknown;
    changed_at: string;
    fade_millis?: number;
    delay_millis?: number;
  }>;
  preload_active?: unknown[];
  preload_group_pending?: Record<
    string,
    Record<string, { value: unknown; changed_at: string; fade_millis?: number; delay_millis?: number }>
  >;
  preload_group_active?: Record<string, Record<string, unknown>>;
  preload_playback_pending?: Array<{
    playback_number: number;
    action: "toggle" | "go" | "go-minus" | "off" | "on" | "temp-on" | "temp-off";
    surface: "physical" | "virtual" | string;
  }>;
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

export type HighlightAction = "on" | "off" | "toggle" | "next" | "previous" | "all";

export interface HighlightFixtureSummary {
  fixture_id: string;
  number?: number | null;
  /** Accepted for compatibility with patch-shaped fixture summaries. */
  fixture_number?: number | null;
  name?: string | null;
}

/**
 * Authoritative, transient Highlight state for the current desk and user.
 * `mode` describes the actual programmer selection independently of whether Highlight is active.
 * `active_index` is zero-based and null while the complete live selection is selected.
 */
export interface HighlightState {
  active: boolean;
  mode: "selection" | "step";
  output_enabled: boolean;
  capture_only: boolean;
  remembered: HighlightFixtureSummary[];
  active_index: number | null;
  active_fixture: HighlightFixtureSummary | null;
  can_previous: boolean;
  can_next: boolean;
  owner_user_id: string | null;
  owner_user_name?: string | null;
  message?: string | null;
}

export interface BootstrapSnapshot {
  api_version: string;
  attribute_registry: AttributeDescriptor[];
  users: DeskUser[];
  desks: ControlDesk[];
  clients: ClientSummary[];
  active_show: ShowEntry | null;
  active_programmers: ProgrammerState[];
  frame_rate_hz: number;
  output_health: OutputHealth;
  active_timecode_source: string | null;
  active_timecode: string | null;
  active_show_error: string | null;
  hardware_connected: boolean;
}

export interface AttributeDescriptor {
  id: string;
  label: string;
  family: "intensity" | "position" | "color" | "beam" | "focus" | "control" | "custom";
  value_type: "continuous" | "color" | "indexed" | "control";
  default_unit: string | null;
}

export interface SessionResponse {
  session_id: string;
  client_id: string;
  token: string;
  user: DeskUser;
  desk: ControlDesk;
}
export interface ClientSummary {
  client_id: string;
  name: string;
  connected: boolean;
  last_connected_at: string | null;
  desk: ControlDesk;
  can_remove: boolean;
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
  playback_layout?: PlaybackSurfaceLayout | null;
}
export interface PlaybackSurfaceRow {
  first_playback_slot: number;
  has_fader: boolean;
  button_count: number;
}
export interface PlaybackSurfaceLayout {
  playbacks_per_row: number;
  rows: PlaybackSurfaceRow[];
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
  playback_layout?: PlaybackSurfaceLayout | null;
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
  /** Schema-v2 fixtures patch each independently addressable split separately. */
  split_patches?: SplitPatch[];
  /** Exact raw values captured with the embedded profile snapshot. */
  highlight_overrides?: Record<string, number>;
  move_in_black_enabled?: boolean;
  move_in_black_delay_millis?: number;
}

export interface MultiPatchInstance {
  id: string;
  name: string;
  universe: number | null;
  address: number | null;
  location: { x: number; y: number; z: number };
  rotation: { x: number; y: number; z: number };
  split_patches?: SplitPatch[];
}

export interface SplitPatch {
  split: number;
  universe: number | null;
  address: number | null;
}

export interface FixtureProfile {
  schema_version: 2;
  id: string;
  revision: number;
  manufacturer: string;
  name: string;
  short_name: string;
  fixture_type: string;
  patch_policy?: "dmx" | "visual_only";
  notes: string;
  photograph_asset: string | null;
  stage_icon_asset: string | null;
  model_asset: string | null;
  model_units?: "auto" | "metres";
  physical: FixtureProfilePhysical;
  modes: FixtureMode[];
  hazardous: boolean;
  direct_control_protocols: Array<"citp">;
  signal_loss_policy: { type: string; duration_millis?: number };
  reserved_source: string | null;
}

export interface FixtureProfilePhysical {
  width_millimetres: number | null;
  height_millimetres: number | null;
  depth_millimetres: number | null;
  weight_kilograms: number | null;
  power_watts: number | null;
  connectors?: string;
  light_source?: string;
  color_temperature_kelvin?: number | null;
  color_rendering_index?: number | null;
  luminous_output_lumens?: number | null;
  lens?: string;
  beam_angle_degrees?: number | null;
}

export interface FixtureMode {
  id: string;
  name: string;
  notes: string;
  splits: FixtureSplit[];
  heads: FixtureHead[];
  channels: FixtureChannel[];
  color_systems: HeadColorSystem[];
  control_actions: ControlAction[];
  geometry: GeometryGraph;
}

export interface FixtureSplit {
  number: number;
  footprint: number;
}

export interface FixtureHead {
  id: string;
  name: string;
  master_shared: boolean;
}

export type ChannelResolution = "u8" | "u16" | "u24" | "u32";
export type ChannelBehavior = "controlled" | "static";

export interface FixtureChannel {
  id: string;
  head_id: string;
  split: number;
  attribute: string;
  resolution: ChannelResolution;
  secondary_slots: number[];
  default_raw: number;
  highlight_raw: number;
  physical_min: number | null;
  physical_max: number | null;
  unit: string | null;
  invert: boolean;
  snap: boolean;
  reacts_to_virtual_intensity: boolean;
  reacts_to_sequence_master: boolean;
  reacts_to_group_master: boolean;
  reacts_to_grand_master: boolean;
  behavior: ChannelBehavior;
  functions: ChannelFunction[];
}

export interface ChannelFunction {
  id: string;
  name: string;
  dmx_from: number;
  dmx_to: number;
  attribute: string;
  priority: number;
  behavior: ChannelFunctionBehavior;
}

export type ChannelFunctionBehavior =
  | { type: "continuous"; physical_min: number; physical_max: number; unit: string | null }
  | { type: "fixed"; semantic_id: string; label: string; raw_value: number }
  | { type: "indexed"; semantic_id: string; label: string; raw_value: number }
  | { type: "control"; action_id: string };

export type ControlActionKind = "latched" | "momentary" | "timed_pulse";

export interface ControlAction {
  id: string;
  name: string;
  kind: ControlActionKind;
  duration_millis: number | null;
  assignments: ControlActionAssignment[];
}

export interface ControlActionAssignment {
  channel_id: string;
  active_raw: number;
  inactive_raw: number;
}

export interface HeadColorSystem {
  head_id: string;
  correction_matrix: [
    [number, number, number],
    [number, number, number],
    [number, number, number],
  ];
  system: ColorSystem;
}

export type ColorSystem =
  | { type: "additive"; emitters: EmitterBinding[] }
  | { type: "subtractive"; cyan_channel_id: string; magenta_channel_id: string; yellow_channel_id: string }
  | { type: "discrete_wheel"; channel_id: string; slots: ColorWheelSlot[] };

export interface EmitterBinding {
  channel_id: string;
  name: string;
  xyz: XyzValue;
  maximum_level: number;
  response_curve: number;
  visible: boolean;
}

export interface ColorWheelSlot {
  semantic_id: string;
  label: string;
  dmx_from: number;
  dmx_to: number;
  measured_xyz: XyzValue | null;
}

export interface XyzValue {
  x: number;
  y: number;
  z: number;
}

export interface GeometryGraph {
  nodes: GeometryNode[];
  emitters: GeometryEmitter[];
}

export interface Vector3Value {
  x: number;
  y: number;
  z: number;
}

export interface GeometryNode {
  id: string;
  name: string;
  parent_id: string | null;
  transform: { translation: Vector3Value; rotation_degrees: Vector3Value; scale: Vector3Value };
  pivot: Vector3Value;
  glb_node: string | null;
  motion: GeometryMotion | null;
}

export interface GeometryMotion {
  attribute: string;
  kind: "rotation" | "translation";
  axis: Vector3Value;
  physical_min: number;
  physical_max: number;
}

export interface GeometryEmitter {
  id: string;
  name: string;
  node_id: string;
  head_id: string;
  origin: Vector3Value;
  orientation_degrees: Vector3Value;
  beam_angle_degrees: number;
  field_angle_degrees: number;
  feather: number;
  focus: number;
  layout: EmitterLayout;
}

export type EmitterLayout =
  | { type: "point" }
  | { type: "matrix"; columns: number; rows: number; spacing: Vector3Value }
  | { type: "ring"; count: number; radius_millimetres: number }
  | { type: "strip"; count: number; spacing_millimetres: number }
  | { type: "explicit_pixels"; positions: Vector3Value[] };

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
  color_calibration: {
    emitters: Array<{ name: string; xyz: XyzValue; limit: number }>;
    correction_matrix: number[][];
  } | null;
  physical: {
    pan_range_degrees?: number | null;
    tilt_range_degrees?: number | null;
    width_millimetres?: number | null;
    height_millimetres?: number | null;
    depth_millimetres?: number | null;
    weight_kilograms?: number | null;
    power_watts?: number | null;
  };
  model_asset?: string | null;
  icon_asset?: string | null;
  hazardous: boolean;
  direct_control_protocols: Array<"citp">;
  signal_loss_policy: { type: string; duration_millis?: number };
  safe_values: Record<string, unknown>;
  profile_id?: string | null;
  mode_id?: string | null;
  profile_snapshot?: FixtureProfile | null;
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

export interface OutputRoute {
  protocol: "art_net" | "sacn";
  logical_universe: number;
  destination_universe: number;
  delivery_mode: "broadcast" | "multicast" | "unicast";
  destination: string | null;
  enabled: boolean;
  minimum_slots: number;
}

export interface PatchSnapshot {
  revision: number;
  fixtures: PatchedFixture[];
  routes: OutputRoute[];
}

export interface PatchLayer {
  id: string;
  name: string;
  order: number;
}

export interface Cue {
  id?: string;
  cue_only?: boolean;
  number: number;
  name: string;
  fade_millis: number;
  delay_millis: number;
  trigger: { type: string; [key: string]: unknown };
  changes: Array<{
    fixture_id: string;
    attribute: string;
    value: AttributeValue | null;
    automatic_restore?: boolean;
    fade_millis?: number;
    delay_millis?: number;
  }>;
  group_changes?: Array<{
    group_id: string;
    attribute: string;
    value: AttributeValue | null;
    automatic_restore?: boolean;
    fade_millis?: number;
    delay_millis?: number;
  }>;
  phasers?: unknown[];
}

export type AttributeValue =
  | { kind: "normalized"; value: number }
  | { kind: "discrete"; value: string }
  | { kind: "color_xyz"; value: { x: number; y: number; z: number } }
  | { kind: "raw_dmx"; value: number }
  | { kind: "raw_dmx_exact"; value: number };

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
  /** Post-profile values projected through the same calibrated/channel/master path as DMX. */
  profile_output_values?: Array<{
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
  chaser_xfade_percent?: number;
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
    fader_position?: number;
    fader_pickup_required?: boolean;
    flash: boolean;
    transition_timing_bypassed?: boolean;
    manual_xfade_position?: number;
    manual_xfade_direction?: "towards_high" | "towards_low";
    manual_xfade_progress?: number;
    temporary_active?: boolean;
    temporary_master?: number;
    swap_active?: boolean;
    enabled?: boolean;
    current_cue_number?: number | null;
    loaded_cue_number?: number | null;
    normal_next_cue_number?: number | null;
    effective_next_cue_number?: number | null;
    effective_next_is_loaded?: boolean;
  }>;
  desk: ControlDesk;
  active_page: number;
  selected_playback?: number | null;
  authoritative_controls?: {
    speed_groups: SpeedSnapshot[];
    groups: Array<{ id: string; master: number; flash_level: number }>;
    grand_master: { level: number; blackout: boolean; flash_active: boolean; dynamics_paused: boolean };
    programmer_fade_millis: number;
    cue_fade_millis: number;
  };
}

export type PlaybackButtonAction = "on" | "off" | "toggle" | "go" | "go_minus" | "fast_forward" | "fast_rewind" | "flash" | "temp" | "swap" | "select" | "select_contents" | "select_dereferenced" | "learn" | "double" | "half" | "pause" | "blackout" | "pause_dynamics" | "none";
export interface PlaybackDefinition {
  number: number;
  name: string;
  target: { type: "cue_list"; cue_list_id: string } | { type: "group"; group_id: string } | { type: "speed_group"; group: string } | { type: "programmer_fade" } | { type: "cue_fade" } | { type: "grand_master" };
  buttons: [PlaybackButtonAction, PlaybackButtonAction, PlaybackButtonAction];
  /** Missing only on legacy show files; every save writes an explicit topology. */
  button_count?: 0 | 1 | 2 | 3;
  fader: "master" | "temp" | "speed" | "x_fade" | "direct_bpm" | "centered_relative" | "learned_percentage";
  /** Missing only on legacy show files; every save writes an explicit topology. */
  has_fader?: boolean;
  go_activates: boolean;
  auto_off: boolean;
  xfade_millis: number;
  color?: string;
  flash_release?: "release_all" | "release_intensity_only";
  protect_from_swap?: boolean;
  presentation_icon?: string;
  presentation_image?: string;
}
export interface PlaybackPage {
  number: number;
  name: string;
  slots: Record<string, number>;
}

export interface VirtualPlaybackExclusionZone {
  id: string;
  name: string;
  slots: number[];
}

export interface VirtualPlaybackExclusionSnapshot {
  show_id: string;
  desk_id: string;
  surfaces: Record<string, VirtualPlaybackExclusionZone[]>;
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
  matter_enabled?: boolean;
  update_settings_by_desk?: Record<string, unknown>;
  file_manager_system_picker_fallback: boolean;
  file_manager_roots: Array<{
    id: string;
    label: string;
    path: string;
    icon?: string;
  }>;
}

export interface MatterPairingData {
  qr_code: string;
  manual_code: string;
  discriminator: number;
}

export interface MatterPlaybackLight {
  endpoint_id: number;
  page: number;
  playback: number;
  playback_number: number;
  name: string;
  on: boolean;
  level: number;
}

export interface MatterBridgeStatus {
  enabled: boolean;
  transport: "disabled" | "adapter_ready" | "starting" | "running" | "failed";
  commissionable: boolean;
  network_running: boolean;
  commissioned: boolean;
  commissioning_window_open: boolean;
  pairing?: MatterPairingData | null;
  revision: number;
  lights: MatterPlaybackLight[];
  limitation?: string | null;
}

export type SpeedGroupId = "A" | "B" | "C" | "D" | "E";

export type FrequencyPreset = "sub" | "low" | "mid" | "high" | "full_range";

export type FrequencySelection =
  | { type: "preset"; preset: FrequencyPreset }
  | { type: "custom"; low_hz: number; high_hz: number };

export interface SoundToLightConfig {
  enabled: boolean;
  analysis_mode: "tempo_bpm";
  frequency: FrequencySelection;
  input_gain_db: number;
  confidence_threshold: number;
  smoothing: number;
  minimum_bpm: number;
  maximum_bpm: number;
  signal_hold_millis: number;
  multiplier: number;
}

export interface SoundObservation {
  captured_at_millis: number;
  source_available: boolean;
  usable_signal: boolean;
  level: number;
  selected_band_level: number;
  detected_bpm: number | null;
  confidence: number;
}

export type SoundLossReason =
  | "source_unavailable"
  | "no_usable_signal"
  | "low_confidence"
  | "tempo_outside_range"
  | "waiting_for_analysis";

export type SoundStatus =
  | { state: "disabled" }
  | { state: "active"; detected_bpm: number; confidence: number }
  | { state: "holding"; reason: SoundLossReason; remaining_millis: number }
  | { state: "manual_fallback"; reason: SoundLossReason };

export interface SpeedSnapshot {
  manual_bpm: number;
  sound_bpm: number | null;
  effective_bpm: number;
  source: "manual" | "sound" | "held_sound" | "manual_fallback";
  sound_status: SoundStatus;
  paused: boolean;
  phase_advancing: boolean;
  speed_master_scale: number;
  sound_multiplier: number;
  source_available: boolean;
  usable_signal: boolean;
  input_level: number;
  selected_band_level: number;
}

export interface SpeedGroupSoundState {
  group: SpeedGroupId;
  configuration: SoundToLightConfig;
  snapshot: SpeedSnapshot;
}

export type SpeedGroupAction = "learn" | "double" | "half" | "pause";

export interface SpeedGroupActionInput {
  action: SpeedGroupAction;
  captured_at_millis?: number;
}

export interface StoredGroup {
  name?: string;
  color?: string;
  icon?: string;
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

export interface GeneratedFixturePresetResult {
  created: Array<{
    id: string;
    name: string;
    family: string;
  }>;
}

export interface ServerEvent {
  revision: number;
  kind: string;
  payload: Record<string, unknown>;
}

export interface CommandHistoryEntry {
  id: string;
  desk_id: string;
  session_id: string;
  command: string;
  status: "accepted" | "rejected";
  feedback: string;
  source: "software" | "osc";
  at: string;
}

export type ConnectionStatus = "connecting" | "connected" | "offline" | "error";
