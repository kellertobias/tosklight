import { describe, expect, it } from "vitest";
import type { RuntimeGroup } from "../features/groupRuntime/groupRuntimeAuthority";
import type { ProgrammerValuesProjection } from "../features/programmerValues/contracts";
import { fixtureSheetProgrammerValueIndex } from "./fixtureSheetProjection";

describe("Fixture Sheet programmer value projection", () => {
	it("uses Programmer order across direct and spread Group values", () => {
		const projection: ProgrammerValuesProjection = {
			userId: "operator",
			revision: 1,
			fixtureValues: [
				{
					fixtureId: "fixture-2",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
					programmerOrder: 20,
					fade: true,
					fadeMillis: null,
					delayMillis: null,
				},
			],
			groupValues: [
				{
					groupId: "line",
					attribute: "intensity",
					value: { kind: "spread", value: [0.1, 0.5] },
					programmerOrder: 10,
					fade: true,
					fadeMillis: null,
					delayMillis: null,
				},
			],
		};
		const group = {
			id: "line",
			body: { fixtures: ["fixture-1", "fixture-2", "fixture-3"] },
		} as RuntimeGroup;

		const values = fixtureSheetProgrammerValueIndex(projection, [group]);

		expect(values.get("fixture-1")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0.1,
		});
		expect(values.get("fixture-2")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0.8,
		});
		expect(values.get("fixture-3")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0.5,
		});
	});
});
