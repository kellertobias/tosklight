import { describe, expect, it } from "vitest";
import {
	decodeFamilyEncoderPagesSnapshot,
	familyEncoderPagesPath,
	MAX_FAMILY_ENCODER_FIXTURES,
} from "./familyEncoderPagesWire";
import { WireValidationError } from "./wireValidation";

const descriptor = {
	owner: "zoom",
	role: "zoom",
	unit: "degrees",
	domain: { kind: "bounded", bounds: { min: 0, max: 180 } },
	step: 1,
	fine_step: 0.1,
	display_scale: 1,
	interpolation: "linear",
	capability: "semantic_intent",
	spread: true,
	align: true,
	dynamics: true,
};

const snapshot = () => ({
	semantic: false,
	supported_programming_contract: 0,
	semantic_programming_contract: 1,
	color_presentation: "advanced",
	show_revision: 4,
	fixture_ids: ["a"],
	future: "tolerated",
	families: [
		{
			family: "focus",
			owners: ["focus", "zoom"],
			fixture_ids: ["a"],
			replaces_attributes: ["focus", "zoom", "softness"],
			replaces_attribute_prefixes: [],
			pages: [
				{
					number: 1,
					label: "Focus · Zoom",
					slots: [
						{
							kind: "component",
							id: "zoom",
							label: "Zoom",
							component: { kind: "zoom" },
							descriptor,
							limits: { min: 8, max: 48 },
							limits_source: "selection",
							convention: "field",
							edit: "scalar",
							fixture_ids: ["a"],
						},
						{ kind: "attribute", attribute: "softness", label: "Softness" },
						null,
						null,
					],
				},
			],
			reserved_pages: [],
		},
	],
});

describe("family encoder pages wire", () => {
	it("decodes the typed snapshot and tolerates unknown fields", () => {
		const decoded = decodeFamilyEncoderPagesSnapshot(snapshot());
		expect(decoded.color_presentation).toBe("advanced");
		expect(decoded.families[0]?.pages[0]?.slots[0]).toMatchObject({
			kind: "component",
			limits: { min: 8, max: 48 },
		});
	});

	it("names the failing field", () => {
		const broken = snapshot();
		(broken.families[0]?.pages[0]?.slots[0] as { edit: string }).edit = "sideways";
		expect(() => decodeFamilyEncoderPagesSnapshot(broken)).toThrow(WireValidationError);
		expect(() => decodeFamilyEncoderPagesSnapshot({ ...snapshot(), semantic: "yes" })).toThrow(
			/semantic/,
		);
	});

	it("bounds the request path", () => {
		const ids = Array.from({ length: MAX_FAMILY_ENCODER_FIXTURES + 3 }, (_, index) => `f${index}`);
		const path = familyEncoderPagesPath(ids);
		expect(path.startsWith("/api/v2/programming/family-encoder-pages?fixture_ids=f0,f1")).toBe(true);
		expect(path.split(",")).toHaveLength(MAX_FAMILY_ENCODER_FIXTURES);
	});
});
