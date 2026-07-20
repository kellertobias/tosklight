import { describe, expect, it } from "vitest";
import { predictProgrammerPreloadValues } from "./prediction";
import {
	FIXTURE_1,
	FIXTURE_2,
	preloadFixtureValue,
	preloadGroupValue,
	preloadProjection,
} from "./testFixtures";

const timing = { fade: true, fadeMillis: 500, delayMillis: 100 };

describe("predictProgrammerPreloadValues", () => {
	it("preserves ordered batch intent, timing, and Programmer order", () => {
		const current = preloadProjection({
			fixtureValues: [preloadFixtureValue(0.2, { programmerOrder: 4 })],
			groupValues: [preloadGroupValue(0.3, { programmerOrder: 7 })],
		});

		const predicted = predictProgrammerPreloadValues({
			action: "batch",
			mutations: [
				{
					action: "set_fixture",
					fixtureId: FIXTURE_2,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
					timing,
				},
				{
					action: "set_group",
					groupId: "back",
					attribute: "color.red",
					value: { kind: "normalized", value: 0.6 },
					timing: { fade: false, fadeMillis: null, delayMillis: null },
				},
				{
					action: "release_fixture",
					fixtureId: FIXTURE_1,
					attribute: "intensity",
				},
			],
		})(current);

		expect(predicted.fixtureValues).toEqual([
			expect.objectContaining({
				fixtureId: FIXTURE_2,
				programmerOrder: 8,
				fade: true,
				fadeMillis: 500,
				delayMillis: 100,
			}),
		]);
		expect(predicted.groupValues).toEqual([
			preloadGroupValue(0.3, { programmerOrder: 7 }),
			expect.objectContaining({ groupId: "back", programmerOrder: 9 }),
		]);
	});

	it("returns the same projection for an exact set or empty batch", () => {
		const current = preloadProjection();
		const exact = predictProgrammerPreloadValues({
			action: "set_fixture",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
			value: { kind: "normalized", value: 0.25 },
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		})(current);

		expect(exact).toBe(current);
		expect(
			predictProgrammerPreloadValues({ action: "batch", mutations: [] })(
				current,
			),
		).toBe(current);
	});
});
