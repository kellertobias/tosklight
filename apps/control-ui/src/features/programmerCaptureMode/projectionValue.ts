import type { ProgrammerCaptureModeProjection } from "./contracts";
import { ProgrammerCaptureModeProtocolError } from "./transport";

export function canonicalCaptureModeProjection(
	projection: ProgrammerCaptureModeProjection,
): ProgrammerCaptureModeProjection {
	assertRevision(projection.revision);
	if (!projection.userId)
		throw new ProgrammerCaptureModeProtocolError(
			"Programmer capture mode projection is missing its user",
		);
	assertBoolean(projection.blind, "blind");
	assertBoolean(projection.preview, "preview");
	assertBoolean(
		projection.preloadCaptureProgrammer,
		"Preload Programmer capture",
	);
	return Object.freeze({
		userId: projection.userId,
		revision: projection.revision,
		blind: projection.blind,
		preview: projection.preview,
		preloadCaptureProgrammer: projection.preloadCaptureProgrammer,
	});
}

export function sameCaptureModeProjection(
	left: ProgrammerCaptureModeProjection,
	right: ProgrammerCaptureModeProjection,
) {
	return (
		left.userId === right.userId &&
		left.revision === right.revision &&
		left.blind === right.blind &&
		left.preview === right.preview &&
		left.preloadCaptureProgrammer === right.preloadCaptureProgrammer
	);
}

export function assertCaptureModeCursor(cursor: number) {
	if (!Number.isSafeInteger(cursor) || cursor < 0)
		throw new ProgrammerCaptureModeProtocolError(
			"Programmer capture mode event cursor must be a non-negative integer",
		);
}

function assertRevision(revision: number) {
	if (!Number.isSafeInteger(revision) || revision < 0)
		throw new ProgrammerCaptureModeProtocolError(
			"Programmer capture mode revision must be a non-negative integer",
		);
}

function assertBoolean(value: boolean, label: string) {
	if (typeof value !== "boolean")
		throw new ProgrammerCaptureModeProtocolError(
			`Programmer capture mode ${label} must be a boolean`,
		);
}
