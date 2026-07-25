import type { AttributeValue, VisualizationSnapshot } from "../../api/types";

export type CueVisualizationChange = {
	fixture_id: string;
	attribute: string;
	value: AttributeValue | null;
};

export function cueVisualization(
	base: VisualizationSnapshot | null,
	changes: CueVisualizationChange[],
) {
	const entries = new Map(
		(base?.values ?? []).map((entry) => [
			`${entry.fixture_id}\0${entry.attribute}`,
			entry,
		]),
	);
	for (const change of changes) {
		const key = `${change.fixture_id}\0${change.attribute}`;
		if (change.value) entries.set(key, { ...change, value: change.value });
		else entries.delete(key);
	}
	return {
		revision: base?.revision ?? 0,
		generated_at: new Date().toISOString(),
		grand_master: 1,
		blackout: false,
		values: [...entries.values()],
	} satisfies VisualizationSnapshot;
}
