import { describe, expect, it } from "vitest";
import { predictProgrammerValues } from "./prediction";
import {
	FIXTURE_1,
	fixtureValue,
	groupValue,
	valuesProjection,
} from "./testFixtures";

const timing = { fade: false, fadeMillis: null, delayMillis: null };

describe("Programmer values prediction", () => {
	it("returns the same projection for an exact fixture set and missing release", () => {
		const projection = valuesProjection();
		const exact = predictProgrammerValues({
			action: "set_fixture",
			fixtureId: FIXTURE_1,
			attribute: "intensity",
			value: { kind: "normalized", value: 0.25 },
			timing,
		})(projection);
		const missing = predictProgrammerValues({
			action: "release_group",
			groupId: "missing",
			attribute: "intensity",
		})(projection);

		expect(exact).toBe(projection);
		expect(missing).toBe(projection);
	});

	it("sets and releases fixture and Group values in one batch", () => {
		const projection = valuesProjection({ groupValues: [groupValue()] });
		const predicted = predictProgrammerValues({
			action: "batch",
			mutations: [
				{
					action: "release_fixture",
					fixtureId: FIXTURE_1,
					attribute: "intensity",
				},
				{
					action: "set_group",
					groupId: "front",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
					timing: { fade: true, fadeMillis: 500, delayMillis: 25 },
				},
			],
		})(projection);

		expect(predicted.fixtureValues).toEqual([]);
		expect(predicted.groupValues).toMatchObject([
			{
				groupId: "front",
				value: { kind: "normalized", value: 0.8 },
				fade: true,
				fadeMillis: 500,
				delayMillis: 25,
			},
		]);
	});

	it("clears both retained collections without cloning an empty projection", () => {
		const populated = valuesProjection({
			fixtureValues: [fixtureValue()],
			groupValues: [groupValue()],
		});
		const cleared = predictProgrammerValues({ action: "clear" })(populated);
		const alreadyEmpty = valuesProjection({ fixtureValues: [], groupValues: [] });

		expect(cleared).toMatchObject({ fixtureValues: [], groupValues: [] });
		expect(predictProgrammerValues({ action: "clear" })(alreadyEmpty)).toBe(
			alreadyEmpty,
		);
	});
});
