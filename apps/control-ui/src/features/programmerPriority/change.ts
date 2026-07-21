import type {
	ProgrammerPriorityChange,
	ProgrammerPriorityProjection,
} from "./contracts";
import {
	assertPriorityRevision,
	samePriorityProjection,
} from "./projectionValue";
import { ProgrammerPriorityProtocolError } from "./transport";

export function priorityChangeUserId(change: ProgrammerPriorityChange) {
	return change.type === "upsert" ? change.projection.userId : change.userId;
}

export function priorityChangeRevision(change: ProgrammerPriorityChange) {
	return change.type === "upsert"
		? change.projection.revision
		: change.revision;
}

export function priorityProtocolError(
	message: string,
	sequence: number | null,
) {
	return new ProgrammerPriorityProtocolError(
		`Programmer priority ${message}`,
		sequence,
	);
}

export function assertDuplicatePriorityChange(
	change: ProgrammerPriorityChange,
	authoritative: ProgrammerPriorityProjection | null,
	authorityRevision: number | null,
	sequence: number,
) {
	const same =
		change.type === "remove"
			? !authoritative && authorityRevision === change.revision
			: Boolean(
					authoritative &&
						samePriorityProjection(authoritative, change.projection),
				);
	if (!same) throw priorityProtocolError("event sequence conflicts", sequence);
}

export function assertNextPriorityChange(
	change: ProgrammerPriorityChange,
	authoritative: ProgrammerPriorityProjection | null,
	currentRevision: number | null,
	sequence: number,
) {
	const revision = priorityChangeRevision(change);
	assertPriorityRevision(revision);
	if (currentRevision === null)
		throw priorityProtocolError(
			"event arrived without priority authority",
			sequence,
		);
	const valid =
		change.type === "upsert" && !authoritative
			? revision === currentRevision || revision === currentRevision + 1
			: revision === currentRevision + 1;
	if (!valid)
		throw priorityProtocolError("event revision is not contiguous", sequence);
}
