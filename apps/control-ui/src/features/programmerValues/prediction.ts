import type { AttributeValue } from "../../api/types/playback";
import type {
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
	ProgrammerValuesCommand,
	ProgrammerValuesMutation,
	ProgrammerValuesProjection,
} from "./contracts";
import type { ProgrammerValuesOptimisticReducer } from "./store";

export function predictProgrammerValues(
	action: ProgrammerValuesCommand,
): ProgrammerValuesOptimisticReducer {
	if (action.action === "clear") return clearPrediction;
	const mutations = action.action === "batch" ? action.mutations : [action];
	return (current) => applyMutations(current, mutations);
}

function clearPrediction(current: ProgrammerValuesProjection) {
	if (!current.fixtureValues.length && !current.groupValues.length) return current;
	return { ...current, fixtureValues: [], groupValues: [] };
}

function applyMutations(
	current: ProgrammerValuesProjection,
	mutations: readonly ProgrammerValuesMutation[],
) {
	if (!mutations.length) return current;
	const fixtureValues = new Map(
		current.fixtureValues.map((value) => [fixtureKey(value), value]),
	);
	const groupValues = new Map(
		current.groupValues.map((value) => [groupKey(value), value]),
	);
	let nextOrder = greatestOrder(current) + 1;
	let changed = false;
	for (const mutation of mutations) {
		const result = applyMutation(
			mutation,
			fixtureValues,
			groupValues,
			nextOrder,
		);
		changed ||= result.changed;
		nextOrder += result.ordersUsed;
	}
	if (!changed) return current;
	return {
		...current,
		fixtureValues: [...fixtureValues.values()],
		groupValues: [...groupValues.values()],
	};
}

function applyMutation(
	mutation: ProgrammerValuesMutation,
	fixtures: Map<string, ProgrammerFixtureValue>,
	groups: Map<string, ProgrammerGroupValue>,
	programmerOrder: number,
) {
	switch (mutation.action) {
		case "set_fixture": {
			const key = fixtureKey(mutation);
			if (sameFixtureWrite(fixtures.get(key), mutation)) return unchanged;
			fixtures.set(key, fixtureWrite(mutation, programmerOrder));
			return changedWithOrder;
		}
		case "release_fixture":
			return deletion(fixtures.delete(fixtureKey(mutation)));
		case "set_group": {
			const key = groupKey(mutation);
			if (sameGroupWrite(groups.get(key), mutation)) return unchanged;
			groups.set(key, groupWrite(mutation, programmerOrder));
			return changedWithOrder;
		}
		case "release_group":
			return deletion(groups.delete(groupKey(mutation)));
	}
}

function fixtureWrite(
	mutation: Extract<ProgrammerValuesMutation, { action: "set_fixture" }>,
	programmerOrder: number,
): ProgrammerFixtureValue {
	return {
		fixtureId: mutation.fixtureId,
		attribute: mutation.attribute,
		value: mutation.value,
		programmerOrder,
		...mutation.timing,
	};
}

function groupWrite(
	mutation: Extract<ProgrammerValuesMutation, { action: "set_group" }>,
	programmerOrder: number,
): ProgrammerGroupValue {
	return {
		groupId: mutation.groupId,
		attribute: mutation.attribute,
		value: mutation.value,
		programmerOrder,
		...mutation.timing,
	};
}

const unchanged = { changed: false, ordersUsed: 0 } as const;
const changedWithOrder = { changed: true, ordersUsed: 1 } as const;

function deletion(changed: boolean) {
	return changed ? { changed: true, ordersUsed: 0 } : unchanged;
}

function sameFixtureWrite(
	current: ProgrammerFixtureValue | undefined,
	mutation: Extract<ProgrammerValuesMutation, { action: "set_fixture" }>,
) {
	return Boolean(
		current && sameWrite(current, mutation.value, mutation.timing),
	);
}

function sameGroupWrite(
	current: ProgrammerGroupValue | undefined,
	mutation: Extract<ProgrammerValuesMutation, { action: "set_group" }>,
) {
	return Boolean(
		current && sameWrite(current, mutation.value, mutation.timing),
	);
}

function sameWrite(
	current: ProgrammerFixtureValue | ProgrammerGroupValue,
	value: AttributeValue,
	timing: { fade: boolean; fadeMillis: number | null; delayMillis: number | null },
) {
	return (
		current.fade === timing.fade &&
		current.fadeMillis === timing.fadeMillis &&
		current.delayMillis === timing.delayMillis &&
		sameAttributeValue(current.value, value)
	);
}

function sameAttributeValue(left: AttributeValue, right: AttributeValue) {
	if (left.kind !== right.kind) return false;
	if (left.kind === "spread" && right.kind === "spread")
		return (
			left.value.length === right.value.length &&
			left.value.every((value, index) => value === right.value[index])
		);
	if (left.kind === "color_xyz" && right.kind === "color_xyz")
		return (
			left.value.x === right.value.x &&
			left.value.y === right.value.y &&
			left.value.z === right.value.z
		);
	return left.value === right.value;
}

function fixtureKey(value: { fixtureId: string; attribute: string }) {
	return `${value.fixtureId}\u0000${value.attribute}`;
}

function groupKey(value: { groupId: string; attribute: string }) {
	return `${value.groupId}\u0000${value.attribute}`;
}

function greatestOrder(projection: ProgrammerValuesProjection) {
	let greatest = 0;
	for (const value of projection.fixtureValues)
		greatest = Math.max(greatest, value.programmerOrder);
	for (const value of projection.groupValues)
		greatest = Math.max(greatest, value.programmerOrder);
	return Math.min(greatest, Number.MAX_SAFE_INTEGER - 1);
}
