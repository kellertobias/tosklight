import { describe, expect, it } from "vitest";
import {
	FIXTURE_1,
	FIXTURE_2,
	fixtureValue,
} from "../../features/programmerValues/testFixtures";
import { mediaColourMutation } from "./mediaColour";
import { mediaColourHex } from "./specializedMediaControls";
import { mediaProgrammerValues } from "./useMediaProgrammerValues";

function picked(hex: string) {
	const mutation = mediaColourMutation(FIXTURE_1, hex);
	if (!mutation || mutation.action !== "set_fixture")
		throw new Error("Missing colour mutation");
	return fixtureValue(0, { attribute: "color", value: mutation.value });
}

describe("Media whole colour", () => {
	it("round trips exact picker RGB including dark colours and black", () => {
		for (const hex of ["#ef4444", "#008080", "#000000", "#000020"])
			expect(mediaColourHex([picked(hex)])).toBe(hex);
		expect(mediaColourMutation(FIXTURE_1, "red")).toBeNull();
	});
	it("retains independent semantic envelope and never borrows another layer", () => {
		const current = picked("#ff0000");
		if (
			current.value.kind !== "color_program" ||
			current.value.value.kind !== "semantic"
		)
			throw new Error("Expected semantic colour");
		const intent = current.value.value.intent;
		intent.white_blend = 0.25;
		intent.uv.amount = 0.4;
		intent.relative_output = 0.8;
		intent.recipe.amber = 0.2;
		const mutation = mediaColourMutation(FIXTURE_1, "#0000ff", [current]);
		expect(mutation).toMatchObject({
			value: {
				kind: "color_program",
				value: {
					kind: "semantic",
					intent: {
						white_blend: 0.25,
						uv: { amount: 0.4 },
						relative_output: 0.8,
						recipe: { rgb: [0, 0, 1], amber: 0.2 },
					},
				},
			},
		});
		expect(mediaColourMutation(FIXTURE_2, "#0000ff", [current])).toMatchObject({
			value: {
				value: {
					intent: {
						white_blend: 0,
						uv: { amount: 0 },
						relative_output: 1,
						recipe: { amber: 0 },
					},
				},
			},
		});
	});
	it("shows capture override while retaining unrelated live values and reset fallback", () => {
		const live = [picked("#ff0000"), fixtureValue(0.7)];
		const captured = picked("#0000ff");
		expect(mediaColourHex(mediaProgrammerValues(live, [captured]))).toBe(
			"#0000ff",
		);
		expect(mediaProgrammerValues(live, [captured])).toContain(live[1]);
		expect(mediaColourHex(mediaProgrammerValues(live, []))).toBe("#ff0000");
		expect(
			mediaColourHex([fixtureValue(0.3, { attribute: "color.red" })]),
		).toBe("#4dffff");
	});
});
