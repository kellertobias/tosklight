import { sameAttributeValue } from "../programmerValues/projectionValue";
import type { StoredPreset, VisualizationSnapshot } from "../../api/types";
import type { AttributeValue } from "../../api/types/playback";
import { resolveSpread } from "../../components/control/parameterControls/parameterValueMutations";

/** How many fixtures a Preset defines, and how many currently show all of those values. */
export interface PresetFixtureCounts {
	active: number;
	defined: number;
	/** Applies to whatever is selected rather than to a stored set of fixtures. */
	universal?: boolean;
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
						: value.kind === "group_family"
                            ? value.value.members?.[fixtureId] ?? value.value.template
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
	preset: Pick<StoredPreset, "values" | "group_values" | "universal_values">,
	resolved: ResolvedValueIndex,
	groupMembers: ReadonlyMap<string, readonly string[]>,
): PresetFixtureCounts {
	const universal = Object.entries(preset.universal_values ?? {}).flatMap(
		([attribute, raw]) => {
			const value = asAttributeValue(raw);
			return value ? [[attribute, value] as const] : [];
		},
	);
	if (universal.length > 0) {
		// A universal colour names no fixtures: count those currently showing it.
		let active = 0;
		for (const current of resolved.values())
			if (
				universal.every(([attribute, value]) => {
					const effective = current.get(attribute);
					return effective != null && sameEffectiveValue(value, effective);
				})
			)
				active += 1;
		return { active, defined: 0, universal: true };
	}
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

/** The tile shows only how many fixtures currently match this Preset. */
export function presetFixtureCountLabel(counts: PresetFixtureCounts) {
	return String(counts.active);
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
		case "group_family":
		case "color_program":
		case "position":
		case "zoom":
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
		case "group_family":
			return sameAttributeValue(stored, effective);
		case "color_program":
		case "position":
		case "zoom":
			// Requested semantic intent, never the achieved output: equal intents match even when
			// the stored and resolved spellings differ in float width.
			return effective.kind === stored.kind && sameIntent(stored.value, effective.value);
		default:
			return (
				effective.kind === stored.kind && near(stored.value, effective.value)
			);
	}
}

function sameIntent(stored: unknown, effective: unknown): boolean {
	if (typeof stored === "number" && typeof effective === "number")
		return near(stored, effective);
	if (Array.isArray(stored))
		return (
			Array.isArray(effective) &&
			stored.length === effective.length &&
			stored.every((item, index) => sameIntent(item, effective[index]))
		);
	if (!stored || typeof stored !== "object" || !effective || typeof effective !== "object")
		return stored === effective;
	const left = stored as Record<string, unknown>;
	const right = effective as Record<string, unknown>;
	const keys = Object.keys(left);
	return (
		keys.length === Object.keys(right).length &&
		keys.every((key) => key in right && sameIntent(left[key], right[key]))
	);
}
