import type {
	ProgrammingUpdateCueSource,
	ProgrammingUpdateMode,
	ProgrammingUpdateObjectIdentity,
	ProgrammingUpdateObjectKind,
	ProgrammingUpdateTarget,
	ProgrammingUpdateTargetFamily,
	ProgrammingUpdateTargetIdentity,
} from "./generated/light-wire";
import {
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	stringAt,
} from "./playbackWirePrimitives";
import { programmingUuidAt } from "./programmingWireProjection";
import { WireValidationError } from "./wireValidation";

const OBJECT_KINDS = ["cue_list", "preset", "group"] as const;
const CUE_MODES = [
	"existing_only",
	"existing_in_current_cue",
	"add_to_current_cue",
	"add_new",
] as const;
const EXISTING_MODES = ["update_existing", "add_new"] as const;
const TARGET_TYPES = ["cue", "preset", "group"] as const;
const NIL_UUID = "00000000-0000-0000-0000-000000000000";

export function requestIdAt(value: unknown, path: string) {
	if (typeof value !== "string")
		throw new WireValidationError(path, "printable request ID", value);
	const bytes = new TextEncoder().encode(value).byteLength;
	if (bytes < 1 || bytes > 128 || /\p{Cc}/u.test(value))
		throw new WireValidationError(path, "1-128 printable bytes", value);
	return value;
}

export function scopedUuidAt(value: unknown, path: string) {
	const uuid = programmingUuidAt(value, path);
	if (uuid.toLowerCase() === NIL_UUID)
		throw new WireValidationError(path, "non-nil UUID", value);
	return uuid;
}

export function boundedIntegerAt(
	value: unknown,
	path: string,
	maximum: number,
) {
	const integer = integerAt(value, path);
	if (integer > maximum)
		throw new WireValidationError(path, `integer at most ${maximum}`, value);
	return integer;
}

export function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string")
		throw new WireValidationError(path, "string", value);
	return value;
}

export function programmerRevisionAt(value: unknown, path: string) {
	if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value))
		throw new WireValidationError(path, "lowercase SHA-256 fingerprint", value);
	return value;
}

export function nullableAt<T>(
	value: unknown,
	path: string,
	decode: (value: unknown, path: string) => T,
) {
	return value == null ? null : decode(value, path);
}

export function decodeUpdateMode(
	value: unknown,
	path: string,
): ProgrammingUpdateMode {
	const mode = exactRecordAt(value, path, ["target_type", "mode"]);
	const targetType = enumAt(mode.target_type, `${path}.target_type`, [
		"cue",
		"existing_content",
	]);
	if (targetType === "cue") enumAt(mode.mode, `${path}.mode`, CUE_MODES);
	else enumAt(mode.mode, `${path}.mode`, EXISTING_MODES);
	return value as ProgrammingUpdateMode;
}

export function decodeUpdateTarget(
	value: unknown,
	path: string,
): ProgrammingUpdateTarget {
	const target = exactRecordAt(value, path, targetFields(value, path));
	const type = enumAt(target.type, `${path}.type`, TARGET_TYPES);
	if (type === "cue") decodeCueTarget(target, path);
	else boundedStringAt(target.object_id, `${path}.object_id`, 256);
	return value as ProgrammingUpdateTarget;
}

export function decodeTargetFamily(
	value: unknown,
	path: string,
): ProgrammingUpdateTargetFamily {
	const family = exactRecordAt(value, path, ["type"]);
	enumAt(family.type, `${path}.type`, TARGET_TYPES);
	return value as ProgrammingUpdateTargetFamily;
}

export function decodeTargetIdentity(
	value: unknown,
	path: string,
): ProgrammingUpdateTargetIdentity {
	const target = exactRecordAt(
		value,
		path,
		optionalFields(
			value,
			path,
			["family", "object_id", "name"],
			["playback_number", "cue"],
		),
	);
	const family = decodeTargetFamily(target.family, `${path}.family`);
	if (family.type === "cue")
		scopedUuidAt(target.object_id, `${path}.object_id`);
	else boundedStringAt(target.object_id, `${path}.object_id`, 256);
	plainStringAt(target.name, `${path}.name`);
	if ("playback_number" in target)
		nullableAt(
			target.playback_number,
			`${path}.playback_number`,
			(item, itemPath) => boundedIntegerAt(item, itemPath, 65_535),
		);
	if ("cue" in target) nullableAt(target.cue, `${path}.cue`, decodeCueIdentity);
	assertIdentityShape(family, target, path);
	return value as ProgrammingUpdateTargetIdentity;
}

export function decodeObjectIdentity(
	value: unknown,
	path: string,
): ProgrammingUpdateObjectIdentity {
	const object = exactRecordAt(value, path, [
		"kind",
		"object_id",
		"object_revision",
	]);
	enumAt(object.kind, `${path}.kind`, OBJECT_KINDS);
	boundedStringAt(object.object_id, `${path}.object_id`, 256);
	integerAt(object.object_revision, `${path}.object_revision`);
	return value as ProgrammingUpdateObjectIdentity;
}

export function decodeCueSource(
	value: unknown,
	path: string,
): ProgrammingUpdateCueSource {
	const source = exactRecordAt(value, path, [
		"cue_id",
		"cue_number",
		"cue_index",
	]);
	scopedUuidAt(source.cue_id, `${path}.cue_id`);
	numberAt(source.cue_number, `${path}.cue_number`);
	integerAt(source.cue_index, `${path}.cue_index`);
	return value as ProgrammingUpdateCueSource;
}

