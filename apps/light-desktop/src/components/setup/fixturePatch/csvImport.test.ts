import { describe, expect, it } from "vitest";
import type { FixtureDefinition, PatchedFixture } from "../../../api/types";
import {
	type CsvColumnAssignments,
	csvImportCandidates,
	csvSourceTypes,
	exactDefinitionMatch,
	guessColumnAssignments,
	parseCsv,
	parseCsvPatch,
	planCsvImport,
} from "./csvImport";

function definition(
	manufacturer: string,
	name: string,
	mode: string,
	footprint: number,
): FixtureDefinition {
	return {
		schema_version: 2,
		id: `${manufacturer}-${name}-${mode}`,
		revision: 1,
		manufacturer,
		device_type: "spot",
		name,
		model: name,
		mode,
		footprint,
		heads: [],
		color_calibration: null,
		physical: {},
		model_asset: null,
		icon_asset: null,
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		safe_values: {},
		profile_id: `${manufacturer}-${name}`,
		mode_id: `${manufacturer}-${name}-${mode}`,
		profile_snapshot: null,
	};
}

const auraExtended = definition("Martin", "MAC Aura", "Extended", 25);
const auraStandard = definition("Martin", "MAC Aura", "Standard", 14);
const dimmer = definition("Generic", "Dimmer", "8 bit", 1);
const library = [auraExtended, auraStandard, dimmer];

const assignments: CsvColumnAssignments = [
	"fixture_id",
	"name",
	"manufacturer",
	"fixture_type",
	"mode",
	"patch",
	"x",
	"y",
	"z",
	"rot_x",
	"rot_y",
	"rot_z",
];

function existingFixture(fixtureNumber: number, universe: number, address: number) {
	return {
		fixture_id: `existing-${fixtureNumber}`,
		fixture_number: fixtureNumber,
		name: `Existing ${fixtureNumber}`,
		definition: dimmer,
		universe,
		address,
		split_patches: [{ split: 1, universe, address }],
		multipatch: [],
	} as unknown as PatchedFixture;
}

function plan(rows: string[][], existing: PatchedFixture[] = [], conflictPolicy: "unpatch" | "skip" = "unpatch") {
	const sourceTypes = csvSourceTypes(rows, assignments, library);
	return planCsvImport({
		rows,
		lines: rows.map((_, index) => index + 2),
		assignments,
		sourceTypes,
		mappings: {},
		existing,
		positionUnit: "m",
		conflictPolicy,
	});
}

