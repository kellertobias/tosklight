import type { ShowObject } from "../showObjects/contracts";

export interface GroupRecordingTarget {
	objectId: string;
	expectedObjectRevision: number;
	label: string;
}

/** Captures the exact object identity and revision when a recording dialog opens. */
export function captureGroupRecordingTarget(
	group: ShowObject<"group">,
): GroupRecordingTarget {
	return {
		objectId: group.id,
		expectedObjectRevision: group.revision,
		label: group.body.name ?? `Group ${group.id}`,
	};
}

export function emptyGroupRecordingTarget(id: string): GroupRecordingTarget {
	return { objectId: id, expectedObjectRevision: 0, label: `Group ${id}` };
}
