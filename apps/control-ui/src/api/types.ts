import type { PresetFamily } from "../presetFamilies";
import type { ShowEntry } from "./types/desk";

export * from "./types/desk";
export type * from "./playbackRuntimeTypes";

export type CueUpdateMode =
	| "existing_only"
	| "existing_in_current_cue"
	| "add_to_current_cue"
	| "add_new";
export type ExistingContentMode = "update_existing" | "add_new";
export type UpdateTargetFilter =
	| "eligible_for_update_existing"
	| "show_all_active";

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

export type UpdateIgnoreReason =
	| "new_address"
	| "not_in_current_cue"
	| "not_in_active_tracked_state"
	| "new_group_member";

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
export interface FileOperationInput {
	operation:
		| "create_file"
		| "create_folder"
		| "rename"
		| "copy"
		| "move"
		| "trash"
		| "delete";
	sources?: string[];
	destination?: string;
	destination_root_id?: string;
	name?: string;
	replace?: boolean;
	conflict?: FileConflictChoice;
	apply_to_all?: boolean;
}
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

export * from "./types/fixtures";

export * from "./types/playback";

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

export * from "./types/configuration";

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
	number: number;
	values: Record<string, Record<string, unknown>>;
	group_values?: Record<string, Record<string, unknown>>;
	family?: PresetFamily | "All";
	color?: string;
	icon?: string;
}

export interface GeneratedFixturePresetResult {
	created: Array<{
		address: { family: PresetFamily; number: number };
		number: number;
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
