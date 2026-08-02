import type { AttributeDescriptor, VisualizationSnapshot } from "../api/types";
import type { AttributeValue } from "../api/types/playback";
import type { ValueSource } from "../types";
import type { FixtureSheetTarget } from "./fixtureSheetTargets";
import { targetDefault, targetHasAttribute } from "./fixtureSheetTargets";

export const FIXTURE_SHEET_ATTRIBUTE_GROUPS = [
	"intensity",
	"color",
	"position",
	"beam",
	"shapers",
	"focus",
	"control",
	"media",
] as const;

export type FixtureSheetAttributeGroup =
	(typeof FIXTURE_SHEET_ATTRIBUTE_GROUPS)[number];
export type FixtureSheetDynamicEntry = NonNullable<
	VisualizationSnapshot["dynamic_stack"]
>[number];

export interface FixtureSheetDynamicIdentity {
	lane: "normal" | "preload";
	attribute: string;
	label: string;
	accessibleName: string;
	poolNumber: number | null;
	dynamicId: string | null;
	paused: boolean;
	pending: boolean;
	hidden: boolean;
	winning: boolean;
}

export interface FixtureSheetMemberValue {
	attribute: string;
	label: string;
	value: AttributeValue;
	text: string;
	preloadValue: AttributeValue | null;
	preloadText: string | null;
	source: ValueSource;
	dynamics: FixtureSheetDynamicIdentity[];
}

export interface FixtureSheetGroupValue {
	id: FixtureSheetAttributeGroup;
	members: FixtureSheetMemberValue[];
	available: boolean;
	source: ValueSource;
	accessibleName: string;
}

export type FixtureSheetGroupValues = Record<
	FixtureSheetAttributeGroup,
	FixtureSheetGroupValue
>;

export function fixtureSheetValueIndex(snapshot: VisualizationSnapshot | null) {
	const result = new Map<string, Map<string, AttributeValue>>();
	for (const value of snapshot?.values ?? []) {
		let fixture = result.get(value.fixture_id);
		if (!fixture) {
			fixture = new Map();
			result.set(value.fixture_id, fixture);
		}
		fixture.set(value.attribute, value.value);
	}
	return result;
}

export function fixtureSheetGroupValues({
	target,
	registry,
	values,
	preloadValues,
	programmerAttributes,
	dynamicStack,
	preloadDynamicStack,
}: {
	target: FixtureSheetTarget;
	registry: readonly AttributeDescriptor[];
	values: ReadonlyMap<string, AttributeValue> | undefined;
	preloadValues: ReadonlyMap<string, AttributeValue> | undefined;
	programmerAttributes: ReadonlySet<string>;
	dynamicStack: readonly FixtureSheetDynamicEntry[];
	preloadDynamicStack: readonly FixtureSheetDynamicEntry[];
}): FixtureSheetGroupValues {
	return Object.fromEntries(
		FIXTURE_SHEET_ATTRIBUTE_GROUPS.map((group) => {
			const descriptors = registry
				.filter(
					(descriptor) =>
						descriptor.encoder_group === group &&
						!descriptor.retired &&
						targetHasAttribute(target, descriptor.id),
				)
				.sort(
					(left, right) =>
						(left.encoder_page ?? 0) - (right.encoder_page ?? 0) ||
						(left.encoder_slot ?? 0) - (right.encoder_slot ?? 0) ||
						left.label.localeCompare(right.label),
				);
			const members = descriptors.map((descriptor) => {
				const fallback: AttributeValue = {
					kind: "normalized",
					value: targetDefault(target, descriptor.id),
				};
				const value = values?.get(descriptor.id) ?? fallback;
				const pending = preloadValues?.get(descriptor.id) ?? null;
				const preloadValue =
					pending && !fixtureSheetAttributeValuesEqual(pending, value)
						? pending
						: null;
				const source = programmerAttributes.has(descriptor.id)
					? ("programmer" as const)
					: values?.has(descriptor.id) &&
							!fixtureSheetAttributeValuesEqual(value, fallback)
						? ("playback" as const)
						: ("default" as const);
				return {
					attribute: descriptor.id,
					label: descriptor.label,
					value,
					text: formatFixtureSheetValue(value, descriptor, target),
					preloadValue,
					preloadText:
						preloadValue == null
							? null
							: formatFixtureSheetValue(preloadValue, descriptor, target),
					source,
					dynamics: [
						...dynamicIdentities(
							dynamicStack,
							descriptor.id,
							"normal" as const,
						),
						...dynamicIdentities(
							preloadDynamicStack,
							descriptor.id,
							"preload" as const,
						),
					],
				};
			});
			const source = groupSource(members);
			const accessibleName = members.length
				? members
						.map((member) => {
							const preload = member.preloadText
								? `, Preload ${member.preloadText}`
								: "";
							const dynamics = member.dynamics.length
								? `, ${member.dynamics
										.map((dynamic) => dynamic.accessibleName)
										.join(", ")}`
								: "";
							return `${member.label}: ${member.text}${preload}${dynamics}`;
						})
						.join("; ")
				: `${fixtureSheetGroupLabel(group)} unavailable`;
			return [
				group,
				{
					id: group,
					members,
					available: members.length > 0,
					source,
					accessibleName,
				} satisfies FixtureSheetGroupValue,
			];
		}),
	) as FixtureSheetGroupValues;
}

