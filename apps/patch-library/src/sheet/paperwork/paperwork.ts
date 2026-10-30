/**
 * Lighting paperwork built from the patch: the same fixtures, positions and addresses the plot
 * draws, as the tables a crew hangs and focuses from.
 *
 * Every report is plain rows, so the dialog, CSV and print all read one source. Venue objects are
 * scenery, not instruments, and stay out. An unpatched fixture is still an instrument: it hangs,
 * so it is listed and marked Unpatched.
 */

import type { GelAssignment, PatchedFixture as WirePatchedFixture } from "../../wire";

/** The fields paperwork reads, which every product's patch fixture carries. */
export type PatchedFixture = Pick<
	WirePatchedFixture,
	| "fixture_id"
	| "fixture_number"
	| "virtual_fixture_number"
	| "name"
	| "universe"
	| "address"
	| "split_patches"
	| "location"
	| "position_master"
	| "shaper_angle"
	| "installed_appearance"
> & { definition: Pick<WirePatchedFixture["definition"], "manufacturer" | "name" | "mode" | "footprint"> };

export type PaperworkKind =
	| "channel_hookup"
	| "instrument_schedule"
	| "dmx_sheet"
	| "color_pull"
	| "accessory_pull"
	| "symbol_key";

export const PAPERWORK_REPORTS: ReadonlyArray<{ kind: PaperworkKind; title: string; description: string }> = [
	{ kind: "channel_hookup", title: "Channel Hookup", description: "Every instrument by fixture ID." },
	{ kind: "instrument_schedule", title: "Instrument Schedule", description: "By hanging position, then unit number." },
	{ kind: "dmx_sheet", title: "DMX Sheet", description: "Every patched split by universe and address." },
	{ kind: "color_pull", title: "Color Pull List", description: "Gels to cut, with counts and fixtures." },
	{ kind: "accessory_pull", title: "Accessory Pull List", description: "Shapers and manual optics to fit." },
	{ kind: "symbol_key", title: "Symbol Key", description: "Each fixture type with its count." },
];

export interface PaperworkReport {
	kind: PaperworkKind;
	title: string;
	columns: string[];
	rows: string[][];
}

export const UNPATCHED = "Unpatched";
const NO_POSITION = "No position";

interface Instrument {
	fixture: PatchedFixture;
	id: string;
	number: number;
	type: string;
	mode: string;
	position: string;
	unit: number;
	dmx: string;
	gel: string;
	accessories: string[];
}

function fixtureId(fixture: PatchedFixture) {
	return fixture.fixture_number == null ? "—" : String(fixture.fixture_number);
}

function splits(fixture: PatchedFixture) {
	return fixture.split_patches?.length
		? fixture.split_patches
		: [{ split: 1, universe: fixture.universe, address: fixture.address }];
}

function dmxLabel(fixture: PatchedFixture) {
	const patched = splits(fixture).filter((split) => split.universe != null && split.address != null);
	return patched.length ? patched.map((split) => `${split.universe}.${split.address}`).join(", ") : UNPATCHED;
}

export function gelLabel(gel: GelAssignment | undefined) {
	if (!gel || gel.type === "open_white") return "Open white";
	if (gel.type === "built_in") return `${gel.embedded_fallback.number} ${gel.embedded_fallback.name}`.trim();
	return gel.name;
}

function accessories(fixture: PatchedFixture) {
	const list: string[] = [];
	if (fixture.shaper_angle != null) list.push("Shaper module");
	const optics = fixture.installed_appearance?.manual_optics;
	if (optics?.zoom_degrees != null) list.push(`Lens ${optics.zoom_degrees}°`);
	if (optics?.shaper_blades?.some((blade) => blade !== 0)) list.push("Shutter cuts");
	return list;
}

/** Instruments only: placed lights and devices, never Venue scenery. */
export function instruments(fixtures: readonly PatchedFixture[]): Instrument[] {
	const byId = new Map(fixtures.map((fixture) => [fixture.fixture_id, fixture]));
	const lights = fixtures.filter((fixture) => fixture.virtual_fixture_number == null);
	const positionOf = (fixture: PatchedFixture) => {
		const master = fixture.position_master ? byId.get(fixture.position_master) : undefined;
		if (!master) return NO_POSITION;
		return master.name || (master.virtual_fixture_number != null ? `0.${master.virtual_fixture_number}` : fixtureId(master));
	};
	// Units count along a position from stage left (lowest x) as a crew reads a pipe.
	const units = new Map<string, number>();
	const byPosition = new Map<string, PatchedFixture[]>();
	for (const fixture of lights) {
		const position = positionOf(fixture);
		byPosition.set(position, [...(byPosition.get(position) ?? []), fixture]);
	}
	for (const [, members] of byPosition)
		[...members]
			.sort((left, right) => (left.location?.x ?? 0) - (right.location?.x ?? 0) || (left.fixture_number ?? 0) - (right.fixture_number ?? 0))
			.forEach((fixture, index) => units.set(fixture.fixture_id, index + 1));
	return lights.map((fixture) => ({
		fixture,
		id: fixtureId(fixture),
		number: fixture.fixture_number ?? Number.MAX_SAFE_INTEGER,
		type: `${fixture.definition.manufacturer} ${fixture.definition.name}`.trim(),
		mode: fixture.definition.mode,
		position: positionOf(fixture),
		unit: units.get(fixture.fixture_id) ?? 0,
		dmx: dmxLabel(fixture),
		gel: gelLabel(fixture.installed_appearance?.gel),
		accessories: accessories(fixture),
	}));
}

