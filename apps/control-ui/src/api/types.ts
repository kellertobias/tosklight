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
  group_values?: Record<string, Record<string, { value: unknown; changed_at: string }>>;
  selection_expression?: { type: string; group_id?: string; source_revision?: number; rule?: { type: string; n?: number; offset?: number } };
}

export interface BootstrapSnapshot {
  api_version: string;
  users: DeskUser[];
  active_show: ShowEntry | null;
  active_programmers: ProgrammerState[];
  frame_rate_hz: number;
  output_health: OutputHealth;
  active_timecode_source: string | null;
}

export interface SessionResponse {
  session_id: string;
  token: string;
  user: DeskUser;
}

export interface PatchedFixture {
  fixture_id: string;
  universe: number;
  address: number;
  direct_control?: { protocol: "citp"; ip_address: string; port: number } | null;
  definition: { name?: string; model: string; mode: string; footprint: number; manufacturer: string; direct_control_protocols?: Array<"citp">; heads?: Array<{ index: number; shared: boolean; parameters: Array<{ attribute: string }> }> };
  logical_heads: Array<{ fixture_id: string; head_index: number }>;
}

export interface MediaServerFixture {
  fixture_id: string;
  name: string;
  endpoint: { protocol: "citp"; ip_address: string; port: number };
  layers: Array<{ fixture_id: string; head_index: number }>;
  status: { online: boolean; last_success: string | null; last_error: string | null };
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

export interface Cue {
  number: number;
  name: string;
  fade_millis: number;
  delay_millis: number;
  trigger: { type: string; [key: string]: unknown };
}

export interface CueList {
  id: string;
  name: string;
  cues: Cue[];
  mode: "sequence" | "chaser";
  priority: number;
  looped: boolean;
}

export interface PlaybackSnapshot {
  cue_lists: CueList[];
  active: Array<{ cue_list_id: string; cue_index: number; paused: boolean }>;
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
}

export interface StoredGroup {
  name?: string;
  fixtures: string[];
  master?: number;
  playback_fader?: number | null;
  programming?: Record<string, unknown>;
  derived_from?: { source_group_id: string; rule: { type: string; n?: number; offset?: number } } | null;
  frozen_from?: { source_group_id: string; source_revision: number; captured_at: string } | null;
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
