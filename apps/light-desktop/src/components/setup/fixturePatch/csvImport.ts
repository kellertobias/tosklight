import type { FixtureDefinition, PatchedFixture } from "../../../api/types";
import {
	newPatchFixtureCandidate,
	type PatchFixtureCandidate,
} from "../../../features/patch/PatchContext";
import { conflicts, isDmxPatchable, isVisualOnly } from "../patchUtils";
import {
	nextAvailableFixtureNumber,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
} from "./fixtureIds";
import { definitionSplits } from "./patchModel";

export type CsvImportField =
	| "patch"
	| "fixture_id"
	| "name"
	| "manufacturer"
	| "fixture_type"
	| "mode"
	| "x"
	| "y"
	| "z"
	| "rot_x"
	| "rot_y"
	| "rot_z";

export const CSV_IMPORT_FIELDS: ReadonlyArray<{
	field: CsvImportField;
	label: string;
}> = [
	{ field: "patch", label: "Patch" },
	{ field: "fixture_id", label: "Fixture ID" },
	{ field: "name", label: "Fixture Name" },
	{ field: "fixture_type", label: "Fixture Type" },
	{ field: "manufacturer", label: "Manufacturer" },
	{ field: "mode", label: "Mode" },
	{ field: "x", label: "X" },
	{ field: "y", label: "Y" },
	{ field: "z", label: "Z" },
	{ field: "rot_x", label: "RotX" },
	{ field: "rot_y", label: "RotY" },
	{ field: "rot_z", label: "RotZ" },
];

/** One assigned field per CSV column index; `null` ignores the column. */
export type CsvColumnAssignments = ReadonlyArray<CsvImportField | null>;

export type CsvPositionUnit = "m" | "mm";

export type CsvConflictPolicy = "unpatch" | "skip";

/** A library selection for one source fixture type, or an explicit decision to skip its rows. */
export type CsvTypeMapping = FixtureDefinition | "skip";

export interface ParsedCsv {
	delimiter: string;
	rows: string[][];
}

export interface CsvSourceType {
	key: string;
	manufacturer: string;
	fixtureType: string;
	mode: string;
	rowCount: number;
	exactMatch: FixtureDefinition | null;
}

export type CsvRowStatus = "ready" | "unpatched" | "skipped" | "error";

export interface CsvImportRowPlan {
	line: number;
	sourceTypeKey: string;
	definition: FixtureDefinition | null;
	fixtureNumber: number | null;
	virtualFixtureNumber: number | null;
	name: string;
	patch: { universe: number; address: number } | null;
	location: { x: number; y: number; z: number };
	rotation: { x: number; y: number; z: number };
	status: CsvRowStatus;
	message: string | null;
}

const CANDIDATE_DELIMITERS = [",", ";", "\t"] as const;

/** Parses RFC 4180 CSV with quoted fields, detecting a comma, semicolon, or tab delimiter. */
export function parseCsv(text: string): ParsedCsv {
	const source = text.replace(/^\uFEFF/, "");
	const delimiter = detectDelimiter(source);
	const rows: string[][] = [];
	let row: string[] = [];
	let field = "";
	let quoted = false;
	for (let index = 0; index < source.length; index++) {
		const char = source[index];
		if (quoted) {
			if (char === '"' && source[index + 1] === '"') {
				field += '"';
				index++;
			} else if (char === '"') quoted = false;
			else field += char;
			continue;
		}
		if (char === '"' && field.trim() === "") {
			field = "";
			quoted = true;
		} else if (char === delimiter) {
			row.push(field);
			field = "";
		} else if (char === "\n" || char === "\r") {
			if (char === "\r" && source[index + 1] === "\n") index++;
			row.push(field);
			rows.push(row);
			row = [];
			field = "";
		} else field += char;
	}
	if (field !== "" || row.length) {
		row.push(field);
		rows.push(row);
	}
	return {
		delimiter,
		rows: rows
			.map((cells) => cells.map((cell) => cell.trim()))
			.filter((cells) => cells.some((cell) => cell !== "")),
	};
}

function detectDelimiter(text: string) {
	const firstLine = text.split(/\r?\n/).find((line) => line.trim() !== "") ?? "";
	let best: string = ",";
	let bestCount = 0;
	for (const delimiter of CANDIDATE_DELIMITERS) {
		let count = 0;
		let quoted = false;
		for (const char of firstLine) {
			if (char === '"') quoted = !quoted;
			else if (!quoted && char === delimiter) count++;
		}
		if (count > bestCount) {
			best = delimiter;
			bestCount = count;
		}
	}
	return best;
}

