import type {
	AttributeValue,
	Cue,
	CueList,
	PresetValueReference,
} from "../../api/types/playback";

/**
 * What one cell of the Track Sheet says about a fixture attribute in one Cue.
 *
 * - `hard`, `up`, `down`: the Cue stores a new value (up/down when an intensity moves).
 * - `block`: the Cue stores the value it would have tracked anyway.
 * - `released`: the Cue stores Release; the Cuelist stops supplying the attribute.
 * - `restore`: the desk-generated restore after a Cue-only Cue.
 * - `tracked`: an earlier Cue's value continues here.
 * - `empty`: the Cuelist supplies nothing here.
 */
export type TrackCellState =
	| "hard"
	| "up"
	| "down"
	| "block"
	| "released"
	| "restore"
	| "tracked"
	| "empty";

export type TrackFamily =
	| "intensity"
	| "color"
	| "position"
	| "beam"
	| "focus"
	| "other";

export const TRACK_FAMILIES: readonly TrackFamily[] = [
	"intensity",
	"color",
	"position",
	"beam",
	"focus",
	"other",
];

export const TRACK_FAMILY_LABELS: Record<TrackFamily, string> = {
	intensity: "Dimmer",
	color: "Color",
	position: "Position",
	beam: "Beam",
	focus: "Focus",
	other: "Other",
};

/** A Cue that stores something for one address. */
export interface TrackEvent {
	cue: number;
	state: Exclude<TrackCellState, "tracked" | "empty">;
	value: AttributeValue | null;
	preset: PresetValueReference | null;
	/** The value was stored on a Group and reaches this fixture as a member. */
	groupId: string | null;
	/** The storing Cue is a Cue-only Cue, so its value ends at the next Cue. */
	cueOnly: boolean;
}

export interface TrackRow {
	key: string;
	fixtureId: string;
	attribute: string;
	family: TrackFamily;
	events: TrackEvent[];
}

export interface TrackCell {
	state: TrackCellState;
	value: AttributeValue | null;
	preset: PresetValueReference | null;
	groupId: string | null;
	cueOnly: boolean;
	/** The Cue the shown value comes from; null when nothing is supplied. */
	sourceCue: number | null;
}

export interface TrackSheet {
	cues: readonly Cue[];
	rows: TrackRow[];
}

export interface TrackSheetContext {
	/** Ordered members of each Group, used to show Group-stored values per fixture. */
	groupMembers: ReadonlyMap<string, readonly string[]>;
	/** Attribute id to its registry family. */
	attributeFamily: (attribute: string) => string | undefined;
	/** Fixture order for rows; fixtures not listed follow in id order. */
	fixtureOrder?: readonly string[];
}

export function trackFamily(
	attribute: string,
	registryFamily: string | undefined,
): TrackFamily {
	const family = registryFamily ?? attribute;
	if (family === "intensity" || attribute === "dimmer") return "intensity";
	if (family === "color") return "color";
	if (family === "position" || attribute === "pan" || attribute === "tilt")
		return "position";
	if (family === "beam" || family === "shapers") return "beam";
	if (family === "focus") return "focus";
	return "other";
}

export const addressKey = (fixtureId: string, attribute: string) =>
	`${fixtureId}\u0000${attribute}`;

function sameValue(
	left: AttributeValue | null,
	right: AttributeValue | null,
	leftPreset: PresetValueReference | null,
	rightPreset: PresetValueReference | null,
) {
	if ((leftPreset?.preset_instance_id ?? null) !== (rightPreset?.preset_instance_id ?? null))
		return false;
	return JSON.stringify(left) === JSON.stringify(right);
}

function intensityLevel(value: AttributeValue | null) {
	return value?.kind === "normalized" ? value.value : null;
}

interface Stored {
	value: AttributeValue | null;
	preset: PresetValueReference | null;
	automatic: boolean;
	groupId: string | null;
}

/**
 * Builds the sheet in one pass over the Cues, the order sequential playback applies them:
 * a preceding Cue-only restore first, then Group values, then fixture values, which win.
 */
