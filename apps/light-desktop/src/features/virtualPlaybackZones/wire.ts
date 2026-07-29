import {
	MAX_VIRTUAL_PLAYBACK_ZONE_NUMBER,
	MIN_VIRTUAL_PLAYBACK_ZONE_NUMBER,
	type VirtualPlaybackZone,
	type VirtualPlaybackZonesChange,
	type VirtualPlaybackZonesSaveOutcome,
	type VirtualPlaybackZonesScope,
	type VirtualPlaybackZonesSnapshot,
} from "./contracts";

const MAX_ZONE_ID_LENGTH = 128;
const MAX_ZONE_NAME_LENGTH = 80;
const UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export class VirtualPlaybackZonesProtocolError extends TypeError {
	constructor(
		readonly path: string,
		expected: string,
		actual: unknown,
	) {
		super(`${path}: expected ${expected}; received ${describe(actual)}`);
		this.name = "VirtualPlaybackZonesProtocolError";
	}
}

export function decodeVirtualPlaybackZonesSnapshot(
	value: unknown,
	expected: VirtualPlaybackZonesScope,
): VirtualPlaybackZonesSnapshot {
	validateScope(expected);
	const snapshot = exactObject(value, "$", ["show_id", "revision", "zones"]);
	const showId = uuid(snapshot.show_id, "$.show_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	return {
		showId,
		revision: revision(snapshot.revision, "$.revision"),
		zones: decodeZones(snapshot.zones, "$.zones"),
	};
}

