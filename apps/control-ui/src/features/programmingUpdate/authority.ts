import type {
	UpdatePreview,
	UpdateTargetIdentity,
	UpdateTargetRequest,
} from "../../api/types";
import type {
	ProgrammingUpdateActionOutcome,
	ProgrammingUpdateMenuEntry,
	ProgrammingUpdateMutationResult,
	ProgrammingUpdateObject,
	ProgrammingUpdatePreview,
	ProgrammingUpdatePreviewResponse,
	ProgrammingUpdateTarget,
	ProgrammingUpdateTargetsResponse,
	UpdatePreviewAuthority,
	UpdateTargetsAuthority,
} from "./contracts";

export function programmingUpdateTarget(
	target: UpdateTargetRequest,
): ProgrammingUpdateTarget {
	if (target.family.type === "cue") {
		return {
			type: "cue",
			cue_list_id: target.object_id,
			...(target.playback_number == null
				? {}
				: { playback_number: target.playback_number }),
			...(target.cue_id == null ? {} : { cue_id: target.cue_id }),
			...(target.cue_number == null ? {} : { cue_number: target.cue_number }),
			validate_active_context: target.validate_active_context ?? false,
		};
	}
	if (target.family.type === "preset" || target.family.type === "group")
		return { type: target.family.type, object_id: target.object_id };
	throw new Error("The v2 Update contract does not support this target family");
}

export function previewAuthority(
	response: ProgrammingUpdatePreviewResponse,
	requestTarget: ProgrammingUpdateTarget,
	scopeKey: string,
): UpdatePreviewAuthority {
	const exactTarget = targetFromIdentity(
		response.preview.target,
		requestTarget.type === "cue" && requestTarget.validate_active_context,
	);
	return {
		scopeKey,
		requestId: response.request_id,
		correlationId: response.correlation_id,
		showId: response.show_id,
		showRevision: response.show_revision,
		requestTarget: exactTarget,
		object: response.object,
		programmerRevision: response.programmer_revision,
		preview: uiPreview(
			response.preview,
			response.object.object_revision,
			response.show_revision,
			response.programmer_revision,
		),
	};
}

export function targetsAuthority(
	response: ProgrammingUpdateTargetsResponse,
	scopeKey: string,
): UpdateTargetsAuthority {
	return {
		scopeKey,
		requestId: response.request_id,
		correlationId: response.correlation_id,
		showId: response.show_id,
		showRevision: response.show_revision,
		entries: response.targets.map((entry) =>
			menuEntry(response, entry, scopeKey),
		),
	};
}

export function mutationResult(
	outcome: ProgrammingUpdateActionOutcome,
): ProgrammingUpdateMutationResult {
	const projection = outcome.projection;
	const object = projectionObject(projection);
	return {
		requestId: outcome.request_id,
		correlationId: outcome.correlation_id,
		replayed: outcome.replayed,
		showRevision: outcome.show_revision,
		eventSequence: outcome.event_sequence,
		object,
		result: outcome.summary,
	};
}

function projectionObject(
	projection: ProgrammingUpdateActionOutcome["projection"],
): ProgrammingUpdateObject {
	const identity = {
		id: projection.object_id,
		revision: projection.object_revision,
		updated_at: "",
	};
	switch (projection.kind) {
		case "cue_list":
			return { ...identity, kind: projection.kind, body: projection.body };
		case "preset":
			return { ...identity, kind: projection.kind, body: projection.body };
		case "group":
			return { ...identity, kind: projection.kind, body: projection.body };
	}
}

function menuEntry(
	response: ProgrammingUpdateTargetsResponse,
	entry: ProgrammingUpdateTargetsResponse["targets"][number],
	scopeKey: string,
): ProgrammingUpdateMenuEntry {
	const existingAuthority = menuPreviewAuthority(
		response,
		entry,
		entry.existing_preview,
		scopeKey,
	);
	const addNewAuthority = menuPreviewAuthority(
		response,
		entry,
		entry.add_new_preview,
		scopeKey,
	);
	return {
		revision: entry.object.object_revision,
		target: existingAuthority.preview.target,
		active_or_referenced: entry.active_or_referenced,
		existing_preview: existingAuthority.preview,
		add_new_preview: addNewAuthority.preview,
		requestTarget: entry.request_target,
		object: entry.object,
		programmerRevision: entry.programmer_revision,
		showRevision: response.show_revision,
		existingAuthority,
		addNewAuthority,
	};
}

function menuPreviewAuthority(
	response: ProgrammingUpdateTargetsResponse,
	entry: ProgrammingUpdateTargetsResponse["targets"][number],
	preview: ProgrammingUpdatePreview,
	scopeKey: string,
): UpdatePreviewAuthority {
	return {
		scopeKey,
		requestId: response.request_id,
		correlationId: response.correlation_id,
		showId: response.show_id,
		showRevision: response.show_revision,
		requestTarget: entry.request_target,
		object: entry.object,
		programmerRevision: entry.programmer_revision,
		preview: uiPreview(
			preview,
			entry.object.object_revision,
			response.show_revision,
			entry.programmer_revision,
		),
	};
}

function uiPreview(
	preview: ProgrammingUpdatePreview,
	objectRevision: number,
	showRevision: number,
	programmerRevision: string,
): UpdatePreview {
	return {
		revision: objectRevision,
		show_revision: showRevision,
		programmer_revision: programmerRevision,
		target: preview.target,
		mode: preview.mode,
		items: preview.items,
	};
}

function targetFromIdentity(
	target: UpdateTargetIdentity,
	validateActiveContext: boolean,
): ProgrammingUpdateTarget {
	if (target.family.type !== "cue")
		return { type: target.family.type, object_id: target.object_id };
	return {
		type: "cue",
		cue_list_id: target.object_id,
		...(target.playback_number == null
			? {}
			: { playback_number: target.playback_number }),
		...(target.cue == null
			? {}
			: { cue_id: target.cue.id, cue_number: target.cue.number }),
		validate_active_context: validateActiveContext,
	};
}
