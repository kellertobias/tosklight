import type { CueList } from "../../api/types";

/** Merge this gesture's timing cells without replacing unrelated authoritative Cue edits. */
export function mergeCueTimingFields(
	body: CueList,
	expected: CueList,
	target: CueList,
): CueList {
	const keys = [
		"delay_millis",
		"fade_millis",
		"out_delay_millis",
		"out_fade_millis",
		"out_delay_link",
		"out_fade_link",
	] as const;
	let changed = false;
	const cues = [...body.cues];
	for (const before of expected.cues) {
		const after = target.cues.find((cue) => cue.id === before.id);
		if (!after)
			throw new Error("The Cue timing edit no longer matches this Cuelist.");
		const fields = keys.filter((key) => before[key] !== after[key]);
		if (!fields.length) continue;
		const index = cues.findIndex((cue) => cue.id === before.id);
		const current = cues[index];
		if (!current)
			throw new Error("A Cue in this timing edit no longer exists.");
		const next = { ...current };
		for (const key of fields) {
			if (current[key] !== before[key] && current[key] !== after[key])
				throw new Error(
					"Cue timings changed during this edit. The timing edit was not applied.",
				);
			if (current[key] === after[key]) continue;
			changed = true;
			if (after[key] === undefined) delete next[key];
			else Object.assign(next, { [key]: after[key] });
		}
		cues[index] = next;
	}
	return changed ? { ...body, cues } : body;
}