const naturally = (left: string, right: string) => left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" });

export function buildPaperwork(kind: PaperworkKind, fixtures: readonly PatchedFixture[]): PaperworkReport {
	const title = PAPERWORK_REPORTS.find((report) => report.kind === kind)?.title ?? kind;
	const list = instruments(fixtures);
	switch (kind) {
		case "channel_hookup":
			return {
				kind,
				title,
				columns: ["Fixture", "Name", "Type", "Mode", "DMX", "Position", "Unit", "Color"],
				rows: [...list]
					.sort((left, right) => left.number - right.number || naturally(left.fixture.name ?? "", right.fixture.name ?? ""))
					.map((item) => [item.id, item.fixture.name ?? "", item.type, item.mode, item.dmx, item.position, String(item.unit), item.gel]),
			};
		case "instrument_schedule":
			return {
				kind,
				title,
				columns: ["Position", "Unit", "Fixture", "Type", "Mode", "DMX", "Color", "Accessories"],
				rows: [...list]
					.sort((left, right) => Number(left.position === NO_POSITION) - Number(right.position === NO_POSITION) || naturally(left.position, right.position) || left.unit - right.unit)
					.map((item) => [item.position, String(item.unit), item.id, item.type, item.mode, item.dmx, item.gel, item.accessories.join(", ")]),
			};
		case "dmx_sheet": {
			const rows = list.flatMap((item) =>
				splits(item.fixture).map((split) => ({ item, split })),
			);
			return {
				kind,
				title,
				columns: ["Universe", "Address", "End", "Fixture", "Type", "Mode", "Split"],
				rows: rows
					.sort((left, right) => {
						const unpatched = (row: (typeof rows)[number]) => row.split.universe == null || row.split.address == null;
						return Number(unpatched(left)) - Number(unpatched(right)) || (left.split.universe ?? 0) - (right.split.universe ?? 0) || (left.split.address ?? 0) - (right.split.address ?? 0) || left.item.number - right.item.number;
					})
					.map(({ item, split }) => {
						const patched = split.universe != null && split.address != null;
						const footprint = item.fixture.definition.footprint;
						return [
							patched ? String(split.universe) : UNPATCHED,
							patched ? String(split.address) : "",
							patched && footprint > 0 ? String((split.address ?? 0) + footprint - 1) : "",
							item.id,
							item.type,
							item.mode,
							String(split.split),
						];
					}),
			};
		}
		case "color_pull": {
			const gels = new Map<string, Instrument[]>();
			for (const item of list) if (item.gel !== "Open white") gels.set(item.gel, [...(gels.get(item.gel) ?? []), item]);
			return {
				kind,
				title,
				columns: ["Color", "Count", "Fixtures"],
				rows: [...gels]
					.sort(([left], [right]) => naturally(left, right))
					.map(([gel, items]) => [gel, String(items.length), items.map((item) => item.id).sort(naturally).join(", ")]),
			};
		}
		case "accessory_pull": {
			const fitted = new Map<string, Instrument[]>();
			for (const item of list) for (const accessory of item.accessories) fitted.set(accessory, [...(fitted.get(accessory) ?? []), item]);
			return {
				kind,
				title,
				columns: ["Accessory", "Count", "Fixtures"],
				rows: [...fitted]
					.sort(([left], [right]) => naturally(left, right))
					.map(([accessory, items]) => [accessory, String(items.length), items.map((item) => item.id).sort(naturally).join(", ")]),
			};
		}
		case "symbol_key": {
			const types = new Map<string, Instrument[]>();
			for (const item of list) types.set(item.type, [...(types.get(item.type) ?? []), item]);
			return {
				kind,
				title,
				columns: ["Type", "Count", "Unpatched", "Modes"],
				rows: [...types]
					.sort(([left], [right]) => naturally(left, right))
					.map(([type, items]) => [
						type,
						String(items.length),
						String(items.filter((item) => item.dmx === UNPATCHED).length),
						[...new Set(items.map((item) => item.mode))].sort(naturally).join(", "),
					]),
			};
		}
	}
}

/** RFC 4180 CSV with a header row; every field quoted where it needs to be. */
export function paperworkCsv(report: PaperworkReport) {
	const field = (value: string) => (/[",\r\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value);
	return [report.columns, ...report.rows].map((row) => row.map(field).join(",")).join("\r\n") + "\r\n";
}
