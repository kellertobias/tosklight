import type {
	ProgrammingUpdateAddress,
	ProgrammingUpdateItemOutcome,
	ProgrammingUpdateMode,
	ProgrammingUpdatePreview,
	ProgrammingUpdatePreviewItem,
	ProgrammingUpdatePreviewRequest,
	ProgrammingUpdatePreviewResponse,
	ProgrammingUpdateTarget,
	ProgrammingUpdateTargetEntry,
	ProgrammingUpdateTargetsRequest,
	ProgrammingUpdateTargetsResponse,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	assertIdentityMatchesRequest,
	assertModeMatchesTarget,
	assertObjectMatchesTarget,
	assertSameMode,
	decodeCueSource,
	decodeObjectIdentity,
	decodeTargetIdentity,
	decodeUpdateMode,
	decodeUpdateTarget,
	plainStringAt,
	programmerRevisionAt,
	requestIdAt,
	scopedUuidAt,
} from "./programmingUpdateWireShared";
import { WireValidationError } from "./wireValidation";

const FILTERS = ["eligible_for_update_existing", "show_all_active"] as const;
const IGNORE_REASONS = [
	"new_address",
	"not_in_current_cue",
	"not_in_active_tracked_state",
	"new_group_member",
] as const;

export function encodeProgrammingUpdatePreviewRequest(
	request: ProgrammingUpdatePreviewRequest,
) {
	decodePreviewRequest(request, "$request");
	return request;
}

export function encodeProgrammingUpdateTargetsRequest(
	request: ProgrammingUpdateTargetsRequest,
) {
	const body = exactRecordAt(request, "$request", ["request_id", "filter"]);
	requestIdAt(body.request_id, "$request.request_id");
	enumAt(body.filter, "$request.filter", FILTERS);
	return request;
}

export function decodeProgrammingUpdatePreviewResponse(
	value: unknown,
	expectedShowId: string,
	request: ProgrammingUpdatePreviewRequest,
): ProgrammingUpdatePreviewResponse {
	scopedUuidAt(expectedShowId, "$expected.show_id");
	decodePreviewRequest(request, "$request");
	const response = exactRecordAt(value, "$", [
		"request_id",
		"correlation_id",
		"show_id",
		"show_revision",
		"object",
		"programmer_revision",
		"preview",
	]);
	assertRequestId(response.request_id, request.request_id, "$.request_id");
	scopedUuidAt(response.correlation_id, "$.correlation_id");
	assertScopeUuid(response.show_id, expectedShowId, "$.show_id");
	integerAt(response.show_revision, "$.show_revision");
	const object = decodeObjectIdentity(response.object, "$.object");
	programmerRevisionAt(response.programmer_revision, "$.programmer_revision");
	const preview = decodePreview(response.preview, "$.preview");
	assertIdentityMatchesRequest(
		preview.target,
		request.target,
		"$.preview.target",
	);
	assertSameMode(preview.mode, request.mode, "$.preview.mode");
	assertObjectMatchesTarget(object, preview.target, "$.object");
	return value as ProgrammingUpdatePreviewResponse;
}

export function decodeProgrammingUpdateTargetsResponse(
	value: unknown,
	expectedShowId: string,
	request: ProgrammingUpdateTargetsRequest,
): ProgrammingUpdateTargetsResponse {
	scopedUuidAt(expectedShowId, "$expected.show_id");
	encodeProgrammingUpdateTargetsRequest(request);
	const response = exactRecordAt(value, "$", [
		"request_id",
		"correlation_id",
		"show_id",
		"show_revision",
		"targets",
	]);
	assertRequestId(response.request_id, request.request_id, "$.request_id");
	scopedUuidAt(response.correlation_id, "$.correlation_id");
	assertScopeUuid(response.show_id, expectedShowId, "$.show_id");
	integerAt(response.show_revision, "$.show_revision");
	const keys = new Set<string>();
	arrayAt(response.targets, "$.targets").forEach((item, index) => {
		const entry = decodeTargetEntry(item, `$.targets[${index}]`);
		const key = targetEntryKey(entry);
		if (keys.has(key))
			throw new WireValidationError("$.targets", "unique target contexts", key);
		keys.add(key);
	});
	return value as ProgrammingUpdateTargetsResponse;
}

