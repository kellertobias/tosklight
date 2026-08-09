import type {
	EventServerMessage,
	PatchDelta,
	PatchErrorResponse,
	PatchFixtureProjection,
	PatchFixturesOutcome,
	PatchProfileRevisionProjection,
	PatchSnapshot,
} from "./generated/light-wire";
import { WireValidationError } from "./wireValidation";

type JsonObject = Record<string, unknown>;

const UUID_PATTERN =
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const EVENT_CLASSES = new Set([
	"transition",
	"projection",
	"command_outcome",
	"error",
	"safety",
	"telemetry",
]);
const EVENT_CAPABILITIES = new Set([
	"programmer",
	"playback",
	"show",
	"desk",
	"output",
	"system",
]);
const EVENT_DELIVERY_POLICIES = new Set(["lossless", "replaceable"]);
const EVENT_ACTION_SOURCES = new Set([
	"user_interface",
	"keyboard",
	"osc",
	"http",
	"extension",
	"matter",
	"cue",
	"timecode",
	"scheduler",
	"macro",
	"system",
]);

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}

function objectAt(value: unknown, path: string): JsonObject {
	if (typeof value !== "object" || value === null || Array.isArray(value))
		return invalid(path, "an object", value);
	return value as JsonObject;
}

function arrayAt(value: unknown, path: string): unknown[] {
	if (!Array.isArray(value)) return invalid(path, "an array", value);
	return value;
}

function stringAt(value: unknown, path: string): asserts value is string {
	if (typeof value !== "string") invalid(path, "a string", value);
}

function booleanAt(value: unknown, path: string): asserts value is boolean {
	if (typeof value !== "boolean") invalid(path, "a boolean", value);
}

function finiteNumberAt(value: unknown, path: string): asserts value is number {
	if (typeof value !== "number" || !Number.isFinite(value))
		invalid(path, "a finite number", value);
}

function unsignedIntegerAt(
	value: unknown,
	path: string,
): asserts value is number {
	if (!Number.isSafeInteger(value) || (value as number) < 0)
		invalid(path, "a non-negative safe integer", value);
}

function signedIntegerAt(
	value: unknown,
	path: string,
): asserts value is number {
	if (!Number.isSafeInteger(value)) invalid(path, "a safe integer", value);
}

function positiveIntegerAt(
	value: unknown,
	path: string,
): asserts value is number {
	unsignedIntegerAt(value, path);
	if (value === 0) invalid(path, "a positive safe integer", value);
}

function nullableUnsignedIntegerAt(value: unknown, path: string): void {
	if (value !== null) unsignedIntegerAt(value, path);
}

function uuidAt(value: unknown, path: string): asserts value is string {
	if (typeof value !== "string" || !UUID_PATTERN.test(value))
		invalid(path, "a hyphenated UUID", value);
}

function nullableUuidAt(value: unknown, path: string): void {
	if (value !== null) uuidAt(value, path);
}

function enumAt(
	value: unknown,
	path: string,
	values: ReadonlySet<string>,
): void {
	if (typeof value !== "string" || !values.has(value))
		invalid(path, "one of " + [...values].join(", "), value);
}

function vectorAt(value: unknown, path: string, integer: boolean): void {
	const vector = objectAt(value, path);
	for (const axis of ["x", "y", "z"] as const) {
		if (integer) signedIntegerAt(vector[axis], path + "." + axis);
		else finiteNumberAt(vector[axis], path + "." + axis);
	}
}

function splitAt(value: unknown, path: string): void {
	const split = objectAt(value, path);
	positiveIntegerAt(split.split, path + ".split");
	nullableUnsignedIntegerAt(split.universe, path + ".universe");
	nullableUnsignedIntegerAt(split.address, path + ".address");
	if ((split.universe === null) !== (split.address === null))
		invalid(path, "matching nullable universe and address fields", value);
}

function splitArrayAt(value: unknown, path: string): void {
	arrayAt(value, path).forEach((split, index) => {
		splitAt(split, path + "[" + index + "]");
	});
}

function directControlAt(value: unknown, path: string): void {
	if (value === null) return;
	const endpoint = objectAt(value, path);
	if (endpoint.protocol !== "citp")
		invalid(path + ".protocol", "citp", endpoint.protocol);
	stringAt(endpoint.ip_address, path + ".ip_address");
	positiveIntegerAt(endpoint.port, path + ".port");
}

