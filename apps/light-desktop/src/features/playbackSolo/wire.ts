import { decodeExactAddress } from "../../api/playbackWireProjection";
import { playbackAddressKey } from "../../api/playbackAddress";
import {
	type PlaybackSoloRegion,
	type PlaybackSoloChange,
	type PlaybackSoloSaveOutcome,
	type PlaybackSoloScope,
	type PlaybackSoloSnapshot,
} from "./contracts";

const MAX_ZONE_ID_LENGTH = 128;
const MAX_ZONE_NAME_LENGTH = 80;
const UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export class PlaybackSoloProtocolError extends TypeError {
	constructor(
		readonly path: string,
		expected: string,
		actual: unknown,
	) {
		super(`${path}: expected ${expected}; received ${describe(actual)}`);
		this.name = "PlaybackSoloProtocolError";
	}
}

export function decodePlaybackSoloSnapshot(
	value: unknown,
	expected: PlaybackSoloScope,
): PlaybackSoloSnapshot {
	validateScope(expected);
	const snapshot = exactObject(value, "$", ["show_id", "revision", "regions"]);
	const showId = uuid(snapshot.show_id, "$.show_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	return {
		showId,
		revision: revision(snapshot.revision, "$.revision"),
		regions: decodeZones(snapshot.regions, "$.regions"),
	};
}

export function decodePlaybackSoloSaveOutcome(
	value: unknown,
	expected: PlaybackSoloScope,
	expectedRequestId: string,
): PlaybackSoloSaveOutcome {
	validateScope(expected);
	const outcome = exactObject(value, "$", [
		"show_id",
		"revision",
		"regions",
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
		regions: decodeZones(outcome.regions, "$.regions"),
		replayed: boolean(outcome.replayed, "$.replayed"),
		changed: boolean(outcome.changed, "$.changed"),
	};
}

export function encodePlaybackSoloSaveRequest(
	requestId: string,
	expectedRevision: number,
	regions: readonly PlaybackSoloRegion[],
) {
	return {
		request_id: nonEmptyString(requestId, "$.request_id"),
		expected_revision: revision(expectedRevision, "$.expected_revision"),
		regions: decodeZones(
			regions.map((zone) => ({
				id: zone.id,
				name: zone.name,
				...(zone.color !== undefined ? { color: zone.color } : {}),
				members: zone.members,
			})),
			"$.regions",
		).map((zone) => ({
			id: zone.id,
			name: zone.name,
			...(zone.color !== undefined ? { color: zone.color } : {}),
			members: [...zone.members],
		})),
	};
}

export function decodePlaybackSoloEvent(
	value: unknown,
): PlaybackSoloChange | "gap" | "ready" | "error" {
	const message = object(value, "$");
	const type = nonEmptyString(message.type, "$.type");
	if (type === "ready" || type === "repaired") return "ready";
	if (type === "gap") return "gap";
	if (type === "error") return "error";
	if (type !== "event")
		invalid("$.type", "ready, repaired, gap, error, or event", type);
	const event = object(message.event, "$.event");
	const payload = object(event.payload, "$.event.payload");
	if (payload.type !== "playback_solo_regions_changed")
		invalid(
			"$.event.payload.type",
			"playback_solo_regions_changed",
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

export function validatePlaybackSoloScope(scope: PlaybackSoloScope) {
	validateScope(scope);
}

function decodeZones(value: unknown, path: string): PlaybackSoloRegion[] {
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

function decodeZone(value: unknown, path: string): PlaybackSoloRegion {
	const zone = exactObject(value, path, ["id", "name", "members"]);
	const id = boundedTrimmedString(zone.id, `${path}.id`, MAX_ZONE_ID_LENGTH);
	const name = boundedTrimmedString(
		zone.name,
		`${path}.name`,
		MAX_ZONE_NAME_LENGTH,
	);
	const members = decodePlaybackNumbers(zone.members, `${path}.members`);
	const color = zone.color;
	if (
		color !== undefined &&
		color !== null &&
		(typeof color !== "string" || !/^#[0-9a-f]{6}$/i.test(color))
	)
		invalid(`${path}.color`, "six-digit hexadecimal color or null", color);
	return {
		id,
		name,
		members,
		...(color !== undefined ? { color: color as string | null } : {}),
	};
}

function decodePlaybackNumbers(value: unknown, path: string) {
	if (!Array.isArray(value)) invalid(path, "array", value);
	if (value.length < 2)
		invalid(path, "at least two unique Playback addresses", value);
	const addresses = value.map((address, index) => {
		exactObject(address, `${path}[${index}]`, ["page", "position"]);
		try {
			return decodeExactAddress(address, `${path}[${index}]`);
		} catch {
			return invalid(
				`${path}[${index}]`,
				"Playback address page 1–99 position 1–1000",
				address,
			);
		}
	});
	if (new Set(addresses.map(playbackAddressKey)).size !== addresses.length)
		invalid(path, "unique Playback addresses", value);
	return addresses;
}

function validateScope(scope: PlaybackSoloScope) {
	uuid(scope.showId, "$.scope.showId");
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
	throw new PlaybackSoloProtocolError(path, expected, actual);
}

function describe(value: unknown) {
	if (value === null) return "null";
	if (Array.isArray(value)) return "array";
	return typeof value;
}
