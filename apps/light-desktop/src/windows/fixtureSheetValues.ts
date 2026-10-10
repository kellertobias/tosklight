import type { AttributeDescriptor, VisualizationSnapshot } from "../api/types";
import type { AttributeValue } from "../api/types/playback";
import { presentColorProgram } from "../components/control/parameterControls/familyValuePresentation";
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
	value: AttributeValue | null;
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

/**
 * The sheet's own key for a fixture's commanded Position pose. It is never a registry
 * attribute, so nothing else reads it as a resolved value.
 */
export const FIXTURE_SHEET_COMMANDED_POSITION = "position:commanded";

/**
 * The Position cell shows the pose the fixture is actually commanded to, the same readout the
 * Pan/Tilt encoders show: an idle mover at its declared default pose, a programmed one at its
 * achieved Angles. The server lists each requested owner with one common commanded pair in
 * `commanded_positions`; it is indexed here under {@link FIXTURE_SHEET_COMMANDED_POSITION}.
 * Divergent copies and owners without a pose are absent.
 */
export function withCommandedPositions(
	snapshot: VisualizationSnapshot | null,
): VisualizationSnapshot | null {
	const owners = snapshot?.commanded_positions;
	if (!snapshot || !owners?.length) return snapshot;
	return {
		...snapshot,
		values: [
			...snapshot.values,
			...owners.map((owner) => ({
				fixture_id: owner.fixture_id,
				attribute: FIXTURE_SHEET_COMMANDED_POSITION,
				value: {
					kind: "position" as const,
					value: {
						kind: "angles" as const,
						pan_degrees: { kind: "value" as const, value: owner.pan_degrees },
						tilt_degrees: { kind: "value" as const, value: owner.tilt_degrees },
					},
				},
			})),
		],
	};
}

/** Pan and Tilt in degrees from one literal Angles value. */
function literalAngles(value: AttributeValue | undefined) {
	if (value?.kind !== "position" || value.value.kind !== "angles") return null;
	const { pan_degrees: pan, tilt_degrees: tilt } = value.value;
	return pan.kind === "value" && tilt.kind === "value"
		? { pan: pan.value, tilt: tilt.value }
		: null;
}

/**
 * Pan/Tilt read in degrees: the commanded pose, else the resolved Angles request. Null when the
 * member is not an axis or neither is known.
 */
function positionAxisText(
	attribute: string,
	values: ReadonlyMap<string, AttributeValue> | undefined,
) {
	if (attribute !== "pan" && attribute !== "tilt") return null;
	const angles =
		literalAngles(values?.get(FIXTURE_SHEET_COMMANDED_POSITION)) ??
		literalAngles(values?.get("position"));
	return angles ? `${formatNumber(angles[attribute])}°` : null;
}

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