function multipatchAt(value: unknown, path: string): void {
	const instance = objectAt(value, path);
	uuidAt(instance.id, path + ".id");
	stringAt(instance.name, path + ".name");
	splitArrayAt(instance.split_patches, path + ".split_patches");
	vectorAt(instance.location, path + ".location", true);
	vectorAt(instance.rotation, path + ".rotation", false);
	if (instance.invert_pan !== undefined)
		booleanAt(instance.invert_pan, path + ".invert_pan");
	if (instance.invert_tilt !== undefined)
		booleanAt(instance.invert_tilt, path + ".invert_tilt");
	angleAt(instance.bracket_angle, path + ".bracket_angle");
	nullableAngleAt(instance.shaper_angle, path + ".shaper_angle");
	installedAppearanceAt(
		instance.installed_appearance,
		path + ".installed_appearance",
	);
}

/** Degrees. Absent is the same as zero, so an older desk's payload still validates. */
function angleAt(value: unknown, path: string): void {
	if (value !== undefined) finiteNumberAt(value, path);
}

/** Degrees, or `null`/absent when no shaper or barn-door module is fitted. */
function nullableAngleAt(value: unknown, path: string): void {
	if (value !== undefined && value !== null) finiteNumberAt(value, path);
}

function installedAppearanceAt(value: unknown, path: string): void {
	const appearance = objectAt(value, path);
	const source = objectAt(appearance.light_source, path + ".light_source");
	enumAt(
		source.type,
		path + ".light_source.type",
		new Set([
			"profile_default",
			"tungsten",
			"halogen",
			"discharge",
			"led",
			"fluorescent",
			"arc",
			"other",
		]),
	);
	if (source.type === "other")
		stringAt(source.label, path + ".light_source.label");
	const cct = appearance.color_temperature_kelvin;
	if (cct !== null) {
		unsignedIntegerAt(cct, path + ".color_temperature_kelvin");
		if ((cct as number) < 1_000 || (cct as number) > 25_000)
			invalid(
				path + ".color_temperature_kelvin",
				"an integer from 1000 through 25000",
				cct,
			);
	}
	const gel = objectAt(appearance.gel, path + ".gel");
	enumAt(
		gel.type,
		path + ".gel.type",
		new Set(["open_white", "built_in", "custom"]),
	);
	if (gel.type === "built_in") {
		stringAt(gel.catalog_id, path + ".gel.catalog_id");
		stringAt(gel.entry_id, path + ".gel.entry_id");
		const fallback = objectAt(
			gel.embedded_fallback,
			path + ".gel.embedded_fallback",
		);
		stringAt(fallback.number, path + ".gel.embedded_fallback.number");
		stringAt(fallback.name, path + ".gel.embedded_fallback.name");
		canonicalSrgbAt(
			fallback.display_srgb,
			path + ".gel.embedded_fallback.display_srgb",
		);
		canonicalSrgbAt(
			fallback.visualizer_srgb,
			path + ".gel.embedded_fallback.visualizer_srgb",
		);
	} else if (gel.type === "custom") {
		stringAt(gel.name, path + ".gel.name");
		canonicalSrgbAt(gel.color_srgb, path + ".gel.color_srgb");
		if (gel.note !== null) stringAt(gel.note, path + ".gel.note");
	}
	const angles = arrayAt(
		appearance.shaper_angles_degrees,
		path + ".shaper_angles_degrees",
	);
	if (angles.length !== 4)
		invalid(path + ".shaper_angles_degrees", "exactly four angles", angles);
	angles.forEach((angle, index) => {
		finiteNumberAt(angle, `${path}.shaper_angles_degrees[${index}]`);
		if ((angle as number) < -180 || (angle as number) >= 180)
			invalid(
				`${path}.shaper_angles_degrees[${index}]`,
				"degrees within [-180, 180)",
				angle,
			);
	});
}

