import type {
	ProgrammerPreloadPlaybackAction,
	ProgrammerPreloadPlaybackQueueEntry,
	ProgrammerPreloadPlaybackQueueProjection,
	ProgrammerPreloadPlaybackSurface,
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
const SURFACES = new Set<ProgrammerPreloadPlaybackSurface>([
	"virtual",
	"physical",
	"osc",
	"matter",
]);

export function canonicalPreloadPlaybackQueueProjection(
	projection: ProgrammerPreloadPlaybackQueueProjection,
): ProgrammerPreloadPlaybackQueueProjection {
	assertIdentifier(projection.userId, "user ID");
	assertNonNegativeInteger(projection.revision, "revision");
	return Object.freeze({
		userId: projection.userId,
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
	assertPlaybackNumber(entry.playbackNumber);
	if (entry.page !== null) assertPage(entry.page);
	if (!ACTIONS.has(entry.action)) protocolError("action is not supported");
	if (!SURFACES.has(entry.surface)) protocolError("surface is not supported");
	return Object.freeze({ ...entry });
}

function assertPage(value: number) {
	if (!Number.isInteger(value) || value < 0 || value > 255)
		protocolError("page must be an unsigned 8-bit integer");
}

function assertPlaybackNumber(value: number) {
	if (!Number.isInteger(value) || value < 0 || value > 65_535)
		protocolError("playback number must be an unsigned 16-bit integer");
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
