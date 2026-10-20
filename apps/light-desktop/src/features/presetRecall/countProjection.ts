import type { AttributeValue } from "../../api/types/playback";
import type { ProgrammerValuesProjection } from "../programmerValues/contracts";
import { sameAttributeValue } from "../programmerValues/projectionValue";
import {
	presetFixtureTargets,
	type ResolvedValueIndex,
} from "../presetRecording/presetFixtureCounts";
import type { PresetCountFeedback } from "./countFeedback";

/** Reuse the pool's display projection, in authoritative Programmer LTP order. Never writes values. */
function fixtureValues(
	projection: ProgrammerValuesProjection,
	groups: ReadonlyMap<string, readonly string[]>,
) {
	const result = new Map<string, Map<string, AttributeValue>>();
	const entries = [
		...projection.groupValues.map((entry) => ({
			order: entry.programmerOrder,
			targets: presetFixtureTargets(
				{
					values: {},
					group_values: { [entry.groupId]: { [entry.attribute]: entry.value } },
				},
				groups,
			),
		})),
		...projection.fixtureValues.map((entry) => ({
			order: entry.programmerOrder,
			targets: new Map([
				[
					entry.fixtureId.toLowerCase(),
					new Map([[entry.attribute, entry.value]]),
				],
			]),
		})),
	].sort((a, b) => a.order - b.order);
	for (const { targets } of entries)
		for (const [id, attributes] of targets) {
			const current = result.get(id) ?? new Map<string, AttributeValue>();
			for (const [attribute, value] of attributes)
				current.set(attribute, value);
			result.set(id, current);
		}
	return result;
}

export function recalledCountValues(
	feedback: PresetCountFeedback,
	groups: ReadonlyMap<string, readonly string[]>,
): ResolvedValueIndex {
	const before = fixtureValues(feedback.before, groups),
		after = fixtureValues(feedback.after, groups);
	const changed = new Map<string, Map<string, AttributeValue>>();
	for (const [id, attributes] of after)
		for (const [attribute, value] of attributes) {
			const previous = before.get(id)?.get(attribute);
			if (previous && sameAttributeValue(previous, value)) continue;
			const current = changed.get(id) ?? new Map<string, AttributeValue>();
			current.set(attribute, value);
			changed.set(id, current);
		}
	return changed;
}
export function withRecalledCountValues(
	resolved: ResolvedValueIndex,
	changed: ResolvedValueIndex,
): ResolvedValueIndex {
	if (changed.size === 0) return resolved;
	const result = new Map(resolved);
	for (const [id, attributes] of changed) {
		const current = new Map(result.get(id));
		for (const [attribute, value] of attributes) current.set(attribute, value);
		result.set(id, current);
	}
	return result;
}
