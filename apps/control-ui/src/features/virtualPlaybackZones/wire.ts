import {
	MAX_PERSISTED_VIRTUAL_PLAYBACK_ZONE_SLOT,
	type VirtualPlaybackZone,
	type VirtualPlaybackZonesSaveOutcome,
	type VirtualPlaybackZonesScope,
	type VirtualPlaybackZonesSnapshot,
} from "./contracts";

const MAX_SURFACE_ID_LENGTH = 128;
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
	const snapshot = exactObject(value, "$", ["show_id", "desk_id", "surfaces"]);
	const showId = uuid(snapshot.show_id, "$.show_id");
	const deskId = uuid(snapshot.desk_id, "$.desk_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	requireIdentity(deskId, expected.deskId, "$.desk_id");
	return {
		showId,
		deskId,
		surfaces: decodeSurfaces(snapshot.surfaces, "$.surfaces"),
	};
}

export function decodeVirtualPlaybackZonesSaveOutcome(
	value: unknown,
	expected: VirtualPlaybackZonesScope,
	expectedSurfaceId: string,
): VirtualPlaybackZonesSaveOutcome {
	validateScope(expected);
	validateSurfaceId(expectedSurfaceId, "$.requested_surface_id");
	const outcome = exactObject(value, "$", [
		"show_id",
		"desk_id",
		"surface_id",
		"zones",
	]);
	const showId = uuid(outcome.show_id, "$.show_id");
	const deskId = uuid(outcome.desk_id, "$.desk_id");
	const surfaceId = validateSurfaceId(outcome.surface_id, "$.surface_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	requireIdentity(deskId, expected.deskId, "$.desk_id");
	requireIdentity(surfaceId, expectedSurfaceId, "$.surface_id");
	return {
		surfaceId,
		zones: decodeZones(outcome.zones, "$.zones"),
	};
}

export function encodeVirtualPlaybackZonesSaveRequest(
	zones: readonly VirtualPlaybackZone[],
) {
	return {
		zones: decodeZones(zones, "$.zones").map((zone) => ({
			id: zone.id,
			name: zone.name,
			slots: [...zone.slots],
		})),
	};
}

export function validateVirtualPlaybackZonesScope(
	scope: VirtualPlaybackZonesScope,
) {
	validateScope(scope);
}

export function validateVirtualPlaybackZoneSurfaceId(surfaceId: unknown) {
	return validateSurfaceId(surfaceId, "$.surface_id");
}

function decodeSurfaces(value: unknown, path: string) {
	const surfaces = object(value, path);
	return Object.fromEntries(
		Object.entries(surfaces).map(([surfaceId, zones]) => {
			validateSurfaceId(surfaceId, `${path}.${surfaceId}`);
			return [surfaceId, decodeZones(zones, `${path}.${surfaceId}`)];
		}),
	);
}

function decodeZones(value: unknown, path: string): VirtualPlaybackZone[] {
	if (!Array.isArray(value)) invalid(path, "array", value);
	const ids = new Set<string>();
	return value.map((entry, index) => {
		const zone = decodeZone(entry, `${path}[${index}]`);
		if (ids.has(zone.id)) invalid(`${path}[${index}].id`, "unique zone id", zone.id);
		ids.add(zone.id);
		return zone;
	});
}

function decodeZone(value: unknown, path: string): VirtualPlaybackZone {
	const zone = exactObject(value, path, ["id", "name", "slots"]);
	const id = boundedTrimmedString(zone.id, `${path}.id`, MAX_ZONE_ID_LENGTH);
	const name = boundedTrimmedString(
		zone.name,
		`${path}.name`,
		MAX_ZONE_NAME_LENGTH,
	);
	const slots = decodeSlots(zone.slots, `${path}.slots`);
	return { id, name, slots };
}

function decodeSlots(value: unknown, path: string) {
	if (!Array.isArray(value)) invalid(path, "array", value);
	if (value.length < 2) invalid(path, "at least two unique cells", value);
	const slots = value.map((slot, index) => boundedSlot(slot, `${path}[${index}]`));
	if (new Set(slots).size !== slots.length) invalid(path, "unique cells", value);
	return slots;
}

function validateScope(scope: VirtualPlaybackZonesScope) {
	uuid(scope.showId, "$.scope.showId");
	uuid(scope.deskId, "$.scope.deskId");
}

function validateSurfaceId(value: unknown, path: string) {
	return boundedTrimmedString(value, path, MAX_SURFACE_ID_LENGTH);
}

function boundedSlot(value: unknown, path: string) {
	if (
		!Number.isSafeInteger(value) ||
		(value as number) < 1 ||
		(value as number) > MAX_PERSISTED_VIRTUAL_PLAYBACK_ZONE_SLOT
	)
		invalid(
			path,
			`integer between 1 and ${MAX_PERSISTED_VIRTUAL_PLAYBACK_ZONE_SLOT}`,
			value,
		);
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

function uuid(value: unknown, path: string) {
	const decoded = nonEmptyString(value, path);
	if (!UUID_PATTERN.test(decoded)) invalid(path, "hyphenated UUID", value);
	return decoded;
}

function exactObject(value: unknown, path: string, keys: readonly string[]) {
	const decoded = object(value, path);
	const unexpected = Object.keys(decoded).find((key) => !keys.includes(key));
	if (unexpected)
		invalid(`${path}.${unexpected}`, "a declared wire field", decoded[unexpected]);
	for (const key of keys) {
		if (!(key in decoded)) invalid(`${path}.${key}`, "declared wire field", undefined);
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
