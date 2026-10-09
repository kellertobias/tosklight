import { describe, expect, it } from "vitest";
import { decodeCueListBody } from "./showObjectCueWire";

const body = {
	id: "list",
	name: "List",
	mode: "sequence",
	priority: 0,
	looped: false,
	cues: [],
};
describe("Cuelist pool address wire metadata", () => {
	it("keeps literal legacy bodies free of invented pool metadata", () => {
		const decoded = decodeCueListBody(body, "$");
		expect(decoded).not.toHaveProperty("pool_number");
		expect(decoded).not.toHaveProperty("legacy_pool_aliases");
	});
	it("retains validated canonical numbers and aliases", () => {
		expect(
			decodeCueListBody(
				{ ...body, pool_number: 101, legacy_pool_aliases: [190] },
				"$",
			),
		).toMatchObject({ pool_number: 101, legacy_pool_aliases: [190] });
	});
	it.each([
		{ pool_number: "101" },
		{ pool_number: 0 },
		{ pool_number: 1001 },
		{ pool_number: 101, legacy_pool_aliases: [101] },
		{ pool_number: 101, legacy_pool_aliases: [190, 190] },
		{ legacy_pool_aliases: [190] },
		{ pool_number: 101, legacy_pool_aliases: [1001] },
	])("rejects malformed pool metadata before installing a Cuelist", (metadata) => {
		expect(() => decodeCueListBody({ ...body, ...metadata }, "$")).toThrow();
	});
});
