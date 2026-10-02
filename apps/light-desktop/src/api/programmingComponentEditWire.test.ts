import { describe, expect, it } from "vitest";
import type { ProgrammingComponentEdit } from "./generated/light-wire";
import {
	encodeProgrammerValueIntentOperation,
	encodeProgrammingComponentEdits,
	MAX_PROGRAMMING_COMPONENT_EDITS,
} from "./programmingComponentEditWire";
import { WireValidationError } from "./wireValidation";

const POINT_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const CHANNEL_ID = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
const FUNCTION_ID = "ffffffff-ffff-4fff-8fff-ffffffffffff";
const NIL = "00000000-0000-0000-0000-000000000000";

const ANGLE_PAN_EDITS: ProgrammingComponentEdit[] = [
	{ kind: "activate_angles" },
	{
		kind: "scalar",
		component: { kind: "pan" },
		operation: { kind: "relative", value: -12.5 },
	},
	{
		kind: "scalar",
		component: { kind: "tilt" },
		operation: { kind: "set", value: { kind: "spread", value: [-30, 30] } },
	},
];

const TARGET_XYZ_EDITS: ProgrammingComponentEdit[] = [
	{ kind: "target", reference: { kind: "point", point_id: POINT_ID } },
	{
		kind: "scalar",
		component: { kind: "target_x" },
		operation: { kind: "set", value: { kind: "value", value: 1.5 } },
	},
	{
		kind: "scalar",
		component: { kind: "target_y" },
		operation: { kind: "relative", value: 0.25 },
	},
	{
		kind: "scalar",
		component: { kind: "target_z" },
		operation: { kind: "set", value: { kind: "value", value: -2 } },
	},
];

const WHITE_BLEND_UV_EDITS: ProgrammingComponentEdit[] = [
	{
		kind: "scalar",
		component: { kind: "color", component: "white_blend" },
		operation: { kind: "set", value: { kind: "value", value: 0.4 } },
	},
	{
		kind: "scalar",
		component: { kind: "color", component: "uv" },
		operation: { kind: "relative", value: 0.1 },
	},
];

const FOCUS_EDITS: ProgrammingComponentEdit[] = [
	{
		kind: "scalar",
		component: { kind: "focus" },
		operation: { kind: "set", value: { kind: "value", value: 0.6 } },
	},
];

const ZOOM_EDITS: ProgrammingComponentEdit[] = [
	{
		kind: "scalar",
		component: { kind: "zoom" },
		operation: { kind: "relative", value: -3 },
	},
];

const NATIVE_EDITS: ProgrammingComponentEdit[] = [
	{
		kind: "native",
		binding: { channel_id: CHANNEL_ID, function_id: FUNCTION_ID },
		operation: { kind: "spread", value: [0, 65_535] },
	},
	{ kind: "coordinates", xyz: { x: 0.3, y: 0.4, z: 0.2 } },
];

