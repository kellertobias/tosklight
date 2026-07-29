import {
	MAX_PERSISTED_VIRTUAL_PLAYBACK_ZONE_SLOT,
	type VirtualPlaybackExclusionSurface,
	type VirtualPlaybackSurfacePageMode,
	type VirtualPlaybackZone,
	type VirtualPlaybackZonesChange,
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
	const snapshot = exactObject(value, "$", ["show_id", "desks"]);
	const showId = uuid(snapshot.show_id, "$.show_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	return {
		showId,
		desks: decodeDesks(snapshot.desks, "$.desks"),
	};
}

export function decodeVirtualPlaybackZonesSaveOutcome(
	value: unknown,
	expected: VirtualPlaybackZonesScope,
	expectedSurfaceId: string,
	expectedRequestId: string,
): VirtualPlaybackZonesSaveOutcome {
	validateScope(expected);
	validateSurfaceId(expectedSurfaceId, "$.requested_surface_id");
	const outcome = exactObject(value, "$", [
		"show_id",
		"desk_id",
		"surface_id",
		"surface",
		"request_id",
		"replayed",
		"changed",
	]);
	const showId = uuid(outcome.show_id, "$.show_id");
	const deskId = uuid(outcome.desk_id, "$.desk_id");
	const surfaceId = validateSurfaceId(outcome.surface_id, "$.surface_id");
	const requestId = nonEmptyString(outcome.request_id, "$.request_id");
	requireIdentity(showId, expected.showId, "$.show_id");
	requireIdentity(deskId, expected.deskId, "$.desk_id");
	requireIdentity(surfaceId, expectedSurfaceId, "$.surface_id");
	requireIdentity(requestId, expectedRequestId, "$.request_id");
	return {
		requestId,
		showId,
		deskId,
		surfaceId,
		surface: decodeSurface(outcome.surface, "$.surface"),
		replayed: boolean(outcome.replayed, "$.replayed"),
		changed: boolean(outcome.changed, "$.changed"),
	};
}

export function encodeVirtualPlaybackZonesSaveRequest(
	requestId: string,
	expectedRevision: number,
	pageMode: VirtualPlaybackSurfacePageMode,
	zones: readonly VirtualPlaybackZone[],
) {
	return {
		request_id: nonEmptyString(requestId, "$.request_id"),
		expected_revision: revision(expectedRevision, "$.expected_revision"),
		page_mode: decodePageMode(pageMode, "$.page_mode"),
		zones: decodeZones(zones, "$.zones").map((zone) => ({
			id: zone.id,
			name: zone.name,
			slots: [...zone.slots],
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
		"desk_id",
		"surface_id",
	]);
	return {
		showId: uuid(change.show_id, "$.event.payload.change.show_id"),
		deskId: uuid(change.desk_id, "$.event.payload.change.desk_id"),
		surfaceId: validateSurfaceId(
			change.surface_id,
			"$.event.payload.change.surface_id",
		),
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
		Object.entries(surfaces).map(([surfaceId, surface]) => {
			validateSurfaceId(surfaceId, `${path}.${surfaceId}`);
			return [surfaceId, decodeSurface(surface, `${path}.${surfaceId}`)];
		}),
	);
}

function decodeSurface(
	value: unknown,
	path: string,
): VirtualPlaybackExclusionSurface {
	const surface = exactObject(value, path, ["revision", "page_mode", "zones"]);
	return {
		revision: revision(surface.revision, `${path}.revision`),
		pageMode: decodePageMode(surface.page_mode, `${path}.page_mode`),
		zones: decodeZones(surface.zones, `${path}.zones`),
	};
}

function decodePageMode(
	value: unknown,
	path: string,
): VirtualPlaybackSurfacePageMode {
	const mode = object(value, path);
	const type = nonEmptyString(mode.type, `${path}.type`);
	if (type === "follow_main") return { type };
	if (type !== "pinned") invalid(`${path}.type`, "follow_main or pinned", type);
	const page = mode.page;
	if (!Number.isSafeInteger(page) || (page as number) < 1 || (page as number) > 127)
		invalid(`${path}.page`, "integer between 1 and 127", page);
	return { type, page: page as number };
}

function decodeDesks(value: unknown, path: string) {
	const desks = object(value, path);
	return Object.fromEntries(
		Object.entries(desks).map(([deskId, surfaces]) => {
			uuid(deskId, `${path}.${deskId}`);
			return [deskId, decodeSurfaces(surfaces, `${path}.${deskId}`)];
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