export function assertModeMatchesTarget(
	mode: ProgrammingUpdateMode,
	target: ProgrammingUpdateTarget | ProgrammingUpdateTargetIdentity,
	path: string,
) {
	const family = "type" in target ? target.type : target.family.type;
	const matches =
		(family === "cue" && mode.target_type === "cue") ||
		(family !== "cue" && mode.target_type === "existing_content");
	if (!matches)
		throw new WireValidationError(path, `mode for ${family} target`, mode);
}

export function assertObjectMatchesTarget(
	object: ProgrammingUpdateObjectIdentity,
	target: ProgrammingUpdateTarget | ProgrammingUpdateTargetIdentity,
	path: string,
) {
	const family = "type" in target ? target.type : target.family.type;
	const objectId = targetObjectId(target);
	const expectedKind = kindForFamily(family);
	const wrongIdentity = family !== "cue" && object.object_id !== objectId;
	if (object.kind !== expectedKind || wrongIdentity)
		throw new WireValidationError(
			path,
			`${expectedKind} object ${objectId}`,
			object,
		);
}

export function assertIdentityMatchesRequest(
	identity: ProgrammingUpdateTargetIdentity,
	target: ProgrammingUpdateTarget,
	path: string,
) {
	const sameObjectId =
		target.type === "cue"
			? identity.object_id.toLowerCase() === target.cue_list_id.toLowerCase()
			: identity.object_id === target.object_id;
	if (identity.family.type !== target.type || !sameObjectId)
		throw new WireValidationError(path, "requested Update target", identity);
	if (target.type !== "cue") return;
	if (
		target.playback_number != null &&
		identity.playback_number !== target.playback_number
	)
		throw new WireValidationError(path, "requested playback context", identity);
	if (
		target.cue_id != null &&
		identity.cue?.id.toLowerCase() !== target.cue_id.toLowerCase()
	)
		throw new WireValidationError(path, "requested Cue identity", identity);
	if (target.cue_number != null && identity.cue?.number !== target.cue_number)
		throw new WireValidationError(path, "requested Cue number", identity);
}

export function assertSameMode(
	actual: ProgrammingUpdateMode,
	expected: ProgrammingUpdateMode,
	path: string,
) {
	if (
		actual.target_type !== expected.target_type ||
		actual.mode !== expected.mode
	)
		throw new WireValidationError(path, "requested Update mode", actual);
}

export function targetObjectId(
	target: ProgrammingUpdateTarget | ProgrammingUpdateTargetIdentity,
) {
	if ("family" in target) return target.object_id;
	return target.type === "cue" ? target.cue_list_id : target.object_id;
}

export function kindForFamily(
	family: ProgrammingUpdateTargetFamily["type"],
): ProgrammingUpdateObjectKind {
	return family === "cue" ? "cue_list" : family;
}

function decodeCueTarget(target: Record<string, unknown>, path: string) {
	scopedUuidAt(target.cue_list_id, `${path}.cue_list_id`);
	if ("playback_number" in target)
		nullableAt(
			target.playback_number,
			`${path}.playback_number`,
			(item, itemPath) => boundedIntegerAt(item, itemPath, 65_535),
		);
	if ("cue_id" in target)
		nullableAt(target.cue_id, `${path}.cue_id`, scopedUuidAt);
	if ("cue_number" in target)
		nullableAt(target.cue_number, `${path}.cue_number`, numberAt);
	if (typeof target.validate_active_context !== "boolean")
		throw new WireValidationError(
			`${path}.validate_active_context`,
			"boolean",
			target.validate_active_context,
		);
}

function decodeCueIdentity(value: unknown, path: string) {
	const cue = exactRecordAt(value, path, ["id", "number"]);
	scopedUuidAt(cue.id, `${path}.id`);
	numberAt(cue.number, `${path}.number`);
	return value;
}

function assertIdentityShape(
	family: ProgrammingUpdateTargetFamily,
	target: Record<string, unknown>,
	path: string,
) {
	if (family.type === "cue") return;
	if (target.playback_number != null || target.cue != null)
		throw new WireValidationError(
			path,
			`${family.type} identity without Cue context`,
			target,
		);
}

function targetFields(value: unknown, path: string) {
	const record = exactRecordAt(value, path, [
		"type",
		"cue_list_id",
		"playback_number",
		"cue_id",
		"cue_number",
		"validate_active_context",
		"object_id",
	]);
	const type = enumAt(record.type, `${path}.type`, TARGET_TYPES);
	return type === "cue"
		? optionalFields(
				value,
				path,
				["type", "cue_list_id", "validate_active_context"],
				["playback_number", "cue_id", "cue_number"],
			)
		: ["type", "object_id"];
}

function optionalFields(
	value: unknown,
	path: string,
	required: string[],
	optional: string[],
) {
	const record = exactRecordAt(value, path, [...required, ...optional]);
	return [...required, ...optional.filter((key) => key in record)];
}

function boundedStringAt(value: unknown, path: string, maximum: number) {
	const decoded = stringAt(value, path);
	const bytes = new TextEncoder().encode(decoded).byteLength;
	if (bytes > maximum || /\p{Cc}/u.test(decoded))
		throw new WireValidationError(path, `1-${maximum} printable bytes`, value);
	return decoded;
}