const HEADER_SYNONYMS: Record<CsvImportField, readonly string[]> = {
	patch: ["patch", "address", "dmx", "dmxaddress", "dmxpatch", "addr", "universeaddress"],
	fixture_id: ["fixtureid", "fid", "id", "fixturenumber", "fixtureno", "fixno", "number", "no"],
	name: ["name", "fixturename", "label"],
	fixture_type: ["fixturetype", "type", "model", "fixturemodel", "instrumenttype", "fixture"],
	manufacturer: ["manufacturer", "make", "brand", "vendor"],
	mode: ["mode", "dmxmode", "fixturemode", "personality"],
	x: ["x", "posx", "positionx", "locationx", "xpos", "xposition"],
	y: ["y", "posy", "positiony", "locationy", "ypos", "yposition"],
	z: ["z", "posz", "positionz", "locationz", "zpos", "zposition"],
	rot_x: ["rotx", "rotationx", "xrot", "xrotation", "rx"],
	rot_y: ["roty", "rotationy", "yrot", "yrotation", "ry"],
	rot_z: ["rotz", "rotationz", "zrot", "zrotation", "rz"],
};

/** Suggests one field per recognised header; the operator confirms or changes every column. */
export function guessColumnAssignments(
	headers: readonly string[],
): CsvColumnAssignments {
	const used = new Set<CsvImportField>();
	return headers.map((header) => {
		const normalized = header.toLocaleLowerCase().replace(/[^a-z0-9]/g, "");
		const field = CSV_IMPORT_FIELDS.find(
			({ field }) =>
				!used.has(field) && HEADER_SYNONYMS[field].includes(normalized),
		)?.field;
		if (!field) return null;
		used.add(field);
		return field;
	});
}