export function fixtureSheetGroupLabel(group: FixtureSheetAttributeGroup) {
	return group[0]?.toUpperCase() + group.slice(1);
}

export function fixtureSheetNormalizedValue(
	member: FixtureSheetMemberValue | undefined,
) {
	return member?.value.kind === "normalized" ? member.value.value : null;
}

function groupSource(members: readonly FixtureSheetMemberValue[]): ValueSource {
	if (members.some((member) => member.source === "programmer"))
		return "programmer";
	if (members.some((member) => member.source === "playback")) return "playback";
	return "default";
}

function dynamicIdentities(
	entries: readonly FixtureSheetDynamicEntry[],
	attribute: string,
	lane: "normal" | "preload",
) {
	return entries
		.filter(
			(entry) =>
				entry.entry_type === "dynamic" && entry.attribute === attribute,
		)
		.map((entry) => dynamicIdentity(entry, lane));
}

function dynamicIdentity(
	entry: FixtureSheetDynamicEntry,
	lane: "normal" | "preload",
): FixtureSheetDynamicIdentity {
	const stableId =
		entry.dynamic_id ??
		entry.runtime_instance_id ??
		entry.controller_id ??
		entry.lane_id;
	const label =
		entry.pool_number == null
			? `Snapshot ${stableId?.slice(0, 8) ?? entry.name}`
			: String(entry.pool_number);
	const states = [
		lane === "preload" || entry.pending ? "pending" : "running",
		entry.paused ? "paused" : null,
		entry.hidden ? "hidden" : null,
		entry.winning ? "winning" : "non-winning",
	].filter(Boolean);
	return {
		lane,
		attribute: entry.attribute,
		label,
		accessibleName: `Dynamic ${label}, ${states.join(", ")}`,
		poolNumber: entry.pool_number ?? null,
		dynamicId: entry.dynamic_id ?? null,
		paused: entry.paused,
		pending: lane === "preload" || entry.pending,
		hidden: entry.hidden,
		winning: entry.winning,
	};
}

function formatFixtureSheetValue(
	value: AttributeValue,
	descriptor: AttributeDescriptor,
	target: FixtureSheetTarget,
) {
	switch (value.kind) {
		case "normalized":
			return formatNormalized(value.value, descriptor);
		case "discrete":
			return semanticValueLabel(value.value, descriptor.id, target);
		case "color_xyz":
			return `XYZ ${formatNumber(value.value.x)}, ${formatNumber(value.value.y)}, ${formatNumber(value.value.z)}`;
		case "spread":
			return "Spread";
		case "raw_dmx":
		case "raw_dmx_exact":
			return "Unavailable raw value";
	}
}

function formatNormalized(value: number, descriptor: AttributeDescriptor) {
	const unit = descriptor.display_unit ?? descriptor.default_unit;
	const domainValue =
		descriptor.domain_min != null && descriptor.domain_max != null
			? descriptor.domain_min +
				value * (descriptor.domain_max - descriptor.domain_min)
			: value;
	if (unit === "percent" || unit === "%") return `${Math.round(value * 100)}%`;
	if (unit === "deg" || unit === "°") return `${formatNumber(domainValue)}°`;
	if (unit) return `${formatNumber(domainValue)} ${unit}`;
	return `${Math.round(value * 100)}%`;
}

function semanticValueLabel(
	semanticId: string,
	attribute: string,
	target: FixtureSheetTarget,
) {
	const mode =
		target.fixture.definition.profile_snapshot?.modes.find(
			(candidate) => candidate.id === target.fixture.definition.mode_id,
		) ?? target.fixture.definition.profile_snapshot?.modes[0];
	for (const channel of mode?.channels ?? []) {
		if (channel.attribute !== attribute) continue;
		for (const fn of channel.functions) {
			if (
				(fn.behavior.type === "fixed" || fn.behavior.type === "indexed") &&
				fn.behavior.semantic_id === semanticId
			)
				return fn.behavior.label;
		}
	}
	return semanticId;
}

function fixtureSheetAttributeValuesEqual(
	left: AttributeValue,
	right: AttributeValue,
) {
	return JSON.stringify(left) === JSON.stringify(right);
}

function formatNumber(value: number) {
	return Number.isInteger(value) ? String(value) : value.toFixed(1);
}
