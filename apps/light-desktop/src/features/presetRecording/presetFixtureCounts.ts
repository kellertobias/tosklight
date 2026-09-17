import type { StoredPreset, VisualizationSnapshot } from "../../api/types";
import type { AttributeValue } from "../../api/types/playback";
import { resolveSpread } from "../../components/control/parameterControls/parameterValueMutations";

/** How many fixtures a Preset defines, and how many currently show all of those values. */
export interface PresetFixtureCounts {
	active: number;
	defined: number;
}

export type ResolvedValueIndex = ReadonlyMap<
	string,
	ReadonlyMap<string, AttributeValue>
>;

const EMPTY_INDEX: ResolvedValueIndex = new Map();

/**
 * Indexes the merged abstract attribute values that feed DMX output, so a Preset tile can tell
 * whether the look it stores is what the stage currently shows.
 */
export function resolvedValueIndex(
	snapshot: VisualizationSnapshot | null,
): ResolvedValueIndex {
	if (!snapshot) return EMPTY_INDEX;
	const result = new Map<string, Map<string, AttributeValue>>();
	for (const value of snapshot.values) {
		const key = value.fixture_id.toLowerCase();
		let fixture = result.get(key);
		if (!fixture) {
			fixture = new Map();
			result.set(key, fixture);
		}
		fixture.set(value.attribute, value.value);
	}
	return result;
}

/**
 * The values a Preset applies per fixture. Group values resolve over the group's ordered
 * membership (spreads by position); fixture values override them, as recall does. An unpatched
 * fixture stays part of the definition.
 */
export function presetFixtureTargets(
	preset: Pick<StoredPreset, "values" | "group_values">,
	groupMembers: ReadonlyMap<string, readonly string[]>,
): Map<string, Map<string, AttributeValue>> {
	const targets = new Map<string, Map<string, AttributeValue>>();
	const target = (fixtureId: string) => {
		const key = fixtureId.toLowerCase();
		let values = targets.get(key);
		if (!values) {
			values = new Map();
			targets.set(key, values);
		}
		return values;
	};
	for (const [groupId, attributes] of Object.entries(
		preset.group_values ?? {},
	)) {
		const members = groupMembers.get(groupId) ?? [];
		for (const [attribute, raw] of Object.entries(attributes)) {
			const value = asAttributeValue(raw);
			if (!value) continue;
			const spread =
				value.kind === "spread"
					? resolveSpread(value.value, members.length)
					: null;
			members.forEach((fixtureId, index) => {
				target(fixtureId).set(
					attribute,
					spread
						? { kind: "normalized", value: spread[index] ?? 0 }
						: value,
				);
			});
		}
	}
	for (const [fixtureId, attributes] of Object.entries(preset.values)) {
		const values = target(fixtureId);
		for (const [attribute, raw] of Object.entries(attributes)) {
			const value = asAttributeValue(raw);
			if (value) values.set(attribute, value);
		}
	}
	for (const [fixtureId, values] of targets)
		if (values.size === 0) targets.delete(fixtureId);
	return targets;
}

export function presetFixtureCounts(
	preset: Pick<StoredPreset, "values" | "group_values">,
	resolved: ResolvedValueIndex,
	groupMembers: ReadonlyMap<string, readonly string[]>,
): PresetFixtureCounts {
	const targets = presetFixtureTargets(preset, groupMembers);
	let active = 0;
	for (const [fixtureId, values] of targets) {
		const current = resolved.get(fixtureId);
		if (!current) continue;
		let matches = true;
		for (const [attribute, value] of values) {
			const effective = current.get(attribute);
			if (!effective || !sameEffectiveValue(value, effective)) {
				matches = false;
				break;
			}
		}
		if (matches) active += 1;
	}
	return { active, defined: targets.size };
}

export function presetFixtureCountLabel(counts: PresetFixtureCounts) {
	return `${counts.active} / ${counts.defined}`;
}

function asAttributeValue(raw: unknown): AttributeValue | null {
	if (!raw || typeof raw !== "object") return null;
	const { kind, value } = raw as { kind?: unknown; value?: unknown };
	switch (kind) {
		case "normalized":
		case "raw_dmx":
		case "raw_dmx_exact":
			return typeof value === "number" ? (raw as AttributeValue) : null;
		case "discrete":
			return typeof value === "string" ? (raw as AttributeValue) : null;
		case "spread":
			return Array.isArray(value) ? (raw as AttributeValue) : null;
		case "color_xyz":
			return value && typeof value === "object" ? (raw as AttributeValue) : null;
		default:
			return null;
	}
}

function near(left: number, right: number) {
	return (
		Math.abs(left - right) <=
		1e-4 * Math.max(1, Math.abs(left), Math.abs(right))
	);
}

function sameEffectiveValue(stored: AttributeValue, effective: AttributeValue) {
	switch (stored.kind) {
		case "color_xyz":
			return (
				effective.kind === "color_xyz" &&
				near(stored.value.x, effective.value.x) &&
				near(stored.value.y, effective.value.y) &&
				near(stored.value.z, effective.value.z)
			);
		case "spread":
			return (
				effective.kind === "normalized" &&
				near(stored.value[0] ?? 0, effective.value)
			);
		case "discrete":
			return effective.kind === "discrete" && effective.value === stored.value;
		default:
			return (
				effective.kind === stored.kind && near(stored.value, effective.value)
			);
	}
}
