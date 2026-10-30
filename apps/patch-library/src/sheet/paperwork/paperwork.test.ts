import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../wire";
import { buildPaperwork, paperworkCsv } from "./paperwork";

function fixture(overrides: Partial<PatchedFixture> & { fixture_id: string }): PatchedFixture {
	return {
		fixture_number: null,
		name: "",
		universe: null,
		address: null,
		logical_heads: [],
		definition: { manufacturer: "Acme", name: "Spot", mode: "16ch", footprint: 16 } as PatchedFixture["definition"],
		...overrides,
	} as PatchedFixture;
}

const truss = fixture({ fixture_id: "truss", virtual_fixture_number: 1, name: "FOH Truss" });
const rig = [
	truss,
	fixture({ fixture_id: "c", fixture_number: 12, name: "Spot SR", position_master: "truss", location: { x: 4, y: 0, z: 6 }, split_patches: [{ split: 1, universe: 2, address: 1 }], installed_appearance: { gel: { type: "built_in", catalog_id: "lee", entry_id: "201", embedded_fallback: { number: "L201", name: "Full CT Blue", display_srgb: "#88aaff", visualizer_srgb: "#88aaff" } } } as never }),
	fixture({ fixture_id: "a", fixture_number: 2, name: "Spot SL", position_master: "truss", location: { x: -4, y: 0, z: 6 }, split_patches: [{ split: 1, universe: 1, address: 17 }], shaper_angle: 10 }),
	fixture({ fixture_id: "b", fixture_number: 7, name: "Floor wash", definition: { manufacturer: "Acme", name: "Wash", mode: "8ch", footprint: 8 } as never, split_patches: [{ split: 1, universe: null, address: null }] }),
];

describe("paperwork", () => {
	it("lists instruments by fixture ID, leaves Venue objects out and marks unpatched fixtures", () => {
		const report = buildPaperwork("channel_hookup", rig);
		expect(report.rows.map((row) => row[0])).toEqual(["2", "7", "12"]);
		expect(report.rows[1]).toEqual(["7", "Floor wash", "Acme Wash", "8ch", "Unpatched", "No position", "1", "Open white"]);
		expect(report.rows[2]).toContain("L201 Full CT Blue");
	});

	it("schedules by position, numbering units stage left to right", () => {
		const report = buildPaperwork("instrument_schedule", rig);
		expect(report.rows.map((row) => [row[0], row[1], row[2]])).toEqual([
			["FOH Truss", "1", "2"],
			["FOH Truss", "2", "12"],
			["No position", "1", "7"],
		]);
		expect(report.rows[0][7]).toBe("Shaper module");
	});

	it("sorts the DMX sheet by universe and address with unpatched splits last", () => {
		const report = buildPaperwork("dmx_sheet", rig);
		expect(report.rows.map((row) => row.slice(0, 4))).toEqual([
			["1", "17", "32", "2"],
			["2", "1", "16", "12"],
			["Unpatched", "", "", "7"],
		]);
	});

	it("counts types, gels and accessories", () => {
		expect(buildPaperwork("symbol_key", rig).rows).toEqual([
			["Acme Spot", "2", "0", "16ch"],
			["Acme Wash", "1", "1", "8ch"],
		]);
		expect(buildPaperwork("color_pull", rig).rows).toEqual([["L201 Full CT Blue", "1", "12"]]);
		expect(buildPaperwork("accessory_pull", rig).rows).toEqual([["Shaper module", "1", "2"]]);
	});

	it("writes RFC 4180 CSV", () => {
		const csv = paperworkCsv({ kind: "color_pull", title: "Color", columns: ["Color", "Fixtures"], rows: [['Lee "201"', "1, 2"]] });
		expect(csv).toBe('Color,Fixtures\r\n"Lee ""201""","1, 2"\r\n');
	});
});
