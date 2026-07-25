import type {
	UpdateMenuEntry,
	UpdateMode,
	UpdatePreview,
	UpdatePreviewItem,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
	UpdateTargetIdentity,
	UpdateTargetRequest,
} from "../../api/types";
import type { ShowObject, ShowObjectBodies } from "../showObjects/contracts";

export type ProgrammingUpdateObjectKind = "cue_list" | "preset" | "group";

export type ProgrammingUpdateTarget =
	| {
			type: "cue";
			cue_list_id: string;
			playback_number?: number | null;
			cue_id?: string | null;
			cue_number?: number | null;
			validate_active_context: boolean;
	  }
	| { type: "preset"; object_id: string }
	| { type: "group"; object_id: string };

export interface ProgrammingUpdateObjectIdentity {
	kind: ProgrammingUpdateObjectKind;
	object_id: string;
	object_revision: number;
}

export interface ProgrammingUpdatePreview {
	target: UpdateTargetIdentity;
	mode: UpdateMode;
	items: UpdatePreviewItem[];
}

export interface ProgrammingUpdatePreviewRequest {
	request_id: string;
	target: ProgrammingUpdateTarget;
	mode: UpdateMode;
}

export interface ProgrammingUpdatePreviewResponse {
	request_id: string;
	correlation_id: string;
	show_id: string;
	show_revision: number;
	object: ProgrammingUpdateObjectIdentity;
	programmer_revision: string;
	preview: ProgrammingUpdatePreview;
}

export interface ProgrammingUpdateTargetsRequest {
	request_id: string;
	filter: UpdateTargetFilter;
}

export interface ProgrammingUpdateTargetEntry {
	request_target: ProgrammingUpdateTarget;
	object: ProgrammingUpdateObjectIdentity;
	programmer_revision: string;
	active_or_referenced: boolean;
	existing_preview: ProgrammingUpdatePreview;
	add_new_preview: ProgrammingUpdatePreview;
}

export interface ProgrammingUpdateTargetsResponse {
	request_id: string;
	correlation_id: string;
	show_id: string;
	show_revision: number;
	targets: ProgrammingUpdateTargetEntry[];
}

export type ProgrammingUpdateAction =
	| {
			type: "confirm_preview";
			target: ProgrammingUpdateTarget;
			mode: UpdateMode;
			expected_object_revision: number;
			expected_programmer_revision: string;
	  }
	| {
			type: "apply_direct";
			target: ProgrammingUpdateTarget;
			mode: UpdateMode;
	  };

export interface ProgrammingUpdateActionRequest {
	request_id: string;
	action: ProgrammingUpdateAction;
}

export type ProgrammingUpdateProjection = {
	[K in ProgrammingUpdateObjectKind]: {
		kind: K;
		object_id: string;
		object_revision: number;
		body: ShowObjectBodies[K];
	};
}[ProgrammingUpdateObjectKind];

export interface ProgrammingUpdateActionOutcome {
	status: "changed";
	request_id: string;
	correlation_id: string;
	replayed: boolean;
	show_id: string;
	show_revision: number;
	projection: ProgrammingUpdateProjection;
	event_sequence: number;
	summary: UpdateResult;
}

export interface ProgrammingUpdateSettingsProjection {
	desk_id: string;
	settings: UpdateSettings;
}

export interface ProgrammingUpdateTransport {
	preview(
		showId: string,
		request: ProgrammingUpdatePreviewRequest,
	): Promise<ProgrammingUpdatePreviewResponse>;
	targets(
		showId: string,
		request: ProgrammingUpdateTargetsRequest,
	): Promise<ProgrammingUpdateTargetsResponse>;
	apply(
		showId: string,
		expectedShowRevision: number,
		request: ProgrammingUpdateActionRequest,
	): Promise<ProgrammingUpdateActionOutcome>;
	loadSettings(deskId: string): Promise<ProgrammingUpdateSettingsProjection>;
	saveSettings(
		deskId: string,
		settings: UpdateSettings,
	): Promise<ProgrammingUpdateSettingsProjection>;
}

export class ProgrammingUpdateTransportError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly currentShowRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammingUpdateTransportError";
	}
}

export interface ProgrammingUpdateScope {
	showId: string;
	deskId: string;
	userId: string;
	initialShowRevision: number | null;
}

export interface UpdatePreviewAuthority {
	scopeKey: string;
	requestId: string;
	correlationId: string;
	showId: string;
	showRevision: number;
	requestTarget: ProgrammingUpdateTarget;
	object: ProgrammingUpdateObjectIdentity;
	programmerRevision: string;
	preview: UpdatePreview;
}

export interface ProgrammingUpdateMenuEntry extends UpdateMenuEntry {
	requestTarget: ProgrammingUpdateTarget;
	object: ProgrammingUpdateObjectIdentity;
	programmerRevision: string;
	showRevision: number;
	existingAuthority: UpdatePreviewAuthority;
	addNewAuthority: UpdatePreviewAuthority;
}

export interface UpdateTargetsAuthority {
	scopeKey: string;
	requestId: string;
	correlationId: string;
	showId: string;
	showRevision: number;
	entries: ProgrammingUpdateMenuEntry[];
}

export type ProgrammingUpdateObject = {
	[K in ProgrammingUpdateObjectKind]: ShowObject<K> & { kind: K };
}[ProgrammingUpdateObjectKind];

export interface ProgrammingUpdateMutationResult {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	showRevision: number;
	eventSequence: number;
	object: ProgrammingUpdateObject;
	result: UpdateResult;
}

export interface ProgrammingUpdateCapability {
	readonly scopeKey: string;
	loadSettings(): Promise<UpdateSettings | null>;
	saveSettings(settings: UpdateSettings): Promise<UpdateSettings | null>;
	preview(
		target: UpdateTargetRequest,
		mode: UpdateMode,
	): Promise<UpdatePreviewAuthority | null>;
	targets(filter: UpdateTargetFilter): Promise<UpdateTargetsAuthority | null>;
	confirm(
		authority: UpdatePreviewAuthority,
	): Promise<ProgrammingUpdateMutationResult | null>;
	applyDirect(
		target: UpdateTargetRequest,
		mode: UpdateMode,
	): Promise<ProgrammingUpdateMutationResult | null>;
}
