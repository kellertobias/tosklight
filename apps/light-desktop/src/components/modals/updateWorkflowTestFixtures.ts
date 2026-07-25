import type {
	UpdatePreview,
	UpdateResult,
	UpdateTargetIdentity,
} from "../../api/types";
import type {
	ProgrammingUpdateMenuEntry,
	ProgrammingUpdateMutationResult,
	ProgrammingUpdateObject,
	UpdatePreviewAuthority,
	UpdateTargetsAuthority,
} from "../../features/programmingUpdate/contracts";

type ProgrammingUpdateTarget = UpdatePreviewAuthority["requestTarget"];

export const SHOW_ID = "show-a";
export const SCOPE_KEY = "authority-a";
export const CORRELATION_ID = "11111111-1111-4111-8111-111111111111";
export const PROGRAMMER_REVISION = "a".repeat(64);

export const cueTarget: UpdateTargetIdentity = {
	family: { type: "cue" },
	object_id: "cue-list-a",
	name: "Main Cuelist",
	playback_number: 7,
	cue: { id: "cue-2", number: 2 },
};

export const cueRequestTarget: ProgrammingUpdateTarget = {
	type: "cue",
	cue_list_id: "cue-list-a",
	playback_number: 7,
	cue_id: "cue-2",
	cue_number: 2,
	validate_active_context: true,
};

const cueObject = {
	kind: "cue_list" as const,
	object_id: "legacy-cue-list-a",
	object_revision: 4,
};

export const existingPreview: UpdatePreview = {
	revision: 4,
	show_revision: 12,
	programmer_revision: PROGRAMMER_REVISION,
	target: cueTarget,
	mode: { target_type: "cue", mode: "existing_only" },
	items: [
		{
			address: {
				type: "fixture_attribute",
				fixture_id: "fixture-1",
				attribute: "intensity",
			},
			outcome: {
				outcome: "change_at_source",
				source: { cue_id: "cue-1", cue_number: 1, cue_index: 0 },
			},
		},
	],
};

export const addNewPreview: UpdatePreview = {
	...existingPreview,
	mode: { target_type: "cue", mode: "add_new" },
	items: [
		{
			address: {
				type: "fixture_attribute",
				fixture_id: "fixture-2",
				attribute: "color.red",
			},
			outcome: {
				outcome: "add_new_to_current_cue",
				cue: { cue_id: "cue-2", cue_number: 2, cue_index: 1 },
			},
		},
	],
};

export const existingAuthority = authorityFor(
	existingPreview,
	cueRequestTarget,
	cueObject,
);
export const addNewAuthority = authorityFor(
	addNewPreview,
	cueRequestTarget,
	cueObject,
);

export const cueEntry: ProgrammingUpdateMenuEntry = {
	revision: cueObject.object_revision,
	target: cueTarget,
	active_or_referenced: true,
	existing_preview: existingPreview,
	add_new_preview: addNewPreview,
	requestTarget: cueRequestTarget,
	object: cueObject,
	programmerRevision: PROGRAMMER_REVISION,
	showRevision: 12,
	existingAuthority,
	addNewAuthority,
};

export function targetsFor(
	entries: ProgrammingUpdateMenuEntry[] = [cueEntry],
): UpdateTargetsAuthority {
	return {
		scopeKey: SCOPE_KEY,
		requestId: "targets-a",
		correlationId: CORRELATION_ID,
		showId: SHOW_ID,
		showRevision: 12,
		entries,
	};
}

export function resultFor(
	target: UpdateResult["target"] = cueTarget,
): UpdateResult {
	return {
		target,
		revision_before: 4,
		revision_after: 5,
		eligible_count: 1,
		changed_count: 1,
		added_count: 1,
		ignored_count: 0,
		changed_cues: [],
		programmer_values_retained: true,
	};
}

export function mutationFor(
	target: UpdateResult["target"] = cueTarget,
): ProgrammingUpdateMutationResult {
	return {
		requestId: "apply-a",
		correlationId: CORRELATION_ID,
		replayed: false,
		showRevision: 13,
		eventSequence: 42,
		object: mutationObject(target),
		result: resultFor(target),
	};
}

export function cueMenuEntryFor(
	preview: UpdatePreview,
	addPreview: UpdatePreview,
	storageObjectId: string,
): ProgrammingUpdateMenuEntry {
	const requestTarget = requestTargetFor(preview.target);
	const object = {
		kind: "cue_list" as const,
		object_id: storageObjectId,
		object_revision: preview.revision,
	};
	return {
		revision: preview.revision,
		target: preview.target,
		active_or_referenced: true,
		existing_preview: preview,
		add_new_preview: addPreview,
		requestTarget,
		object,
		programmerRevision: preview.programmer_revision,
		showRevision: preview.show_revision,
		existingAuthority: authorityFor(preview, requestTarget, object),
		addNewAuthority: authorityFor(addPreview, requestTarget, object),
	};
}

function authorityFor(
	preview: UpdatePreview,
	requestTarget: ProgrammingUpdateTarget,
	object: UpdatePreviewAuthority["object"],
): UpdatePreviewAuthority {
	return {
		scopeKey: SCOPE_KEY,
		requestId: "targets-a",
		correlationId: CORRELATION_ID,
		showId: SHOW_ID,
		showRevision: preview.show_revision,
		requestTarget,
		object,
		programmerRevision: preview.programmer_revision,
		preview,
	};
}

function requestTargetFor(
	target: UpdateTargetIdentity,
): ProgrammingUpdateTarget {
	if (target.family.type !== "cue")
		throw new Error("test fixture requires a Cue target");
	return {
		type: "cue",
		cue_list_id: target.object_id,
		...(target.playback_number == null
			? {}
			: { playback_number: target.playback_number }),
		...(target.cue == null
			? {}
			: { cue_id: target.cue.id, cue_number: target.cue.number }),
		validate_active_context: true,
	};
}

function mutationObject(
	target: UpdateResult["target"],
): ProgrammingUpdateObject {
	if (target.family.type === "group") {
		return {
			kind: "group",
			id: target.object_id,
			revision: 5,
			updated_at: "",
			body: { name: target.name, fixtures: [] },
		};
	}
	return {
		kind: "cue_list",
		id: "legacy-cue-list-a",
		revision: 5,
		updated_at: "",
		body: {
			id: target.object_id,
			name: target.name,
			cues: [],
			mode: "sequence",
			priority: 0,
			looped: false,
		},
	};
}
