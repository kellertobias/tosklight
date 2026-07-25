import type { ShowObject } from "../features/showObjects/contracts";
import {
	arrayAt,
	enumAt,
	integerAt,
	numberAt,
	recordAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

export function decodeRecordedGroupBody(
	value: unknown,
	groupId: string,
): ShowObject<"group">["body"] {
	const body = recordAt(value, "$.group.body");
	if ("id" in body) {
		const id = printableAt(body.id, "$.group.body.id", 256, "Group ID");
		if (id !== groupId) invalid("$.group.body.id", `Group ID ${groupId}`, id);
	}
	if ("name" in body) stringValueAt(body.name, "$.group.body.name");
	if ("color" in body) nullableStringAt(body.color, "$.group.body.color");
	if ("icon" in body) nullableStringAt(body.icon, "$.group.body.icon");
	const fixtures =
		"fixtures" in body
			? arrayAt(body.fixtures, "$.group.body.fixtures").map((fixture, index) =>
					printableAt(
						fixture,
						`$.group.body.fixtures[${index}]`,
						256,
						"fixture ID",
					),
				)
			: [];
	if ("derived_from" in body)
		derivedGroupAt(body.derived_from, "$.group.body.derived_from");
	if ("frozen_from" in body)
		frozenGroupAt(body.frozen_from, "$.group.body.frozen_from");
	if ("programming" in body)
		programmingAt(body.programming, "$.group.body.programming");
	if ("master" in body) numberAt(body.master, "$.group.body.master");
	if ("playback_fader" in body)
		playbackFaderAt(body.playback_fader, "$.group.body.playback_fader");
	return { ...body, fixtures } as ShowObject<"group">["body"];
}

function derivedGroupAt(value: unknown, path: string) {
	if (value == null) return;
	const derived = recordAt(value, path);
	printableAt(
		derived.source_group_id,
		`${path}.source_group_id`,
		256,
		"source Group ID",
	);
	selectionRuleAt(derived.rule, `${path}.rule`);
}

function selectionRuleAt(value: unknown, path: string) {
	const rule = recordAt(value, path);
	const type = enumAt(rule.type, `${path}.type`, [
		"all",
		"odd",
		"even",
		"every_nth",
	]);
	if (type !== "every_nth") return;
	const n = integerAt(rule.n, `${path}.n`);
	if (n < 1) invalid(`${path}.n`, "positive integer", rule.n);
	integerAt(rule.offset, `${path}.offset`);
}

function frozenGroupAt(value: unknown, path: string) {
	if (value == null) return;
	const frozen = recordAt(value, path);
	printableAt(
		frozen.source_group_id,
		`${path}.source_group_id`,
		256,
		"source Group ID",
	);
	integerAt(frozen.source_revision, `${path}.source_revision`);
	const capturedAt = stringValueAt(frozen.captured_at, `${path}.captured_at`);
	if (Number.isNaN(Date.parse(capturedAt)))
		invalid(`${path}.captured_at`, "timestamp", capturedAt);
}

function programmingAt(value: unknown, path: string) {
	const programming = recordAt(value, path);
	for (const [attribute, rawValue] of Object.entries(programming))
		attributeValueAt(rawValue, `${path}.${attribute}`);
}

function attributeValueAt(value: unknown, path: string) {
	const attribute = recordAt(value, path);
	const kind = enumAt(attribute.kind, `${path}.kind`, [
		"normalized",
		"spread",
		"discrete",
		"color_xyz",
		"raw_dmx",
		"raw_dmx_exact",
	]);
	if (kind === "normalized") numberAt(attribute.value, `${path}.value`);
	else if (kind === "spread")
		arrayAt(attribute.value, `${path}.value`).forEach((item, index) =>
			numberAt(item, `${path}.value[${index}]`),
		);
	else if (kind === "discrete") stringValueAt(attribute.value, `${path}.value`);
	else if (kind === "color_xyz") xyzAt(attribute.value, `${path}.value`);
	else
		boundedIntegerAt(
			attribute.value,
			`${path}.value`,
			kind === "raw_dmx" ? 255 : 4_294_967_295,
		);
}

function xyzAt(value: unknown, path: string) {
	const xyz = recordAt(value, path);
	for (const coordinate of ["x", "y", "z"])
		numberAt(xyz[coordinate], `${path}.${coordinate}`);
}

function playbackFaderAt(value: unknown, path: string) {
	if (value == null) return;
	boundedIntegerAt(value, path, 255);
}

function boundedIntegerAt(value: unknown, path: string, maximum: number) {
	const decoded = integerAt(value, path);
	if (decoded > maximum) invalid(path, `integer at most ${maximum}`, value);
	return decoded;
}

function nullableStringAt(value: unknown, path: string) {
	if (value != null) stringValueAt(value, path);
}

function stringValueAt(value: unknown, path: string) {
	if (typeof value !== "string") invalid(path, "string", value);
	return value;
}

function printableAt(
	value: unknown,
	path: string,
	byteLimit: number,
	label: string,
) {
	if (
		typeof value !== "string" ||
		!value.trim() ||
		new TextEncoder().encode(value).length > byteLimit ||
		/\p{Cc}/u.test(value)
	)
		invalid(path, `1-${byteLimit} printable ${label} bytes`, value);
	return value;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
