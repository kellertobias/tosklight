import type { OutputRuntimeProjection } from "./contracts";
import { OutputRuntimeProtocolError } from "./transport";

export function canonicalOutputProjection(
	projection: OutputRuntimeProjection,
): OutputRuntimeProjection {
	if (!projection.showId) throw protocolError("projection is missing its Show");
	if (projection.identity !== "global_master")
		throw protocolError("projection identity must be global_master");
	assertOutputRevision(projection.revision);
	assertGrandMaster(projection.grandMaster);
	if (typeof projection.blackout !== "boolean")
		throw protocolError("blackout must be boolean");
	return Object.freeze({ ...projection });
}

export function sameOutputProjection(
	left: OutputRuntimeProjection,
	right: OutputRuntimeProjection,
) {
	return (
		left.showId.toLowerCase() === right.showId.toLowerCase() &&
		left.identity === right.identity &&
		left.revision === right.revision &&
		left.grandMaster === right.grandMaster &&
		left.blackout === right.blackout
	);
}

export function assertGrandMaster(value: number) {
	if (!Number.isFinite(value) || value < 0 || value > 1)
		throw protocolError(
			"Grand Master must be a finite number from 0 through 1",
		);
}

export function assertOutputRevision(value: number) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw protocolError("revision must be a non-negative safe integer");
}

export function assertOutputCursor(value: number) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw protocolError("event cursor must be a non-negative safe integer");
}

export function assertOutputMutation(
	grandMaster: number | undefined,
	blackout: boolean | undefined,
) {
	if (grandMaster === undefined && blackout === undefined)
		throw protocolError("at least one output value is required");
	if (grandMaster !== undefined) assertGrandMaster(grandMaster);
	if (blackout !== undefined && typeof blackout !== "boolean")
		throw protocolError("blackout must be boolean");
}

export function assertOutputRequestId(value: string) {
	const bytes = new TextEncoder().encode(value).length;
	if (!value || bytes > 128 || /\p{Cc}/u.test(value))
		throw protocolError("request ID must contain 1-128 printable bytes");
}

function protocolError(message: string) {
	return new OutputRuntimeProtocolError(`Output runtime ${message}`);
}
