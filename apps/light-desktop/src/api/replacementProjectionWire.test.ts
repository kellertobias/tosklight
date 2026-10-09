import { describe, expect, it } from "vitest";
import {
	decodeReplacementProjection,
	decodeReplacementMap,
	decodePresetReplacementFields,
} from "./replacementProjectionWire";
import { decodeProgrammerValuesProjection } from "./programmerValuesWireProjection";
const ids = Array.from(
	{ length: 8 },
	(_, index) =>
		`00000000-0000-0000-0000-${String(index + 1).padStart(12, "0")}`,
);
const projection = () => ({
	source_owner: ids[0],
	source_profile: { profile_id: ids[1], profile_revision: 1, mode_id: ids[2] },
	source_head_id: ids[3],
	target_profile: { profile_id: ids[4], profile_revision: 2, mode_id: ids[5] },
	targets: [{ fixture_id: ids[6], profile_head_id: ids[7] }],
});
describe("replacement envelope decoding", () => {
	it("preserves exact source/destination identity including explicit dormant targets", () => {
		expect(decodeReplacementProjection(projection(), "$", ids[0])).toEqual(
			projection(),
		);
		const dormant = { ...projection(), targets: [] };
		expect(decodeReplacementProjection(dormant, "$", ids[0])).toEqual(dormant);
		expect(decodeReplacementMap({ [ids[0]]: dormant }, "$")).toEqual({
			[ids[0]]: dormant,
		});
	});
	it("rejects bad source identity, duplicate destinations, nil UUID and unknown metadata", () => {
		expect(() =>
			decodeReplacementProjection(projection(), "$", ids[6]),
		).toThrow();
		expect(() =>
			decodeReplacementProjection(
				{
					...projection(),
					targets: [projection().targets[0], projection().targets[0]],
				},
				"$",
			),
		).toThrow();
		expect(() =>
			decodeReplacementProjection(
				{
					...projection(),
					source_head_id: "00000000-0000-0000-0000-000000000000",
				},
				"$",
			),
		).toThrow();
		expect(() =>
			decodeReplacementProjection({ ...projection(), extra: true }, "$"),
		).toThrow();
		expect(() =>
			decodeReplacementMap({ [ids[6]]: projection() }, "$"),
		).toThrow();
	});
	it("keeps legacy absent fields absent and validates direct/group preset source maps", () => {
		expect(decodePresetReplacementFields({}, "$")).toEqual({});
		const body = {
			fixture_replacement_projections: {
				[ids[0]]: { intensity: projection() },
			},
			group_replacement_projections: {
				Front: { intensity: { [ids[0]]: projection() } },
			},
		};
		expect(decodePresetReplacementFields(body, "$")).toEqual(body);
		expect(() =>
			decodePresetReplacementFields(
				{
					fixture_replacement_projections: {
						[ids[6]]: { intensity: projection() },
					},
				},
				"$",
			),
		).toThrow();
	});
	it("normal Programmer DTO carries envelope without changing scalar/timing/order", () => {
		const fixture = {
			fixture_id: ids[0],
			attribute: "intensity",
			value: { kind: "normalized", value: 0.4 },
			programmer_order: 19,
			fade: true,
			fade_millis: 300,
			delay_millis: 20,
			replacement_projection: projection(),
		};
		const result = decodeProgrammerValuesProjection(
			{
				revision: 3,
				fixture_values: [fixture],
				group_values: [],
				dynamic_values: [],
			},
			"$",
		);
		expect(result.fixtureValues[0]).toMatchObject({
			fixtureId: ids[0],
			programmerOrder: 19,
			fadeMillis: 300,
			delayMillis: 20,
			replacementProjection: projection(),
		});
		expect(() =>
			decodeProgrammerValuesProjection(
				{
					revision: 3,
					fixture_values: [
						{
							...fixture,
							replacement_projection: { ...projection(), source_owner: ids[6] },
						},
					],
					group_values: [],
					dynamic_values: [],
				},
				"$",
			),
		).toThrow();
	});
});
