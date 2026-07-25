import type {
	ProgrammerLifecycleChange,
	ProgrammerLifecycleProjection,
	ProgrammerLifecycleRow,
	ProgrammerLifecycleSessionProjection,
} from "./contracts";
import { ProgrammerLifecycleProtocolError } from "./transport";

export function canonicalLifecycleProjection(
	projection: ProgrammerLifecycleProjection,
): ProgrammerLifecycleProjection {
	const programmers = projection.programmers.map(canonicalRow);
	return lifecycleProjectionFromCanonicalRows(projection.revision, programmers);
}

/** Assemble a projection from already canonical rows without cloning unchanged users. */
export function lifecycleProjectionFromCanonicalRows(
	revision: number,
	programmers: readonly ProgrammerLifecycleRow[],
): ProgrammerLifecycleProjection {
	assertNonNegativeInteger(revision, "revision");
	const ordered = [...programmers];
	assertUnique(ordered, (row) => row.programmerId, "Programmer ID");
	assertUnique(ordered, (row) => row.userId, "user ID");
	ordered.sort(compareRows);
	return Object.freeze({
		revision,
		programmers: Object.freeze(ordered),
	});
}

export function canonicalLifecycleChange(
	change: ProgrammerLifecycleChange,
): ProgrammerLifecycleChange {
	assertNonNegativeInteger(change.revision, "revision");
	if (change.delta.type === "upsert")
		return Object.freeze({
			revision: change.revision,
			delta: Object.freeze({
				type: "upsert",
				programmer: canonicalRow(change.delta.programmer),
			}),
		});
	assertIdentifier(change.delta.programmerId, "removed Programmer ID");
	return Object.freeze({
		revision: change.revision,
		delta: Object.freeze({
			type: "remove",
			programmerId: change.delta.programmerId,
		}),
	});
}

export function assertLifecycleCursor(cursor: number) {
	assertNonNegativeInteger(cursor, "event cursor");
}

function canonicalRow(row: ProgrammerLifecycleRow): ProgrammerLifecycleRow {
	assertIdentifier(row.programmerId, "Programmer ID");
	assertIdentifier(row.userId, "user ID");
	if (typeof row.connected !== "boolean")
		throw protocolError("connected must be a boolean");
	if (typeof row.preloadActive !== "boolean")
		throw protocolError("Preload active must be a boolean");
	assertNonNegativeInteger(row.selectedFixtureCount, "selected fixture count");
	assertNonNegativeInteger(row.normalValueCount, "normal value count");
	const sessions = row.sessions.map(canonicalSession);
	assertUnique(sessions, (session) => session.sessionId, "session ID");
	sessions.sort(compareSessions);
	return Object.freeze({ ...row, sessions: Object.freeze(sessions) });
}

function canonicalSession(
	session: ProgrammerLifecycleSessionProjection,
): ProgrammerLifecycleSessionProjection {
	assertIdentifier(session.sessionId, "session ID");
	return Object.freeze({ ...session });
}

function assertIdentifier(value: string, label: string) {
	if (typeof value !== "string" || value.length === 0)
		throw protocolError(`${label} must not be empty`);
}

function assertNonNegativeInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw protocolError(`${label} must be a non-negative integer`);
}

function assertUnique<T>(
	values: readonly T[],
	key: (value: T) => string,
	label: string,
) {
	const seen = new Set<string>();
	for (const value of values) {
		const identity = key(value);
		if (seen.has(identity)) throw protocolError(`duplicate ${label}`);
		seen.add(identity);
	}
}

function compareRows(
	left: ProgrammerLifecycleRow,
	right: ProgrammerLifecycleRow,
) {
	return (
		left.userId.localeCompare(right.userId) ||
		left.programmerId.localeCompare(right.programmerId)
	);
}

function compareSessions(
	left: ProgrammerLifecycleSessionProjection,
	right: ProgrammerLifecycleSessionProjection,
) {
	return left.sessionId.localeCompare(right.sessionId);
}

function protocolError(message: string) {
	return new ProgrammerLifecycleProtocolError(
		`Programmer lifecycle ${message}`,
	);
}
