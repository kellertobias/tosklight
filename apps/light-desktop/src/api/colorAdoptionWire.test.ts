import { describe, expect, it } from "vitest";
import { decodeNativeColorPagesSnapshot, nativeColorPagesPath } from "./nativeColorPagesWire";
import {
	decodeProgrammerPreloadValuesActionOutcome,
	encodeProgrammerPreloadValuesActionRequest,
} from "./programmerPreloadValuesWire";
import {
	decodeProgrammerValuesActionOutcome,
	encodeProgrammerValuesActionRequest,
} from "./programmerValuesWire";
import { WireValidationError } from "./wireValidation";

const NATIVE = {
	type: "component_edits",
	edits: [
		{
			kind: "native",
			binding: {
				channel_id: "c1000000-0000-4000-8000-000000000000",
				function_id: "f1000000-0000-4000-8000-000000000000",
			},
			operation: { kind: "set", value: 4_294_967_295 },
		},
	],
} as const;

describe("TL-554 Direct Color wire (both lanes)", () => {
	it("encodes the reference head and an explicit start, and only when supplied", () => {
		for (const encode of [
			encodeProgrammerValuesActionRequest,
			encodeProgrammerPreloadValuesActionRequest,
		] as const) {
			const request = (colorAdoption?: object) =>
				encode({
					requestId: "r",
					expectedRevision: 1,
					expectedPreloadRevision: 1,
					expectedCaptureModeRevision: 1,
					action: {
						action: "apply_intent",
						fixtureIds: ["a"],
						attribute: "color",
						operation: NATIVE,
						undoGroup: "g",
						timing: { fade: false, fadeMillis: null, delayMillis: null },
						...(colorAdoption ? { colorAdoption } : {}),
					},
				} as never) as unknown as { action: Record<string, unknown> };
			const plain = request().action;
			expect(plain).not.toHaveProperty("native_reference");
			expect(plain).not.toHaveProperty("explicit_color_start");
			const action = request({
				nativeReference: { fixtureId: "a", headId: "h" },
				explicitStart: { rgb: [0, 0, 0] },
			}).action;
			expect(action.native_reference).toEqual({ fixture_id: "a", head_id: "h" });
			expect(action.explicit_color_start).toEqual({ rgb: [0, 0, 0] });
			expect(action.operation).toEqual(NATIVE);
			expect(() => request({ explicitStart: { rgb: [1.5, 0, 0] } })).toThrow(WireValidationError);
		}
	});

	it("decodes every hold reason and the adoption report of exact-key outcomes", () => {
		const base = {
			request_id: "r",
			correlation_id: "a1000000-0000-4000-8000-000000000000",
			revision: 1,
			capture_mode_revision: 1,
			replayed: false,
			warning: null,
			status: "no_change",
		};
		for (const hold of [
			"displayed_source_unavailable",
			"native_color_unavailable",
			"explicit_color_start_required",
			"zoom_unavailable",
		]) {
			expect(decodeProgrammerValuesActionOutcome({ ...base, hold })).toMatchObject({ hold });
			expect(decodeProgrammerPreloadValuesActionOutcome({ ...base, hold })).toMatchObject({ hold });
		}
		const adopted = decodeProgrammerValuesActionOutcome({
			...base,
			color_adoption: {
				fixtures: [{ fixture_id: "a", start: "approximate", uv_unknown: true }],
				limitations: ["Direct UV amount is unknown; UV is adopted off."],
			},
		});
		expect(adopted).toMatchObject({
			colorAdoption: {
				fixtures: [{ fixtureId: "a", start: "approximate", uvUnknown: true }],
			},
		});
		expect(() =>
			decodeProgrammerValuesActionOutcome({ ...base, color_adoption: { fixtures: [{}] } }),
		).toThrow(WireValidationError);
	});

	it("decodes native pages at full width and names the reference in the path", () => {
		expect(nativeColorPagesPath(["a", "b"], { fixtureId: "b", headId: "h" })).toBe(
			"/api/v2/programming/color/native-pages?fixture_ids=a,b&reference=b&head=h",
		);
		const control = {
			id: "native.c",
			channel_id: "c",
			label: "White",
			raw_max: 4_294_967_295,
			resolution: "32bit",
			ultraviolet: false,
			functions: [
				{ function_id: "f", label: "White", raw_from: 0, raw_to: 4_294_967_295, continuous: true },
			],
		};
		const snapshot = {
			semantic: true,
			show_revision: 1,
			fixture_ids: ["a"],
			candidates: [],
			pages: [{ number: 3, controls: [control, null, null, null] }],
			overflow: [],
			fixtures: [{ fixture_id: "a", replay: "exact" }],
			future: true,
		};
		expect(decodeNativeColorPagesSnapshot(snapshot).pages[0].controls[0]?.raw_max).toBe(
			4_294_967_295,
		);
		expect(() =>
			decodeNativeColorPagesSnapshot({
				...snapshot,
				pages: [{ number: 3, controls: [{ ...control, raw_max: 4_294_967_296 }] }],
			}),
		).toThrow(WireValidationError);
	});
});