export function buildTrackSheet(
	list: Pick<CueList, "cues">,
	context: TrackSheetContext,
): TrackSheet {
	const rows = new Map<string, TrackRow>();
	const current = new Map<string, Stored>();
	const rowFor = (fixtureId: string, attribute: string) => {
		const key = addressKey(fixtureId, attribute);
		let row = rows.get(key);
		if (!row) {
			row = {
				key,
				fixtureId,
				attribute,
				family: trackFamily(attribute, context.attributeFamily(attribute)),
				events: [],
			};
			rows.set(key, row);
		}
		return row;
	};
	list.cues.forEach((cue, index) => {
		const stored = new Map<string, Stored & { fixtureId: string; attribute: string }>();
		for (const change of cue.group_changes ?? []) {
			for (const fixtureId of context.groupMembers.get(change.group_id) ?? []) {
				stored.set(addressKey(fixtureId, change.attribute), {
					fixtureId,
					attribute: change.attribute,
					value: change.value,
					preset: change.preset_reference ?? null,
					automatic: Boolean(change.automatic_restore),
					groupId: change.group_id,
				});
			}
		}
		for (const change of cue.changes) {
			const key = addressKey(change.fixture_id, change.attribute);
			const previous = stored.get(key);
			// A desk-generated restore never hides a value the Cue stores itself.
			if (change.automatic_restore && previous && !previous.automatic) continue;
			stored.set(key, {
				fixtureId: change.fixture_id,
				attribute: change.attribute,
				value: change.value,
				preset: change.preset_reference ?? null,
				automatic: Boolean(change.automatic_restore),
				groupId: null,
			});
		}
		for (const [key, entry] of stored) {
			const before = current.get(key);
			const row = rowFor(entry.fixtureId, entry.attribute);
			let state: TrackEvent["state"];
			if (entry.automatic) state = "restore";
			else if (entry.value === null) state = "released";
			else if (
				before?.value != null &&
				sameValue(before.value, entry.value, before.preset, entry.preset)
			)
				state = "block";
			else {
				const was = intensityLevel(before?.value ?? null);
				const now = intensityLevel(entry.value);
				state =
					row.family === "intensity" && was !== null && now !== null
						? now > was
							? "up"
							: now < was
								? "down"
								: "hard"
						: "hard";
			}
			row.events.push({
				cue: index,
				state,
				value: entry.value,
				preset: entry.preset,
				groupId: entry.groupId,
				cueOnly: Boolean(cue.cue_only) && !entry.automatic,
			});
			current.set(key, entry);
		}
	});
	const order = new Map(
		(context.fixtureOrder ?? []).map((fixtureId, index) => [fixtureId, index]),
	);
	const familyIndex = (family: TrackFamily) => TRACK_FAMILIES.indexOf(family);
	const sorted = [...rows.values()].sort(
		(left, right) =>
			(order.get(left.fixtureId) ?? Number.MAX_SAFE_INTEGER) -
				(order.get(right.fixtureId) ?? Number.MAX_SAFE_INTEGER) ||
			left.fixtureId.localeCompare(right.fixtureId, undefined, { numeric: true }) ||
			familyIndex(left.family) - familyIndex(right.family) ||
			left.attribute.localeCompare(right.attribute),
	);
	return { cues: list.cues, rows: sorted };
}

/** The last event at or before `cue`, found by binary search over the row's sorted events. */
function eventAtOrBefore(row: TrackRow, cue: number): TrackEvent | null {
	let low = 0;
	let high = row.events.length - 1;
	let found: TrackEvent | null = null;
	while (low <= high) {
		const middle = (low + high) >> 1;
		const event = row.events[middle];
		if (event.cue <= cue) {
			found = event;
			low = middle + 1;
		} else high = middle - 1;
	}
	return found;
}

export function trackCell(row: TrackRow, cue: number): TrackCell {
	const event = eventAtOrBefore(row, cue);
	if (!event)
		return {
			state: "empty",
			value: null,
			preset: null,
			groupId: null,
			cueOnly: false,
			sourceCue: null,
		};
	if (event.cue === cue)
		return {
			state: event.state,
			value: event.value,
			preset: event.preset,
			groupId: event.groupId,
			cueOnly: event.cueOnly,
			sourceCue: event.value === null ? null : cue,
		};
	if (event.value === null)
		return {
			state: "empty",
			value: null,
			preset: null,
			groupId: null,
			cueOnly: false,
			sourceCue: null,
		};
	return {
		state: "tracked",
		value: event.value,
		preset: event.preset,
		groupId: event.groupId,
		cueOnly: false,
		sourceCue: event.cue,
	};
}

export interface TrackSheetFilter {
	fixtures: ReadonlySet<string> | null;
	family: TrackFamily | "all";
	changesOnly: boolean;
}

export interface TrackSheetView {
	rows: TrackRow[];
	/** Indexes into `sheet.cues` of the Cue columns shown. */
	columns: number[];
}

/** Applies the fixture, family and "changes only" filters. */
export function filterTrackSheet(
	sheet: TrackSheet,
	filter: TrackSheetFilter,
): TrackSheetView {
	const rows = sheet.rows.filter(
		(row) =>
			(!filter.fixtures || filter.fixtures.has(row.fixtureId)) &&
			(filter.family === "all" || row.family === filter.family),
	);
	const all = sheet.cues.map((_, index) => index);
	if (!filter.changesOnly) return { rows, columns: all };
	const changed = new Set<number>();
	for (const row of rows) for (const event of row.events) changed.add(event.cue);
	return {
		rows: rows.filter((row) => row.events.length > 0),
		columns: all.filter((index) => changed.has(index)),
	};
}
