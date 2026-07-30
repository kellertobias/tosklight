import type { PatchedFixture, VisualizationSnapshot } from "../api/types";

type IndexedNormalizedValue = {
	index: number;
	value: number;
};

const visualizationValueIndexes = new WeakMap<
	VisualizationSnapshot,
	ReadonlyMap<string, ReadonlyMap<string, IndexedNormalizedValue>>
>();
const profileOutputValueIndexes = new WeakMap<
	VisualizationSnapshot,
	ReadonlyMap<string, ReadonlyMap<string, IndexedNormalizedValue>>
>();
const fixtureDefaultIndexes = new WeakMap<
	object,
	ReadonlyMap<string, number>
>();

export function fixtureTargetIds(fixture: PatchedFixture) {
	return [
		fixture.fixture_id,
		...fixture.logical_heads.map((head) => head.fixture_id),
	];
}

export function fixtureDefault(
	fixture: PatchedFixture,
	attribute: string,
	fallback = 0,
) {
	let defaults = fixtureDefaultIndexes.get(fixture.definition);
	if (!defaults) {
		const indexed = new Map<string, number>();
		for (const head of fixture.definition.heads) {
			for (const parameter of head.parameters) {
				if (!indexed.has(parameter.attribute))
					indexed.set(parameter.attribute, parameter.default);
			}
		}
		defaults = indexed;
		fixtureDefaultIndexes.set(fixture.definition, defaults);
	}
	return defaults.get(attribute) ?? fallback;
}

export function fixtureValue(
	snapshot: VisualizationSnapshot | null,
	fixture: PatchedFixture,
	attribute: string,
	fallback = 0,
) {
	if (snapshot) {
		const values = visualizationValueIndex(snapshot).get(attribute);
		let live: IndexedNormalizedValue | undefined;
		for (const fixtureId of fixtureTargetIds(fixture)) {
			const candidate = values?.get(fixtureId);
			if (candidate && (!live || candidate.index < live.index))
				live = candidate;
		}
		if (live) return live.value;
	}
	return fixtureDefault(fixture, attribute, fallback);
}

export function fixtureProfileOutputValue(
	snapshot: VisualizationSnapshot | null,
	fixture: PatchedFixture,
	attribute: string,
) {
	if (!snapshot?.profile_output_values) return undefined;
	const values = profileOutputValueIndex(snapshot).get(attribute);
	let live: IndexedNormalizedValue | undefined;
	for (const fixtureId of fixtureTargetIds(fixture)) {
		const candidate = values?.get(fixtureId);
		if (candidate && (!live || candidate.index < live.index)) live = candidate;
	}
	return live?.value;
}

function profileOutputValueIndex(snapshot: VisualizationSnapshot) {
	const cached = profileOutputValueIndexes.get(snapshot);
	if (cached) return cached;
	const indexed = indexNormalizedValues(snapshot.profile_output_values ?? []);
	profileOutputValueIndexes.set(snapshot, indexed);
	return indexed;
}

function visualizationValueIndex(snapshot: VisualizationSnapshot) {
	const cached = visualizationValueIndexes.get(snapshot);
	if (cached) return cached;
	const indexed = indexNormalizedValues(snapshot.values);
	visualizationValueIndexes.set(snapshot, indexed);
	return indexed;
}

function indexNormalizedValues(
	entries: NonNullable<VisualizationSnapshot["values"]>,
) {
	const indexed = new Map<string, Map<string, IndexedNormalizedValue>>();
	for (const [index, entry] of entries.entries()) {
		if (entry.value.kind !== "normalized") continue;
		let attributeValues = indexed.get(entry.attribute);
		if (!attributeValues) {
			attributeValues = new Map();
			indexed.set(entry.attribute, attributeValues);
		}
		if (!attributeValues.has(entry.fixture_id))
			attributeValues.set(entry.fixture_id, {
				index,
				value: entry.value.value,
			});
	}
	return indexed;
}
