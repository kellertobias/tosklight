import type {
	ProgrammingUpdateActionRequest,
	ProgrammingUpdateObjectIdentity,
	ProgrammingUpdatePreview,
	ProgrammingUpdatePreviewRequest,
	ProgrammingUpdatePreviewResponse,
	ProgrammingUpdateSettingsProjection,
	ProgrammingUpdateTarget,
	ProgrammingUpdateTargetsRequest,
	ProgrammingUpdateTargetsResponse,
} from "../features/programmingUpdate/contracts";
import type {
	ProgrammingUpdateActionRequest as WireActionRequest,
	ProgrammingUpdateAddress as WireAddress,
	ProgrammingUpdateItemOutcome as WireItemOutcome,
	ProgrammingUpdateMode as WireMode,
	ProgrammingUpdateObjectIdentity as WireObjectIdentity,
	ProgrammingUpdatePreview as WirePreview,
	ProgrammingUpdatePreviewRequest as WirePreviewRequest,
	ProgrammingUpdatePreviewResponse as WirePreviewResponse,
	ProgrammingUpdateSettings as WireSettings,
	ProgrammingUpdateSettingsProjection as WireSettingsProjection,
	ProgrammingUpdateTarget as WireTarget,
	ProgrammingUpdateTargetIdentity as WireTargetIdentity,
	ProgrammingUpdateTargetsRequest as WireTargetsRequest,
	ProgrammingUpdateTargetsResponse as WireTargetsResponse,
} from "./generated/light-wire";
import type { DecodedProgrammingUpdateActionOutcome } from "./programmingUpdateWire";
import type {
	UpdateAddress,
	UpdateItemOutcome,
	UpdateMode,
	UpdatePreviewItem,
	UpdateSettings,
	UpdateTargetIdentity,
} from "./types";

export function wirePreviewRequest(
	request: ProgrammingUpdatePreviewRequest,
): WirePreviewRequest {
	return {
		request_id: request.request_id,
		target: wireTarget(request.target),
		mode: wireMode(request.mode),
	};
}

export function wireTargetsRequest(
	request: ProgrammingUpdateTargetsRequest,
): WireTargetsRequest {
	return { request_id: request.request_id, filter: request.filter };
}

export function wireActionRequest(
	request: ProgrammingUpdateActionRequest,
): WireActionRequest {
	const action = request.action;
	return {
		request_id: request.request_id,
		action:
			action.type === "confirm_preview"
				? {
						type: action.type,
						target: wireTarget(action.target),
						mode: wireMode(action.mode),
						expected_object_revision: action.expected_object_revision,
						expected_programmer_revision: action.expected_programmer_revision,
					}
				: {
						type: action.type,
						target: wireTarget(action.target),
						mode: wireMode(action.mode),
					},
	};
}

export function wireSettings(settings: UpdateSettings): WireSettings {
	return {
		cue_mode: settings.cue_mode,
		preset_mode: settings.preset_mode,
		group_mode: settings.group_mode,
		show_update_modal_on_touch: settings.show_update_modal_on_touch,
	};
}

export function programmingUpdatePreviewResponse(
	response: WirePreviewResponse,
): ProgrammingUpdatePreviewResponse {
	return {
		request_id: response.request_id,
		correlation_id: response.correlation_id,
		show_id: response.show_id,
		show_revision: response.show_revision,
		object: objectIdentity(response.object),
		programmer_revision: response.programmer_revision,
		preview: preview(response.preview),
	};
}

export function programmingUpdateTargetsResponse(
	response: WireTargetsResponse,
): ProgrammingUpdateTargetsResponse {
	return {
		request_id: response.request_id,
		correlation_id: response.correlation_id,
		show_id: response.show_id,
		show_revision: response.show_revision,
		targets: response.targets.map((entry) => ({
			request_target: target(entry.request_target),
			object: objectIdentity(entry.object),
			programmer_revision: entry.programmer_revision,
			active_or_referenced: entry.active_or_referenced,
			existing_preview: preview(entry.existing_preview),
			add_new_preview: preview(entry.add_new_preview),
		})),
	};
}

export function programmingUpdateActionOutcome(
	outcome: DecodedProgrammingUpdateActionOutcome,
) {
	return {
		status: outcome.status,
		request_id: outcome.request_id,
		correlation_id: outcome.correlation_id,
		replayed: outcome.replayed,
		show_id: outcome.show_id,
		show_revision: outcome.show_revision,
		projection: projection(outcome.projection),
		event_sequence: outcome.event_sequence,
		summary: {
			...outcome.summary,
			target: targetIdentity(outcome.summary.target),
		},
	};
}

export function programmingUpdateSettingsProjection(
	projection: WireSettingsProjection,
): ProgrammingUpdateSettingsProjection {
	return {
		desk_id: projection.desk_id,
		settings: { ...projection.settings },
	};
}

function wireTarget(value: ProgrammingUpdateTarget): WireTarget {
	if (value.type !== "cue") return { ...value };
	return {
		type: value.type,
		cue_list_id: value.cue_list_id,
		...(value.playback_number == null
			? {}
			: { playback_number: value.playback_number }),
		...(value.cue_id == null ? {} : { cue_id: value.cue_id }),
		...(value.cue_number == null ? {} : { cue_number: value.cue_number }),
		validate_active_context: value.validate_active_context,
	};
}

function target(value: WireTarget): ProgrammingUpdateTarget {
	if (value.type !== "cue") return { ...value };
	return {
		type: value.type,
		cue_list_id: value.cue_list_id,
		...(value.playback_number == null
			? {}
			: { playback_number: value.playback_number }),
		...(value.cue_id == null ? {} : { cue_id: value.cue_id }),
		...(value.cue_number == null ? {} : { cue_number: value.cue_number }),
		validate_active_context: value.validate_active_context,
	};
}

function wireMode(mode: UpdateMode): WireMode {
	return mode.target_type === "cue" ? { ...mode } : { ...mode };
}

function objectIdentity(
	value: WireObjectIdentity,
): ProgrammingUpdateObjectIdentity {
	return {
		kind: value.kind,
		object_id: value.object_id,
		object_revision: value.object_revision,
	};
}

function preview(value: WirePreview): ProgrammingUpdatePreview {
	return {
		target: targetIdentity(value.target),
		mode: wireMode(value.mode),
		items: value.items.map(previewItem),
	};
}

function targetIdentity(value: WireTargetIdentity): UpdateTargetIdentity {
	return {
		family: { ...value.family },
		object_id: value.object_id,
		name: value.name,
		...(value.playback_number == null
			? {}
			: { playback_number: value.playback_number }),
		...(value.cue == null ? {} : { cue: { ...value.cue } }),
	};
}

function previewItem(value: WirePreview["items"][number]): UpdatePreviewItem {
	return {
		address: address(value.address),
		outcome: itemOutcome(value.outcome),
	};
}

function address(value: WireAddress): UpdateAddress {
	return { ...value };
}

function itemOutcome(value: WireItemOutcome): UpdateItemOutcome {
	if (value.outcome !== "unchanged") return { ...value };
	return value.source == null
		? { outcome: "unchanged" }
		: { outcome: "unchanged", source: { ...value.source } };
}

function projection(
	value: DecodedProgrammingUpdateActionOutcome["projection"],
) {
	const identity = {
		object_id: value.object_id,
		object_revision: value.object_revision,
	};
	switch (value.kind) {
		case "cue_list":
			return { ...identity, kind: value.kind, body: value.body };
		case "preset":
			return { ...identity, kind: value.kind, body: value.body };
		case "group":
			return { ...identity, kind: value.kind, body: value.body };
	}
}