describe("CSV patch import", () => {
	it("parses quoted fields, CRLF, BOM, and a detected semicolon delimiter", () => {
		const parsed = parseCsv(
			'﻿ID;Name;Type\r\n1;"Wash; Left";"MAC ""Aura"""\r\n\r\n2;Right;Dimmer\n',
		);
		expect(parsed.delimiter).toBe(";");
		expect(parsed.rows).toEqual([
			["ID", "Name", "Type"],
			["1", "Wash; Left", 'MAC "Aura"'],
			["2", "Right", "Dimmer"],
		]);
	});

	it("suggests every requested column from common header names", () => {
		expect(
			guessColumnAssignments([
				"Patch",
				"Fixture ID",
				"Fixture Name",
				"Fixture Type",
				"Manufacturer",
				"Mode",
				"X",
				"Y",
				"Z",
				"RotX",
				"RotY",
				"RotZ",
				"Notes",
			]),
		).toEqual([
			"patch",
			"fixture_id",
			"name",
			"fixture_type",
			"manufacturer",
			"mode",
			"x",
			"y",
			"z",
			"rot_x",
			"rot_y",
			"rot_z",
			null,
		]);
	});

	it("matches manufacturer, type, and mode exactly, ignoring case and spacing only", () => {
		expect(exactDefinitionMatch(library, "martin", " MAC  Aura ", "extended")).toBe(
			auraExtended,
		);
		expect(exactDefinitionMatch(library, "", "Martin MAC Aura", "Standard")).toBe(
			auraStandard,
		);
		expect(exactDefinitionMatch(library, "Martin", "MAC Aura", "")).toBeNull();
		expect(exactDefinitionMatch(library, "Generic", "Dimmer", "")).toBe(dimmer);
		expect(exactDefinitionMatch(library, "Martin", "MAC Aura XB", "Extended")).toBeNull();
		expect(exactDefinitionMatch(library, "Robe", "MAC Aura", "Extended")).toBeNull();
	});

	it("groups rows into source fixture types and reports exact matches", () => {
		const rows = [
			["1", "", "Martin", "MAC Aura", "Extended", "1.1", "", "", "", "", "", ""],
			["2", "", "martin", "mac aura", "extended", "1.26", "", "", "", "", "", ""],
			["3", "", "Acme", "Spot 1", "Basic", "", "", "", "", "", "", ""],
		];
		const types = csvSourceTypes(rows, assignments, library);
		expect(types).toHaveLength(2);
		expect(types[0]).toMatchObject({ rowCount: 2, exactMatch: auraExtended });
		expect(types[1]).toMatchObject({
			manufacturer: "Acme",
			fixtureType: "Spot 1",
			rowCount: 1,
			exactMatch: null,
		});
	});

	it("parses dotted, slashed, and absolute DMX patches", () => {
		expect(parseCsvPatch("2.101")).toEqual({ universe: 2, address: 101 });
		expect(parseCsvPatch("2/101")).toEqual({ universe: 2, address: 101 });
		expect(parseCsvPatch("513")).toEqual({ universe: 2, address: 1 });
		expect(parseCsvPatch("")).toBeNull();
		expect(parseCsvPatch("-")).toBeNull();
		expect(parseCsvPatch("1.600")).toBe("invalid");
		expect(parseCsvPatch("A1")).toBe("invalid");
	});

	it("plans IDs, names, metre positions, rotations, and patches for each row", () => {
		const rows = [
			["101", "Aura SL", "Martin", "MAC Aura", "Extended", "1.1", "1.5", "-2", "6,25", "0", "90", "180"],
			["", "", "Generic", "Dimmer", "", "", "", "", "", "", "", ""],
		];
		const plans = plan(rows, [existingFixture(1, 3, 1)]);
		expect(plans[0]).toMatchObject({
			status: "ready",
			fixtureNumber: 101,
			name: "Aura SL",
			definition: auraExtended,
			patch: { universe: 1, address: 1 },
			location: { x: 1500, y: -2000, z: 6250 },
			rotation: { x: 0, y: 90, z: 180 },
		});
		expect(plans[1]).toMatchObject({
			status: "ready",
			fixtureNumber: 2,
			name: "Dimmer 2",
			patch: null,
		});

		const candidates = csvImportCandidates(plans, "default");
		expect(candidates.map((candidate) => candidate.input)).toMatchObject([
			{
				fixtureNumber: 101,
				name: "Aura SL",
				splitPatches: [{ split: 1, universe: 1, address: 1 }],
				location: { x: 1500, y: -2000, z: 6250 },
				rotation: { x: 0, y: 90, z: 180 },
			},
			{ fixtureNumber: 2, splitPatches: [{ split: 1, universe: null, address: null }] },
		]);
	});

	it("rejects duplicate or used IDs and invalid cells per row without blocking others", () => {
		const rows = [
			["5", "", "Generic", "Dimmer", "", "1.10", "", "", "", "", "", ""],
			["5", "", "Generic", "Dimmer", "", "1.11", "", "", "", "", "", ""],
			["7", "", "Generic", "Dimmer", "", "1.12", "", "", "", "", "", ""],
			["x", "", "Generic", "Dimmer", "", "", "", "", "", "", "", ""],
			["8", "", "Generic", "Dimmer", "", "1.13", "abc", "", "", "", "", ""],
			["9", "", "Martin", "MAC Aura", "Extended", "1.500", "", "", "", "", "", ""],
			["10", "", "Acme", "Unknown", "", "", "", "", "", "", "", ""],
		];
		const plans = plan(rows, [existingFixture(7, 4, 1)]);
		expect(plans.map((row) => [row.status, row.message])).toEqual([
			["ready", null],
			["error", "Fixture ID 5 is already used on row 2."],
			["error", "Fixture ID 7 is already used in the show."],
			["error", "Fixture ID “x” is not a positive whole number."],
			["error", "X is not a number."],
			["error", "1.500 leaves no room for 25 channels in the universe."],
			["error", "No library fixture is selected for this fixture type."],
		]);
		expect(csvImportCandidates(plans, "default")).toHaveLength(1);
	});

	it("imports address conflicts unpatched or skips them by explicit policy", () => {
		const rows = [
			["1", "", "Generic", "Dimmer", "", "1.1", "", "", "", "", "", ""],
			["2", "", "Generic", "Dimmer", "", "1.20", "", "", "", "", "", ""],
			["3", "", "Generic", "Dimmer", "", "1.20", "", "", "", "", "", ""],
		];
		const existing = [existingFixture(50, 1, 1)];
		expect(plan(rows, existing).map((row) => [row.status, row.patch])).toEqual([
			["unpatched", null],
			["ready", { universe: 1, address: 20 }],
			["unpatched", null],
		]);
		expect(plan(rows, existing, "skip").map((row) => row.status)).toEqual([
			"error",
			"ready",
			"error",
		]);
	});

	it("uses the wizard selection or skip decision for unmatched types", () => {
		const rows = [["", "", "Acme", "Spot 1", "Basic", "2.1", "", "", "", "", "", ""]];
		const sourceTypes = csvSourceTypes(rows, assignments, library);
		const input = {
			rows,
			lines: [2],
			assignments,
			sourceTypes,
			existing: [],
			positionUnit: "mm" as const,
			conflictPolicy: "unpatch" as const,
		};
		expect(
			planCsvImport({ ...input, mappings: { [sourceTypes[0].key]: auraStandard } })[0],
		).toMatchObject({ status: "ready", definition: auraStandard, patch: { universe: 2, address: 1 } });
		expect(
			planCsvImport({ ...input, mappings: { [sourceTypes[0].key]: "skip" } })[0].status,
		).toBe("skipped");
	});
});