function normalizeName(value: string) {
	return value.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

export function csvSourceTypeKey(
	manufacturer: string,
	fixtureType: string,
	mode: string,
) {
	return JSON.stringify([
		normalizeName(manufacturer),
		normalizeName(fixtureType),
		normalizeName(mode),
	]);
}

/**
 * Finds the single library mode whose manufacturer, fixture type, and mode exactly match the file,
 * ignoring case and repeated whitespace. Without a Manufacturer column the Fixture Type may carry
 * `Manufacturer Type`. Without a Mode column only a single-mode fixture is an exact match.
 */
export function exactDefinitionMatch(
	definitions: readonly FixtureDefinition[],
	manufacturer: string,
	fixtureType: string,
	mode: string,
): FixtureDefinition | null {
	const type = normalizeName(fixtureType);
	if (!type) return null;
	const wantedManufacturer = normalizeName(manufacturer);
	const wantedMode = normalizeName(mode);
	const typeMatches = definitions.filter((definition) => {
		const names = [definition.name, definition.model]
			.filter(Boolean)
			.map(normalizeName);
		const definitionManufacturer = normalizeName(definition.manufacturer);
		return wantedManufacturer
			? definitionManufacturer === wantedManufacturer && names.includes(type)
			: names.some(
					(name) =>
						name === type || `${definitionManufacturer} ${name}` === type,
				);
	});
	const candidates = wantedMode
		? typeMatches.filter(
				(definition) => normalizeName(definition.mode) === wantedMode,
			)
		: typeMatches;
	return candidates.length === 1 ? candidates[0] : null;
}

function cell(
	row: readonly string[],
	assignments: CsvColumnAssignments,
	field: CsvImportField,
) {
	const index = assignments.indexOf(field);
	return index < 0 ? "" : (row[index] ?? "").trim();
}

export function csvSourceTypes(
	rows: readonly (readonly string[])[],
	assignments: CsvColumnAssignments,
	definitions: readonly FixtureDefinition[],
): CsvSourceType[] {
	const byKey = new Map<string, CsvSourceType>();
	for (const row of rows) {
		const manufacturer = cell(row, assignments, "manufacturer");
		const fixtureType = cell(row, assignments, "fixture_type");
		const mode = cell(row, assignments, "mode");
		const key = csvSourceTypeKey(manufacturer, fixtureType, mode);
		const existing = byKey.get(key);
		if (existing) {
			existing.rowCount++;
			continue;
		}
		byKey.set(key, {
			key,
			manufacturer,
			fixtureType,
			mode,
			rowCount: 1,
			exactMatch: exactDefinitionMatch(
				definitions,
				manufacturer,
				fixtureType,
				mode,
			),
		});
	}
	return [...byKey.values()];
}

export function describeSourceType(
	source: Pick<CsvSourceType, "manufacturer" | "fixtureType" | "mode">,
) {
	const type =
		[source.manufacturer, source.fixtureType].filter(Boolean).join(" ") ||
		"No fixture type";
	return source.mode ? `${type} · ${source.mode}` : type;
}

/**
 * Parses a DMX patch cell. Accepts `universe.address`, `universe/address`, `universe:address`,
 * or one absolute 1-based address. An empty cell, `-`, or `none` means unpatched.
 */
export function parseCsvPatch(
	value: string,
): { universe: number; address: number } | null | "invalid" {
	const text = value.trim();
	if (!text || /^(-|—|none|unpatched|n\/a)$/i.test(text)) return null;
	const split = /^(\d+)\s*[./:]\s*(\d+)$/.exec(text);
	if (split) {
		const universe = Number(split[1]);
		const address = Number(split[2]);
		return universe >= 1 && address >= 1 && address <= 512
			? { universe, address }
			: "invalid";
	}
	if (/^\d+$/.test(text)) {
		const absolute = Number(text);
		if (absolute < 1) return "invalid";
		return {
			universe: Math.floor((absolute - 1) / 512) + 1,
			address: ((absolute - 1) % 512) + 1,
		};
	}
	return "invalid";
}

function parseCsvNumber(value: string): number | null | "invalid" {
	const text = value.trim();
	if (!text) return null;
	const normalized = text.includes(".") ? text : text.replace(",", ".");
	const number = Number(normalized);
	return Number.isFinite(number) ? number : "invalid";
}

const AXES = ["x", "y", "z"] as const;

export interface CsvImportPlanInput {
	rows: readonly (readonly string[])[];
	/** 1-based spreadsheet row number of each data row, for operator-facing messages. */
	lines: readonly number[];
	assignments: CsvColumnAssignments;
	sourceTypes: readonly CsvSourceType[];
	mappings: Readonly<Record<string, CsvTypeMapping>>;
	existing: readonly PatchedFixture[];
	positionUnit: CsvPositionUnit;
	conflictPolicy: CsvConflictPolicy;
}

export function planCsvImport(input: CsvImportPlanInput): CsvImportRowPlan[] {
	const sourceByKey = new Map(input.sourceTypes.map((item) => [item.key, item]));
	const usedNumbers = new Set(
		input.existing.flatMap((fixture) =>
			fixture.fixture_number == null ? [] : [fixture.fixture_number],
		),
	);
	const usedVirtual = new Set(
		input.existing.flatMap((fixture) =>
			fixture.virtual_fixture_number == null
				? []
				: [fixture.virtual_fixture_number],
		),
	);
	const fileNumbers = new Map<string, number>();
	const plans = input.rows.map((row, index) =>
		planRow(row, input.lines[index] ?? index + 1, input, sourceByKey),
	);

	// Explicit IDs are reserved first so rows without an ID never take one the file names later.
	for (const plan of plans) {
		if (plan.status === "error" || plan.status === "skipped") continue;
		const id = plan.fixtureNumber ?? plan.virtualFixtureNumber;
		if (id == null) continue;
		const virtual = plan.virtualFixtureNumber != null;
		const idKey = `${virtual ? "0." : ""}${id}`;
		const used = virtual ? usedVirtual : usedNumbers;
		const earlier = fileNumbers.get(idKey);
		if (earlier != null)
			markError(plan, `Fixture ID ${idKey} is already used on row ${earlier}.`);
		else if (used.has(id))
			markError(plan, `Fixture ID ${idKey} is already used in the show.`);
		else {
			fileNumbers.set(idKey, plan.line);
			used.add(id);
		}
	}
	for (const plan of plans) {
		if (plan.status === "error" || plan.status === "skipped" || !plan.definition)
			continue;
		if (plan.fixtureNumber != null || plan.virtualFixtureNumber != null) continue;
		const virtual = isVisualOnly(plan.definition);
		const used = virtual ? usedVirtual : usedNumbers;
		const next = nextAvailableFixtureNumber(1, used);
		if (next == null) {
			markError(plan, "No free fixture ID is available.");
			continue;
		}
		used.add(next);
		if (virtual) plan.virtualFixtureNumber = next;
		else plan.fixtureNumber = next;
	}

	const occupied: Array<{ universe: number; start: number; end: number; line: number }> = [];
	for (const plan of plans) {
		if (!plan.name && plan.definition)
			plan.name = `${plan.definition.name || plan.definition.model} ${
				plan.virtualFixtureNumber != null
					? `0.${plan.virtualFixtureNumber}`
					: plan.fixtureNumber
			}`;
		if (plan.status === "error" || plan.status === "skipped") continue;
		if (!plan.patch || !plan.definition) continue;
		const footprint = definitionSplits(plan.definition)[0]?.footprint ?? 0;
		const start = plan.patch.address;
		const end = start + Math.max(footprint, 1) - 1;
		const clash =
			conflicts(
				input.existing as PatchedFixture[],
				plan.patch.universe,
				start,
				Math.max(footprint, 1),
			)[0] ?? null;
		const fileClash = occupied.find(
			(range) =>
				range.universe === plan.patch?.universe &&
				range.start <= end &&
				start <= range.end,
		);
		if (!clash && !fileClash) {
			occupied.push({ universe: plan.patch.universe, start, end, line: plan.line });
			continue;
		}
		const reason = clash
			? `${plan.patch.universe}.${start} overlaps ${clash.name || clash.definition.name} in the show`
			: `${plan.patch.universe}.${start} overlaps row ${fileClash?.line}`;
		if (input.conflictPolicy === "skip") markError(plan, `${reason}.`);
		else {
			plan.patch = null;
			plan.status = "unpatched";
			plan.message = `${reason}; imported unpatched.`;
		}
	}
	return plans;
}

function markError(plan: CsvImportRowPlan, message: string) {
	plan.status = "error";
	plan.message = message;
}

function planRow(
	row: readonly string[],
	line: number,
	input: CsvImportPlanInput,
	sourceByKey: ReadonlyMap<string, CsvSourceType>,
): CsvImportRowPlan {
	const { assignments } = input;
	const sourceTypeKey = csvSourceTypeKey(
		cell(row, assignments, "manufacturer"),
		cell(row, assignments, "fixture_type"),
		cell(row, assignments, "mode"),
	);
	const plan: CsvImportRowPlan = {
		line,
		sourceTypeKey,
		definition: null,
		fixtureNumber: null,
		virtualFixtureNumber: null,
		name: cell(row, assignments, "name"),
		patch: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		status: "ready",
		message: null,
	};
	const mapping =
		input.mappings[sourceTypeKey] ?? sourceByKey.get(sourceTypeKey)?.exactMatch;
	if (mapping === "skip") {
		plan.status = "skipped";
		plan.message = "Fixture type skipped.";
		return plan;
	}
	if (!mapping) {
		markError(plan, "No library fixture is selected for this fixture type.");
		return plan;
	}
	plan.definition = mapping;

	const idText = cell(row, assignments, "fixture_id");
	if (idText) {
		if (isVisualOnly(mapping)) {
			plan.virtualFixtureNumber = parseVirtualFixtureNumber(idText);
			if (plan.virtualFixtureNumber == null) {
				markError(plan, `Venue fixture ID “${idText}” must use 0.x.`);
				return plan;
			}
		} else {
			plan.fixtureNumber = parseFixtureNumber(idText);
			if (plan.fixtureNumber == null) {
				markError(
					plan,
					`Fixture ID “${idText}” is not a positive whole number.`,
				);
				return plan;
			}
		}
	}

	const patchText = cell(row, assignments, "patch");
	const patch = parseCsvPatch(patchText);
	if (patch === "invalid") {
		markError(plan, `Patch “${patchText}” is not a DMX address such as 1.101.`);
		return plan;
	}
	if (patch && !isDmxPatchable(mapping)) {
		plan.message = "No DMX patch applies to this fixture; the address was ignored.";
	} else if (patch) {
		const footprint = definitionSplits(mapping)[0]?.footprint ?? 0;
		if (patch.address + Math.max(footprint, 1) - 1 > 512) {
			markError(
				plan,
				`${patch.universe}.${patch.address} leaves no room for ${footprint} channels in the universe.`,
			);
			return plan;
		}
		plan.patch = patch;
		if (definitionSplits(mapping).length > 1)
			plan.message = "Only the first split is patched; the other splits stay unpatched.";
	}

	const scale = input.positionUnit === "m" ? 1000 : 1;
	for (const axis of AXES) {
		const location = parseCsvNumber(cell(row, assignments, axis));
		const rotation = parseCsvNumber(
			cell(row, assignments, `rot_${axis}` as CsvImportField),
		);
		if (location === "invalid" || rotation === "invalid") {
			markError(
				plan,
				`${location === "invalid" ? axis.toUpperCase() : `Rot${axis.toUpperCase()}`} is not a number.`,
			);
			return plan;
		}
		plan.location[axis] = (location ?? 0) * scale;
		plan.rotation[axis] = rotation ?? 0;
	}
	return plan;
}

export function csvImportCandidates(
	plans: readonly CsvImportRowPlan[],
	layerId: string,
): PatchFixtureCandidate[] {
	return plans.flatMap((plan) => {
		if (
			!plan.definition ||
			(plan.status !== "ready" && plan.status !== "unpatched")
		)
			return [];
		const definition = plan.definition;
		return [
			newPatchFixtureCandidate({
				name: plan.name,
				fixture_number: plan.fixtureNumber,
				virtual_fixture_number: plan.virtualFixtureNumber,
				definition,
				universe: plan.patch?.universe ?? null,
				address: plan.patch?.address ?? null,
				split_patches: definitionSplits(definition).map((split, index) => ({
					split: split.number,
					universe: index === 0 ? (plan.patch?.universe ?? null) : null,
					address: index === 0 ? (plan.patch?.address ?? null) : null,
				})),
				layer_id: layerId,
				location: plan.location,
				rotation: plan.rotation,
			}),
		];
	});
}
