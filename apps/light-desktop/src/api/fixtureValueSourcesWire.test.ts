import { describe, expect, it } from "vitest";
import { MediaOutputApiClient } from "./client/mediaOutput";
import { decodeFixtureValueSources } from "./fixtureValueSourcesWire";
import type { LiveClientTransport } from "./client/transport";

describe("fixture source wire boundary", () => {
	it("uses the authenticated current-show read and validates semantic values", async () => {
		const paths: string[] = [];
		const payload = {
			fixture_id: "head",
			generated_at: "now",
			grand_master: 1,
			blackout: false,
			highlighted: false,
			values: [
				{
					attribute: "intensity",
					value: { kind: "normalized", value: 0.5 },
					sources: ["Programmer"],
				},
			],
			dynamics: [],
		};
		const client = new MediaOutputApiClient({
			request: async (path: string) => {
				paths.push(path);
				return payload;
			},
		} as LiveClientTransport);
		expect((await client.valueSources("a/b")).values[0].sources).toEqual([
			"Programmer",
		]);
		expect(paths).toEqual(["/api/v2/output/value-sources?fixture_id=a%2Fb"]);
		expect(() =>
			decodeFixtureValueSources({
				...payload,
				values: [
					{ ...payload.values[0], value: { kind: "invented", value: 1 } },
				],
			}),
		).toThrow();
	});
});
