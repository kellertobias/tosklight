import { assertPlaybackAddress } from "../../api/playbackAddress";
import type {
	ProgrammerPreloadPlaybackAction,
	ProgrammerPreloadPlaybackQueueEntry,
	ProgrammerPreloadPlaybackQueueProjection,
} from "./contracts";
import { ProgrammerPreloadPlaybackQueueProtocolError } from "./transport";

const ACTIONS = new Set<ProgrammerPreloadPlaybackAction>([
	"toggle",
	"go",
	"back",
	"off",
	"on",
	"temporary_on",
	"temporary_off",
]);
export function canonicalPreloadPlaybackQueueProjection(
	projection: ProgrammerPreloadPlaybackQueueProjection,
): ProgrammerPreloadPlaybackQueueProjection {
	assertNonNegativeInteger(projection.revision, "revision");
	return Object.freeze({
		revision: projection.revision,
		actions: Object.freeze(projection.actions.map(canonicalEntry)),
	});
}

export function assertPreloadPlaybackQueueCursor(cursor: number) {
	assertNonNegativeInteger(cursor, "event cursor");
}

function canonicalEntry(
	entry: ProgrammerPreloadPlaybackQueueEntry,
): ProgrammerPreloadPlaybackQueueEntry {
	try { assertPlaybackAddress(entry.address); } catch { protocolError("address is invalid"); }
	if (!ACTIONS.has(entry.action)) protocolError("action is not supported");
	assertIdentifier(entry.controlSource.surface_id, "surface ID");
	if (!Number.isSafeInteger(entry.controlSource.control_id) || entry.controlSource.control_id < 0 || entry.controlSource.control_id > 0xffffffff) protocolError("control ID is invalid");
	return Object.freeze({ ...entry, address: Object.freeze({...entry.address}), controlSource: Object.freeze({...entry.controlSource}) });
}

function assertIdentifier(value: string, label: string) {
	if (typeof value !== "string" || value.length === 0)
		protocolError(`${label} must not be empty`);
}

function assertNonNegativeInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		protocolError(`${label} must be a non-negative integer`);
}

function protocolError(message: string): never {
	throw new ProgrammerPreloadPlaybackQueueProtocolError(
		`Preload playback queue ${message}`,
	);
}