function canonicalSrgbAt(value: unknown, path: string): void {
	if (typeof value !== "string" || !/^#[0-9A-F]{6}$/.test(value))
		invalid(path, "canonical #RRGGBB sRGB", value);
}

function logicalHeadAt(value: unknown, path: string): void {
	const head = objectAt(value, path);
	nullableUuidAt(head.profile_head_id, path + ".profile_head_id");
	unsignedIntegerAt(head.head_index, path + ".head_index");
	uuidAt(head.fixture_id, path + ".fixture_id");
}

function highlightOverrideAt(value: unknown, path: string): void {
	const override = objectAt(value, path);
	uuidAt(override.channel_id, path + ".channel_id");
	unsignedIntegerAt(override.raw_value, path + ".raw_value");
}

function fixtureAt(
	value: unknown,
	path: string,
): asserts value is PatchFixtureProjection {
	const fixture = objectAt(value, path);
	uuidAt(fixture.fixture_id, path + ".fixture_id");
	unsignedIntegerAt(fixture.fixture_revision, path + ".fixture_revision");
	nullableUnsignedIntegerAt(fixture.fixture_number, path + ".fixture_number");
	nullableUnsignedIntegerAt(
		fixture.virtual_fixture_number,
		path + ".virtual_fixture_number",
	);
	stringAt(fixture.name, path + ".name");
	uuidAt(fixture.profile_id, path + ".profile_id");
	unsignedIntegerAt(fixture.profile_revision, path + ".profile_revision");
	uuidAt(fixture.mode_id, path + ".mode_id");
	splitArrayAt(fixture.split_patches, path + ".split_patches");
	stringAt(fixture.layer_id, path + ".layer_id");
	directControlAt(fixture.direct_control, path + ".direct_control");
	vectorAt(fixture.location, path + ".location", true);
	vectorAt(fixture.rotation, path + ".rotation", false);
	arrayAt(fixture.logical_heads, path + ".logical_heads").forEach(
		(head, index) => {
			logicalHeadAt(head, path + ".logical_heads[" + index + "]");
		},
	);
	arrayAt(fixture.multipatch, path + ".multipatch").forEach(
		(instance, index) => {
			multipatchAt(instance, path + ".multipatch[" + index + "]");
		},
	);
	if (fixture.group_masters_enabled !== undefined)
		booleanAt(fixture.group_masters_enabled, path + ".group_masters_enabled");
	if (fixture.grand_master_enabled !== undefined)
		booleanAt(fixture.grand_master_enabled, path + ".grand_master_enabled");
	if (fixture.invert_pan !== undefined)
		booleanAt(fixture.invert_pan, path + ".invert_pan");
	if (fixture.invert_tilt !== undefined)
		booleanAt(fixture.invert_tilt, path + ".invert_tilt");
	angleAt(fixture.bracket_angle, path + ".bracket_angle");
	nullableAngleAt(fixture.shaper_angle, path + ".shaper_angle");
	installedAppearanceAt(
		fixture.installed_appearance,
		path + ".installed_appearance",
	);
	booleanAt(fixture.move_in_black_enabled, path + ".move_in_black_enabled");
	unsignedIntegerAt(
		fixture.move_in_black_delay_millis,
		path + ".move_in_black_delay_millis",
	);
	arrayAt(fixture.highlight_overrides, path + ".highlight_overrides").forEach(
		(override, index) => {
			highlightOverrideAt(
				override,
				path + ".highlight_overrides[" + index + "]",
			);
		},
	);
}

function profileAt(
	value: unknown,
	path: string,
): asserts value is PatchProfileRevisionProjection {
	const profile = objectAt(value, path);
	uuidAt(profile.profile_id, path + ".profile_id");
	unsignedIntegerAt(profile.profile_revision, path + ".profile_revision");
	stringAt(profile.content_digest, path + ".content_digest");
	stringAt(profile.manufacturer, path + ".manufacturer");
	stringAt(profile.name, path + ".name");
	stringAt(profile.fixture_type, path + ".fixture_type");
	if (profile.patch_policy !== "dmx" && profile.patch_policy !== "visual_only")
		invalid(path + ".patch_policy", "dmx or visual_only", profile.patch_policy);
	arrayAt(profile.referenced_modes, path + ".referenced_modes").forEach(
		(modeValue, modeIndex) => {
			const modePath = path + ".referenced_modes[" + modeIndex + "]";
			const mode = objectAt(modeValue, modePath);
			uuidAt(mode.mode_id, modePath + ".mode_id");
			stringAt(mode.name, modePath + ".name");
			arrayAt(mode.splits, modePath + ".splits").forEach(
				(splitValue, splitIndex) => {
					const splitPath = modePath + ".splits[" + splitIndex + "]";
					const split = objectAt(splitValue, splitPath);
					positiveIntegerAt(split.split, splitPath + ".split");
					unsignedIntegerAt(split.footprint, splitPath + ".footprint");
				},
			);
		},
	);
}

function deltaAt(value: unknown, path: string): asserts value is PatchDelta {
	const delta = objectAt(value, path);
	uuidAt(delta.show_id, path + ".show_id");
	unsignedIntegerAt(delta.show_revision, path + ".show_revision");
	unsignedIntegerAt(delta.patch_revision, path + ".patch_revision");
	if (delta.event_sequence !== undefined && delta.event_sequence !== null)
		unsignedIntegerAt(delta.event_sequence, path + ".event_sequence");
	arrayAt(delta.fixtures, path + ".fixtures").forEach((fixture, index) => {
		fixtureAt(fixture, path + ".fixtures[" + index + "]");
	});
	arrayAt(delta.removed_fixture_ids, path + ".removed_fixture_ids").forEach(
		(id, index) => {
			uuidAt(id, path + ".removed_fixture_ids[" + index + "]");
		},
	);
	arrayAt(delta.profile_revisions, path + ".profile_revisions").forEach(
		(profile, index) => {
			profileAt(profile, path + ".profile_revisions[" + index + "]");
		},
	);
}

function cursorAt(value: unknown, path: string): void {
	const cursor = objectAt(value, path);
	unsignedIntegerAt(cursor.sequence, path + ".sequence");
}

export function validatePatchSnapshot(value: unknown): PatchSnapshot {
	const snapshot = objectAt(value, "$");
	uuidAt(snapshot.show_id, "$.show_id");
	unsignedIntegerAt(snapshot.show_revision, "$.show_revision");
	unsignedIntegerAt(snapshot.patch_revision, "$.patch_revision");
	cursorAt(snapshot.cursor, "$.cursor");
	arrayAt(snapshot.fixtures, "$.fixtures").forEach((fixture, index) => {
		fixtureAt(fixture, "$.fixtures[" + index + "]");
	});
	arrayAt(snapshot.profile_revisions, "$.profile_revisions").forEach(
		(profile, index) => {
			profileAt(profile, "$.profile_revisions[" + index + "]");
		},
	);
	return value as PatchSnapshot;
}

export function validatePatchFixturesOutcome(
	value: unknown,
): PatchFixturesOutcome {
	const outcome = objectAt(value, "$");
	stringAt(outcome.request_id, "$.request_id");
	booleanAt(outcome.replayed, "$.replayed");
	booleanAt(outcome.changed, "$.changed");
	deltaAt(value, "$");
	if (outcome.changed !== (outcome.event_sequence != null))
		invalid(
			"$.event_sequence",
			outcome.changed
				? "an event sequence for a changed outcome"
				: "null or absent for an unchanged outcome",
			outcome.event_sequence,
		);
	return value as PatchFixturesOutcome;
}

export function validatePatchErrorResponse(value: unknown): PatchErrorResponse {
	const error = objectAt(value, "$");
	stringAt(error.error, "$.error");
	if (error.current_revision !== undefined && error.current_revision !== null)
		unsignedIntegerAt(error.current_revision, "$.current_revision");
	booleanAt(error.retryable, "$.retryable");
	return value as PatchErrorResponse;
}

function eventSourceAt(value: unknown, path: string): void {
	const source = objectAt(value, path);
	if (source.kind === "runtime") return;
	if (source.kind !== "action")
		invalid(path + ".kind", "runtime or action", source.kind);
	enumAt(source.source, path + ".source", EVENT_ACTION_SOURCES);
}

function patchEventAt(value: unknown, path: string): void {
	const envelope = objectAt(value, path);
	unsignedIntegerAt(envelope.sequence, path + ".sequence");
	stringAt(envelope.occurred_at, path + ".occurred_at");
	if (envelope.desk_id !== null) uuidAt(envelope.desk_id, path + ".desk_id");
	enumAt(envelope.class, path + ".class", EVENT_CLASSES);
	if (envelope.object !== null) {
		const eventObject = objectAt(envelope.object, path + ".object");
		enumAt(
			eventObject.capability,
			path + ".object.capability",
			EVENT_CAPABILITIES,
		);
		stringAt(eventObject.id, path + ".object.id");
	}
	eventSourceAt(envelope.source, path + ".source");
	if (envelope.correlation_id !== null)
		uuidAt(envelope.correlation_id, path + ".correlation_id");
	enumAt(envelope.delivery, path + ".delivery", EVENT_DELIVERY_POLICIES);
	const payload = objectAt(envelope.payload, path + ".payload");
	if (payload.type !== "show_patch_changed")
		invalid(path + ".payload.type", "show_patch_changed", payload.type);
	deltaAt(payload.delta, path + ".payload.delta");
	const delta = payload.delta as PatchDelta;
	if (delta.event_sequence !== envelope.sequence)
		invalid(
			path + ".payload.delta.event_sequence",
			"the enclosing event sequence",
			delta.event_sequence,
		);
}

export function validatePatchEventServerMessage(
	value: unknown,
): EventServerMessage {
	const message = objectAt(value, "$");
	switch (message.type) {
		case "ready":
		case "repaired":
			cursorAt(message.cursor, "$.cursor");
			break;
		case "gap": {
			const gap = objectAt(message.gap, "$.gap");
			unsignedIntegerAt(gap.after_sequence, "$.gap.after_sequence");
			unsignedIntegerAt(gap.oldest_available, "$.gap.oldest_available");
			unsignedIntegerAt(gap.latest_sequence, "$.gap.latest_sequence");
			break;
		}
		case "event":
			patchEventAt(message.event, "$.event");
			break;
		case "error":
			stringAt(message.error, "$.error");
			break;
		default:
			invalid("$.type", "ready, event, gap, repaired, or error", message.type);
	}
	return value as EventServerMessage;
}