/** Legacy scalar-only snapshots; never substitute these descriptors for a whole Color family. */
const NATIVE_RGB_DESCRIPTORS: readonly AttributeDescriptor[] = (
	[
		["color.red", "Red", 1],
		["color.green", "Green", 2],
		["color.blue", "Blue", 3],
	] as const
).map(([id, label, slot]) => ({
	id,
	label,
	family: "color",
	value_type: "continuous",
	default_unit: "percent",
	display_unit: "percent",
	domain_min: null,
	domain_max: null,
	encoder_group: "color",
	encoder_page: 1,
	encoder_slot: slot,
	retired: false,
}));

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
	const known = new Set(registry.map((descriptor) => descriptor.id));
	const fullRegistry = [
		...registry,
		...NATIVE_RGB_DESCRIPTORS.filter((descriptor) => !known.has(descriptor.id)),
	];
	const familyColor = usesWholeColor(
		target,
		fullRegistry,
		values,
		preloadValues,
		dynamicStack,
		preloadDynamicStack,
	);
	return Object.fromEntries(
		FIXTURE_SHEET_ATTRIBUTE_GROUPS.map((group) => {
			if (group === "color" && familyColor) {
				return [
					group,
					colorFamilyGroup(
						values?.get("color"),
						preloadValues?.get("color"),
						programmerAttributes,
						dynamicStack,
						preloadDynamicStack,
					),
				];
			}
			const descriptors = fullRegistry
				.filter(
					(descriptor) =>
						descriptor.encoder_group === group &&
						(group !== "color" || descriptor.id !== "color") &&
						!descriptor.retired &&
						targetHasAttribute(target, descriptor.id),
				)
				.sort(
					(left, right) =>
						(left.encoder_page ?? 0) - (right.encoder_page ?? 0) ||
						(left.encoder_slot ?? 0) - (right.encoder_slot ?? 0) ||
						left.label.localeCompare(right.label),
				);
			const members = descriptors.map((descriptor, index) => {
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
				// Since TL-552 Pan and Tilt are programmed as the Position family, and colour
				// channels as one Color Intent.
				const family =
					group === "position" || group === "color" ? group : descriptor.id;
				const source =
					programmerAttributes.has(descriptor.id) ||
					programmerAttributes.has(family)
						? ("programmer" as const)
						: values?.has(descriptor.id) || values?.has(family)
							? ("playback" as const)
							: ("default" as const);
				return {
					attribute: descriptor.id,
					label: descriptor.label,
					value,
					text:
						(group === "position" && positionAxisText(descriptor.id, values)) ||
						formatFixtureSheetValue(value, descriptor, target),
					preloadValue,
					preloadText:
						preloadValue == null
							? null
							: formatFixtureSheetValue(preloadValue, descriptor, target),
					source,
					dynamics: [
						...(group === "color" && index === 0
							? [
									...dynamicIdentities(dynamicStack, "color", "normal"),
									...dynamicIdentities(preloadDynamicStack, "color", "preload"),
								]
							: []),
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

/** Real legacy values win over capability-only family fallback, but never over authored Color. */
function usesWholeColor(
	target: FixtureSheetTarget,
	registry: readonly AttributeDescriptor[],
	values: ReadonlyMap<string, AttributeValue> | undefined,
	pending: ReadonlyMap<string, AttributeValue> | undefined,
	normalDynamics: readonly FixtureSheetDynamicEntry[],
	pendingDynamics: readonly FixtureSheetDynamicEntry[],
) {
	if (values?.has("color") || pending?.has("color")) return true;
	const scalar = (value: AttributeValue | undefined) =>
		value &&
		["normalized", "spread", "discrete", "raw_dmx", "raw_dmx_exact"].includes(
			value.kind,
		);
	const legacy = registry.some(
		(descriptor) =>
			descriptor.encoder_group === "color" &&
			descriptor.id !== "color" &&
			!descriptor.retired &&
			targetHasAttribute(target, descriptor.id) &&
			(scalar(values?.get(descriptor.id)) ||
				scalar(pending?.get(descriptor.id))),
	);
	return (
		!legacy &&
		(targetHasAttribute(target, "color") ||
			[...normalDynamics, ...pendingDynamics].some(
				(entry) =>
					entry.attribute === "color" && entry.entry_type === "dynamic",
			))
	);
}

/** Whole requested Color, including an explicitly absent Normal base with a pending value. */
function colorFamilyGroup(
	value: AttributeValue | undefined,
	pending: AttributeValue | undefined,
	programmerAttributes: ReadonlySet<string>,
	normalDynamics: readonly FixtureSheetDynamicEntry[],
	pendingDynamics: readonly FixtureSheetDynamicEntry[],
): FixtureSheetGroupValue {
	const preloadValue =
		pending && (!value || !fixtureSheetAttributeValuesEqual(pending, value))
			? pending
			: null;
	const source = programmerAttributes.has("color")
		? "programmer"
		: value
			? "playback"
			: "default";
	const text = colorFamilyText(value);
	const preloadText = preloadValue ? colorFamilyText(preloadValue) : null;
	const dynamics = [
		...dynamicIdentities(normalDynamics, "color", "normal"),
		...dynamicIdentities(pendingDynamics, "color", "preload"),
	];
	const provenance = colorFamilyProvenance(value);
	const pendingProvenance = colorFamilyProvenance(preloadValue ?? undefined);
	return {
		id: "color",
		available: value != null || pending != null,
		source,
		members: [
			{
				attribute: "color",
				label: "Color",
				value: value ?? null,
				text,
				preloadValue,
				preloadText,
				source,
				dynamics,
			},
		],
		accessibleName: `Color: ${text}${provenance}${preloadText ? `; Preload ${preloadText}${pendingProvenance}` : ""}${dynamics.map((item) => `; ${item.accessibleName}`).join("")}`,
	};
}

function colorFamilyProvenance(value: AttributeValue | undefined) {
	if (value?.kind !== "color_program" || value.value.kind !== "direct")
		return "";
	const source = value.value.recipe.source;
	return `; Direct source profile ${source.profile_id} revision ${source.profile_revision}, mode ${source.mode_id}, head ${source.head_id}, path ${source.path_id}`;
}

function colorFamilyText(value: AttributeValue | undefined) {
	if (!value) return "Unavailable";
	if (value.kind === "color_xyz") return "Color";
	if (value.kind !== "color_program") return "Unavailable";
	const program = presentColorProgram(value.value, () => null, {
		unknownAppearanceLabel: "unknown appearance",
	});
	if (program.kind === "semantic")
		return program.uvOnly
			? "Color intent · UV-only"
			: program.requestedVisibleBlack
				? "Color intent · black"
				: value.value.kind === "semantic" && value.value.intent.spreads?.length
					? "Color intent · spread"
					: "Color intent";
	if (value.value.kind === "direct" && value.value.recipe.spreads?.length)
		return "Direct · spread; unknown appearance";
	const visible = program.portable.visible;
	return visible.kind === "unknown"
		? "Direct · unknown appearance"
		: visible.black
			? program.portable.uv.kind === "known" &&
				program.portable.uv.amount.requested > 0
				? "Direct · UV-only"
				: "Direct · black"
			: "Direct · source appearance";
}

export function fixtureSheetGroupLabel(group: FixtureSheetAttributeGroup) {
	return group[0]?.toUpperCase() + group.slice(1);
}

export function fixtureSheetNormalizedValue(
	member: FixtureSheetMemberValue | undefined,
) {
	return member?.value?.kind === "normalized" ? member.value.value : null;
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
		case "group_family":
			// Fixture projections normally arrive materialized. Group summaries retain an
			// explicit mixed indication instead of choosing an arbitrary member's appearance.
			return "Group values";
		case "color_program":
			return value.value.kind === "semantic" ? "Color intent" : "Direct color";
		case "position":
			return value.value.kind === "angles" ? "Angles" : "Target";
		case "zoom":
			return value.value.opening_degrees.kind === "value"
				? `${formatNumber(value.value.opening_degrees.value)}°`
				: "Spread";
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
	// Without a declared domain a channel fraction is not an angle: never label it in degrees.
	if (unit === "deg" || unit === "°")
		return domainValue === value ? "—" : `${formatNumber(domainValue)}°`;
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