export function decodeVirtualPlaybackZonesSaveOutcome(
	value: unknown,
	expected: VirtualPlaybackZonesScope,
	expectedRequestId: string,
): VirtualPlaybackZonesSaveOutcome {
	validateScope(expected);
	const outcome = exactObject(value, "$", [
		"show_id",
		"revision",
		"zones",
		"request_id",
		"replayed",
		"changed",
	]);
	const showId = uuid(outcome.show_id, "$.show_id");
	const requestId = nonEmptyString(outcome.request_id, "$.request_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	requireIdentity(requestId, expectedRequestId, "$.request_id");
	return {
		requestId,
		showId,
		revision: revision(outcome.revision, "$.revision"),
		zones: decodeZones(outcome.zones, "$.zones"),
		replayed: boolean(outcome.replayed, "$.replayed"),
		changed: boolean(outcome.changed, "$.changed"),
	};
}

export function encodeVirtualPlaybackZonesSaveRequest(
	requestId: string,
	expectedRevision: number,
	zones: readonly VirtualPlaybackZone[],
) {
	return {
		request_id: nonEmptyString(requestId, "$.request_id"),
		expected_revision: revision(expectedRevision, "$.expected_revision"),
		zones: decodeZones(
			zones.map((zone) => ({
				id: zone.id,
				name: zone.name,
				playback_numbers: zone.playbackNumbers,
			})),
			"$.zones",
		).map((zone) => ({
			id: zone.id,
			name: zone.name,
			playback_numbers: [...zone.playbackNumbers],
		})),
	};
}

export function decodeVirtualPlaybackZonesEvent(
	value: unknown,
): VirtualPlaybackZonesChange | "gap" | "ready" | "error" {
	const message = object(value, "$");
	const type = nonEmptyString(message.type, "$.type");
	if (type === "ready" || type === "repaired") return "ready";
	if (type === "gap") return "gap";
	if (type === "error") return "error";
	if (type !== "event")
		invalid("$.type", "ready, repaired, gap, error, or event", type);
	const event = object(message.event, "$.event");
	const payload = object(event.payload, "$.event.payload");
	if (payload.type !== "virtual_playback_exclusion_zones_changed")
		invalid(
			"$.event.payload.type",
			"virtual_playback_exclusion_zones_changed",
			payload.type,
		);
	const change = exactObject(payload.change, "$.event.payload.change", [
		"show_id",
		"revision",
	]);
	return {
		showId: uuid(change.show_id, "$.event.payload.change.show_id"),
		revision: revision(change.revision, "$.event.payload.change.revision"),
	};
}

export function validateVirtualPlaybackZonesScope(
	scope: VirtualPlaybackZonesScope,
) {
	validateScope(scope);
}

function decodeZones(value: unknown, path: string): VirtualPlaybackZone[] {
	if (!Array.isArray(value)) invalid(path, "array", value);
	const ids = new Set<string>();
	return value.map((entry, index) => {
		const zone = decodeZone(entry, `${path}[${index}]`);
		if (ids.has(zone.id))
			invalid(`${path}[${index}].id`, "unique zone id", zone.id);
		ids.add(zone.id);
		return zone;
	});
}

function decodeZone(value: unknown, path: string): VirtualPlaybackZone {
	const zone = exactObject(value, path, ["id", "name", "playback_numbers"]);
	const id = boundedTrimmedString(zone.id, `${path}.id`, MAX_ZONE_ID_LENGTH);
	const name = boundedTrimmedString(
		zone.name,
		`${path}.name`,
		MAX_ZONE_NAME_LENGTH,
	);
	const playbackNumbers = decodePlaybackNumbers(
		zone.playback_numbers,
		`${path}.playback_numbers`,
	);
	return { id, name, playbackNumbers };
}

function decodePlaybackNumbers(value: unknown, path: string) {
	if (!Array.isArray(value)) invalid(path, "array", value);
	if (value.length < 2)
		invalid(path, "at least two unique Virtual Playback numbers", value);
	const numbers = value.map((number, index) =>
		boundedPlaybackNumber(number, `${path}[${index}]`),
	);
	if (new Set(numbers).size !== numbers.length)
		invalid(path, "unique Virtual Playback numbers", value);
	return numbers;
}

function validateScope(scope: VirtualPlaybackZonesScope) {
	uuid(scope.showId, "$.scope.showId");
}

function boundedPlaybackNumber(value: unknown, path: string) {
	if (
		!Number.isSafeInteger(value) ||
		(value as number) < MIN_VIRTUAL_PLAYBACK_ZONE_NUMBER ||
		(value as number) > MAX_VIRTUAL_PLAYBACK_ZONE_NUMBER
	)
		invalid(
			path,
			`integer between ${MIN_VIRTUAL_PLAYBACK_ZONE_NUMBER} and ${MAX_VIRTUAL_PLAYBACK_ZONE_NUMBER}`,
			value,
		);
	return value as number;
}

function revision(value: unknown, path: string) {
	if (!Number.isSafeInteger(value) || (value as number) < 0)
		invalid(path, "non-negative safe integer", value);
	return value as number;
}

function boundedTrimmedString(value: unknown, path: string, maximum: number) {
	const decoded = nonEmptyString(value, path);
	if (decoded !== decoded.trim() || decoded.length > maximum)
		invalid(path, `trimmed string containing 1-${maximum} characters`, value);
	return decoded;
}

function nonEmptyString(value: unknown, path: string) {
	if (typeof value !== "string" || value.length === 0)
		invalid(path, "non-empty string", value);
	return value as string;
}

function boolean(value: unknown, path: string) {
	if (typeof value !== "boolean") invalid(path, "boolean", value);
	return value;
}

function uuid(value: unknown, path: string) {
	const decoded = nonEmptyString(value, path);
	if (!UUID_PATTERN.test(decoded)) invalid(path, "hyphenated UUID", value);
	return decoded;
}

function exactObject(value: unknown, path: string, keys: readonly string[]) {
	const decoded = object(value, path);
	for (const key of keys) {
		if (!(key in decoded))
			invalid(`${path}.${key}`, "declared wire field", undefined);
	}
	return decoded;
}

function object(value: unknown, path: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value))
		invalid(path, "object", value);
	return value as Record<string, unknown>;
}

function requireIdentity(actual: string, expected: string, path: string) {
	if (actual !== expected) invalid(path, expected, actual);
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new VirtualPlaybackZonesProtocolError(path, expected, actual);
}

function describe(value: unknown) {
	if (value === null) return "null";
	if (Array.isArray(value)) return "array";
	return typeof value;
}