function targetEntryKey(entry: ProgrammingUpdateTargetEntry) {
	const target = entry.request_target;
	if (target.type !== "cue") return `${target.type}:${entry.object.object_id}`;
	return [
		"cue",
		entry.object.object_id,
		target.playback_number ?? "",
		target.cue_id ?? "",
		target.cue_number ?? "",
	].join(":");
}

export function decodeProgrammingUpdatePreview(
	value: unknown,
	path = "$",
): ProgrammingUpdatePreview {
	return decodePreview(value, path);
}

function decodePreviewRequest(
	value: unknown,
	path: string,
): ProgrammingUpdatePreviewRequest {
	const request = exactRecordAt(value, path, ["request_id", "target", "mode"]);
	requestIdAt(request.request_id, `${path}.request_id`);
	const target = decodeUpdateTarget(request.target, `${path}.target`);
	const mode = decodeUpdateMode(request.mode, `${path}.mode`);
	assertModeMatchesTarget(mode, target, `${path}.mode`);
	return value as ProgrammingUpdatePreviewRequest;
}

function decodeTargetEntry(
	value: unknown,
	path: string,
): ProgrammingUpdateTargetEntry {
	const entry = exactRecordAt(value, path, [
		"request_target",
		"object",
		"programmer_revision",
		"active_or_referenced",
		"existing_preview",
		"add_new_preview",
	]);
	const requestTarget = decodeUpdateTarget(
		entry.request_target,
		`${path}.request_target`,
	);
	const object = decodeObjectIdentity(entry.object, `${path}.object`);
	assertObjectMatchesTarget(object, requestTarget, `${path}.object`);
	programmerRevisionAt(
		entry.programmer_revision,
		`${path}.programmer_revision`,
	);
	booleanAt(entry.active_or_referenced, `${path}.active_or_referenced`);
	const existing = decodePreview(
		entry.existing_preview,
		`${path}.existing_preview`,
	);
	const addNew = decodePreview(
		entry.add_new_preview,
		`${path}.add_new_preview`,
	);
	assertIdentityMatchesRequest(
		existing.target,
		requestTarget,
		`${path}.existing_preview.target`,
	);
	assertIdentityMatchesRequest(
		addNew.target,
		requestTarget,
		`${path}.add_new_preview.target`,
	);
	assertMenuModes(requestTarget, existing.mode, addNew.mode, path);
	return value as ProgrammingUpdateTargetEntry;
}

function decodePreview(value: unknown, path: string): ProgrammingUpdatePreview {
	const preview = exactRecordAt(value, path, ["target", "mode", "items"]);
	const target = decodeTargetIdentity(preview.target, `${path}.target`);
	const mode = decodeUpdateMode(preview.mode, `${path}.mode`);
	assertModeMatchesTarget(mode, target, `${path}.mode`);
	arrayAt(preview.items, `${path}.items`).forEach((item, index) => {
		decodePreviewItem(item, `${path}.items[${index}]`);
	});
	return value as ProgrammingUpdatePreview;
}

function decodePreviewItem(
	value: unknown,
	path: string,
): ProgrammingUpdatePreviewItem {
	const item = exactRecordAt(value, path, ["address", "outcome"]);
	decodeAddress(item.address, `${path}.address`);
	decodeItemOutcome(item.outcome, `${path}.outcome`);
	return value as ProgrammingUpdatePreviewItem;
}