describe("semantic component edit wire", () => {
	it.each([
		["Angle activation plus Pan/Tilt", ANGLE_PAN_EDITS],
		["Target reference plus XYZ", TARGET_XYZ_EDITS],
		["White Blend and UV", WHITE_BLEND_UV_EDITS],
		["Focus", FOCUS_EDITS],
		["Zoom", ZOOM_EDITS],
		["native and coordinate edits", NATIVE_EDITS],
	])("round-trips %s in order without flattening", (_name, edits) => {
		const encoded = encodeProgrammerValueIntentOperation(
			{ type: "component_edits", edits },
			"$.operation",
		);
		expect(encoded).toEqual({ type: "component_edits", edits });
		// A fresh copy: later caller mutation cannot rewrite a queued request.
		expect(encoded).not.toBe(edits);
		if (encoded.type === "component_edits")
			encoded.edits.forEach((edit, index) => {
				expect(edit).not.toBe(edits[index]);
			});
		const wire = JSON.parse(JSON.stringify(encoded));
		expect(wire).toEqual({ type: "component_edits", edits });
		expect(encodeProgrammingComponentEdits(wire.edits, "$")).toEqual(edits);
	});

	it("keeps an empty edit list as a quiet, explicit no-op", () => {
		expect(
			encodeProgrammerValueIntentOperation(
				{ type: "component_edits", edits: [] },
				"$.operation",
			),
		).toEqual({ type: "component_edits", edits: [] });
	});

	it("leaves absolute and relative operations on their own branches", () => {
		const value = { kind: "normalized", value: 0.8 } as const;
		expect(
			encodeProgrammerValueIntentOperation(
				{ type: "absolute_set", value },
				"$.operation",
			),
		).toEqual({ type: "absolute_set", value });
		expect(
			encodeProgrammerValueIntentOperation(
				{ type: "relative_step", delta: -0.1 },
				"$.operation",
			),
		).toEqual({ type: "relative_step", delta: -0.1 });
	});

	const scalar = (component: unknown, operation: unknown = {
		kind: "relative",
		value: 1,
	}) => ({ kind: "scalar", component, operation });
	it.each<[string, unknown, RegExp]>([
		["unknown operation", { type: "whole_value", value: 1 }, /\$\.operation\.type/],
		["non-finite relative step", { type: "relative_step", delta: Number.NaN }, /\$\.operation\.delta/],
		["edits that are not a list", { type: "component_edits", edits: {} }, /\$\.operation\.edits/],
		["undeclared operation field", { type: "component_edits", edits: [], value: 1 }, /\$\.operation\.value/],
		["unknown edit kind", { type: "component_edits", edits: [{ kind: "whole" }] }, /edits\[0\]\.kind/],
		["undeclared edit field", { type: "component_edits", edits: [{ kind: "activate_angles", pan: 1 }] }, /edits\[0\]\.pan/],
		["unknown component", { type: "component_edits", edits: [scalar({ kind: "iris" })] }, /edits\[0\]\.component\.kind/],
		["scalar edit of a typed component", { type: "component_edits", edits: [scalar({ kind: "target_reference" })] }, /edits\[0\]\.component\.kind/],
		["scalar edit of a native binding", { type: "component_edits", edits: [scalar({ kind: "native_color", component: { channel_id: CHANNEL_ID, function_id: FUNCTION_ID } })] }, /edits\[0\]\.component\.kind/],
		["non-finite scalar step", { type: "component_edits", edits: [ANGLE_PAN_EDITS[0], scalar({ kind: "pan" }, { kind: "relative", value: Number.POSITIVE_INFINITY })] }, /edits\[1\]\.operation\.value/],
		["one-point scalar spread", { type: "component_edits", edits: [scalar({ kind: "focus" }, { kind: "set", value: { kind: "spread", value: [0.5] } })] }, /edits\[0\]\.operation\.value\.value/],
		["nil target point", { type: "component_edits", edits: [{ kind: "target", reference: { kind: "point", point_id: NIL } }] }, /edits\[0\]\.reference\.point_id/],
		["negative color coordinate", { type: "component_edits", edits: [{ kind: "coordinates", xyz: { x: -0.1, y: 0, z: 0 } }] }, /edits\[0\]\.xyz\.x/],
		["fractional native value", { type: "component_edits", edits: [{ kind: "native", binding: { channel_id: CHANNEL_ID, function_id: FUNCTION_ID }, operation: { kind: "set", value: 1.5 } }] }, /edits\[0\]\.operation\.value/],
		["one-point native spread", { type: "component_edits", edits: [{ kind: "native", binding: { channel_id: CHANNEL_ID, function_id: FUNCTION_ID }, operation: { kind: "spread", value: [1] } }] }, /edits\[0\]\.operation\.value/],
		["oversized native step", { type: "component_edits", edits: [{ kind: "native", binding: { channel_id: CHANNEL_ID, function_id: FUNCTION_ID }, operation: { kind: "relative", value: 2 ** 33 } }] }, /edits\[0\]\.operation\.value/],
		["invalid native binding", { type: "component_edits", edits: [{ kind: "native", binding: { channel_id: "red", function_id: FUNCTION_ID }, operation: { kind: "set", value: 1 } }] }, /edits\[0\]\.binding\.channel_id/],
		["too many edits", { type: "component_edits", edits: Array.from({ length: MAX_PROGRAMMING_COMPONENT_EDITS + 1 }, () => FOCUS_EDITS[0]) }, /\$\.operation\.edits/],
	])("rejects %s locally", (_name, operation, path) => {
		expect(() =>
			encodeProgrammerValueIntentOperation(operation as never, "$.operation"),
		).toThrow(WireValidationError);
		expect(() =>
			encodeProgrammerValueIntentOperation(operation as never, "$.operation"),
		).toThrow(path);
	});
});
