import type { AttributeValue, Cue, CueList } from "../../api/types/playback";
import { addressKey, buildTrackSheet, type TrackSheetContext, trackCell } from "./trackSheetModel";

/**
 * How a Track Sheet edit is written. Each matches a documented command-line Update:
 *
 * - `cue`: the value goes into this Cue and tracks on (`UPDATE ALL` for this address).
 * - `source`: the value goes into the Cue the shown value comes from (`UPDATE TRACKED`).
 * - `cue_only`: the value goes into this Cue and the next Cue restores the previous tracked
 *   value, or releases it when nothing tracked before (a Cue-only record of this address).
 */
export type TrackEditMode = "cue" | "source" | "cue_only";

type FixtureChange = Cue["changes"][number];

function withChange(cue: Cue, change: FixtureChange): Cue {
	const index = cue.changes.findIndex(
		(item) =>
			!item.automatic_restore &&
			item.fixture_id === change.fixture_id &&
			item.attribute === change.attribute,
	);
	const changes = cue.changes.filter(
		(item, position) =>
			position === index ||
			!(
				item.automatic_restore &&
				item.fixture_id === change.fixture_id &&
				item.attribute === change.attribute
			),
	);
	if (index < 0) return { ...cue, changes: [...changes, change] };
	const kept = changes.indexOf(cue.changes[index]);
	// An in-place change keeps its own timing; the stored value replaces any Preset reference.
	changes[kept] = {
		...cue.changes[index],
		value: change.value,
		preset_reference: null,
		replacement_projection: null,
	};
	return { ...cue, changes };
}

function replaceCue(list: CueList, index: number, cue: Cue): CueList {
	return { ...list, cues: list.cues.map((item, position) => (position === index ? cue : item)) };
}

export function editTrackValue(
	list: CueList,
	context: TrackSheetContext,
	edit: {
		cue: number;
		fixtureId: string;
		attribute: string;
		value: AttributeValue;
		mode: TrackEditMode;
	},
): CueList {
	const { fixtureId, attribute, value } = edit;
	const change: FixtureChange = { fixture_id: fixtureId, attribute, value };
	const sheet = buildTrackSheet(list, context);
	const row = sheet.rows.find((item) => item.key === addressKey(fixtureId, attribute));
	if (edit.mode === "source") {
		const source = row ? trackCell(row, edit.cue).sourceCue : null;
		if (source === null) throw new Error("This value is not supplied by an earlier Cue.");
		return replaceCue(list, source, withChange(list.cues[source], change));
	}
	let next = replaceCue(list, edit.cue, withChange(list.cues[edit.cue], change));
	if (edit.mode === "cue_only" && edit.cue + 1 < list.cues.length) {
		const following = next.cues[edit.cue + 1];
		const storesOwn = following.changes.some(
			(item) =>
				!item.automatic_restore &&
				item.fixture_id === fixtureId &&
				item.attribute === attribute,
		);
		if (!storesOwn) {
			const before = row && edit.cue > 0 ? trackCell(row, edit.cue - 1) : null;
			next = replaceCue(
				next,
				edit.cue + 1,
				withChange(following, {
					fixture_id: fixtureId,
					attribute,
					value: before?.value ?? null,
					...(before?.preset ? { preset_reference: before.preset } : {}),
				}),
			);
		}
	}
	return next;
}

/** Removes the value this Cue stores for one fixture attribute; tracking recalculates. */
export function removeTrackValue(
	list: CueList,
	edit: { cue: number; fixtureId: string; attribute: string },
): CueList {
	const cue = list.cues[edit.cue];
	const changes = cue.changes.filter(
		(item) =>
			item.automatic_restore ||
			item.fixture_id !== edit.fixtureId ||
			item.attribute !== edit.attribute,
	);
	if (changes.length === cue.changes.length)
		throw new Error("This Cue does not store its own value for this fixture.");
	return replaceCue(list, edit.cue, { ...cue, changes });
}