function decodeAddress(value: unknown, path: string): ProgrammingUpdateAddress {
	const address = exactRecordAt(value, path, addressFields(value, path));
	const type = enumAt(address.type, `${path}.type`, [
		"fixture_attribute",
		"group_attribute",
		"group_membership",
	]);
	if (type === "fixture_attribute") {
		scopedUuidAt(address.fixture_id, `${path}.fixture_id`);
		stringAt(address.attribute, `${path}.attribute`);
	} else if (type === "group_attribute") {
		stringAt(address.group_id, `${path}.group_id`);
		stringAt(address.attribute, `${path}.attribute`);
	} else scopedUuidAt(address.fixture_id, `${path}.fixture_id`);
	return value as ProgrammingUpdateAddress;
}

function decodeItemOutcome(
	value: unknown,
	path: string,
): ProgrammingUpdateItemOutcome {
	const outcome = exactRecordAt(value, path, outcomeFields(value, path));
	const type = enumAt(outcome.outcome, `${path}.outcome`, [
		"change_at_source",
		"change_in_current_cue",
		"add_to_current_cue",
		"add_new_to_current_cue",
		"update_existing",
		"add_new",
		"unchanged",
		"ignored",
	]);
	if (type === "change_at_source")
		decodeCueSource(outcome.source, `${path}.source`);
	else if (
		type === "change_in_current_cue" ||
		type === "add_to_current_cue" ||
		type === "add_new_to_current_cue"
	)
		decodeCueSource(outcome.cue, `${path}.cue`);
	else if (type === "unchanged" && outcome.source != null)
		decodeCueSource(outcome.source, `${path}.source`);
	else if (type === "ignored")
		enumAt(outcome.reason, `${path}.reason`, IGNORE_REASONS);
	return value as ProgrammingUpdateItemOutcome;
}

function assertMenuModes(
	target: ProgrammingUpdateTarget,
	existing: ProgrammingUpdateMode,
	addNew: ProgrammingUpdateMode,
	path: string,
) {
	const expectedExisting =
		target.type === "cue" ? "existing_only" : "update_existing";
	if (existing.mode !== expectedExisting)
		throw new WireValidationError(
			`${path}.existing_preview.mode`,
			expectedExisting,
			existing,
		);
	if (addNew.mode !== "add_new")
		throw new WireValidationError(
			`${path}.add_new_preview.mode`,
			"add_new",
			addNew,
		);
}

function assertRequestId(value: unknown, expected: string, path: string) {
	const actual = requestIdAt(value, path);
	if (actual !== expected)
		throw new WireValidationError(path, `request ${expected}`, actual);
}

function assertScopeUuid(value: unknown, expected: string, path: string) {
	const actual = scopedUuidAt(value, path);
	if (actual.toLowerCase() !== expected.toLowerCase())
		throw new WireValidationError(path, `scope ${expected}`, actual);
}

function addressFields(value: unknown, path: string) {
	const address = exactRecordAt(value, path, [
		"type",
		"fixture_id",
		"group_id",
		"attribute",
	]);
	const type = enumAt(address.type, `${path}.type`, [
		"fixture_attribute",
		"group_attribute",
		"group_membership",
	]);
	if (type === "fixture_attribute") return ["type", "fixture_id", "attribute"];
	if (type === "group_attribute") return ["type", "group_id", "attribute"];
	return ["type", "fixture_id"];
}

function outcomeFields(value: unknown, path: string) {
	const outcome = exactRecordAt(value, path, [
		"outcome",
		"source",
		"cue",
		"reason",
	]);
	const type = plainStringAt(outcome.outcome, `${path}.outcome`);
	if (type === "change_at_source") return ["outcome", "source"];
	if (
		[
			"change_in_current_cue",
			"add_to_current_cue",
			"add_new_to_current_cue",
		].includes(type)
	)
		return ["outcome", "cue"];
	if (type === "unchanged")
		return "source" in outcome ? ["outcome", "source"] : ["outcome"];
	if (type === "ignored") return ["outcome", "reason"];
	return ["outcome"];
}
