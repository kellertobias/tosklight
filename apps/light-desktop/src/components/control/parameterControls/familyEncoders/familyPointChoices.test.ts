import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../../../api/types";
import { FIXTURE_A, FIXTURE_B, POINT } from "./familyEncoderTestSupport";
import { familyPointChoices, pointSlotDisplay } from "./familyPointChoices";

/** TL-544 G4: the Point slot's ordered choices and readout. */

const POINT_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

function fixture(id: string, number: number, name: string, point: boolean) {
	return {
		fixture_id: id,
		fixture_number: number,
		virtual_fixture_number: null,
		name,
		definition: {
			name,
			mode_id: "m",
			profile_snapshot: null,
			heads: [
				{ parameters: point ? [{ attribute: "point.position.x" }] : [{ attribute: "dimmer" }] },
			],
		},
	} as unknown as PatchedFixture;
}

const target = (reference: object) => ({
	kind: "position",
	value: { kind: "target", reference, offset_metres: [] },
});

describe("Point slot choices", () => {
	it("lists Origin, then the show's 3D Points in Patch order, named as the Patch names them", () => {
		const choices = familyPointChoices([
			fixture(FIXTURE_A, 1, "Spot", false),
			fixture(POINT_ID, 900, "Singer", true),
		]);
		expect(choices.map(({ value, label }) => ({ value, label }))).toEqual([
			{ value: "origin", label: "Origin" },
			{ value: `point:${POINT_ID}`, label: "900 · Singer" },
		]);
		expect(choices[1]?.reference).toEqual({ kind: "point", point_id: POINT_ID });
	});

	it("reads the selection's shared reference, Mixed, a missing Point, or nothing", () => {
		const choices = familyPointChoices([fixture(POINT_ID, 900, "Singer", true)]);
		const entry = (fixtureId: string, reference: object) => ({
			fixtureId,
			attribute: "position",
			value: target(reference),
		});
		const read = (values: unknown[]) =>
			pointSlotDisplay(POINT, values as never, choices).text;
		const point = { kind: "point", point_id: POINT_ID };
		expect(read([])).toBe("—");
		expect(read([entry(FIXTURE_A, point), entry(FIXTURE_B, point)])).toBe("900 · Singer");
		expect(read([entry(FIXTURE_A, point), entry(FIXTURE_B, { kind: "origin" })])).toBe("Mixed");
		const gone = { kind: "point", point_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb" };
		expect(read([entry(FIXTURE_A, gone), entry(FIXTURE_B, gone)])).toBe("Missing point");
	});
});
