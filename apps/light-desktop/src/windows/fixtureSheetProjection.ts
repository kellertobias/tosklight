import { useEffect, useMemo, useState } from "react";
import type {
	CueList,
	HighlightState,
	PatchedFixture,
	VisualizationSnapshot,
} from "../api/types";
import type { AttributeValue } from "../api/types/playback";
import { fixtures } from "../data/mockData";
import {
	useActiveShowId,
	useAttributeRegistry,
	useBootstrapReady,
} from "../features/deskSnapshot/DeskSnapshotState";
import {
	type GroupRuntimeState,
	type RuntimeGroup,
	useGroupRuntimeAuthority,
} from "../features/groupRuntime/groupRuntimeAuthority";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import type { ProgrammerPreloadValuesProjection } from "../features/programmerPreloadValues/contracts";
import type { ProgrammerValuesProjection } from "../features/programmerValues/contracts";
import { useProgrammerValueTargets } from "../features/programmerValues/useProgrammerValueTargets";
import { resolveGroupMembership } from "../features/showObjects/groupProjection";
import { useVisualizationRuntimeRead } from "../features/visualizationRuntime/VisualizationRuntimeView";
import type { FixtureSheetIncludedHeads, FixtureSheetOrder } from "../types";
import {
	activeProgrammerFixtureIds,
	compareFixtureIds,
	cueListFixtureIds,
	fixtureSheetIncludesFixture,
} from "./fixtureSheetFilters";
import { fixtureSheetTargets } from "./fixtureSheetTargets";
import {
	fixtureSheetGroupValues,
	fixtureSheetNormalizedValue,
	fixtureSheetValueIndex,
} from "./fixtureSheetValues";

type FixtureSheetTarget = ReturnType<typeof fixtureSheetTargets>[number];
type FixtureGroup = RuntimeGroup;
type LimitingGroup = FixtureGroup & { runtime: GroupRuntimeState };
type ProgrammingValue = {
	value: AttributeValue;
	programmerOrder: number;
};
type DynamicStackEntry = NonNullable<
	VisualizationSnapshot["dynamic_stack"]
>[number];
type ProgrammingValueIndex = Map<string, Map<string, ProgrammingValue>>;
type DynamicStackIndex = Map<string, DynamicStackEntry[]>;

function targetFamilyActive(
	target: FixtureSheetTarget,
	activeIds: Set<string>,
) {
	return (
		activeIds.has(target.fixtureId) ||
		target.fixture.logical_heads.some((head) => activeIds.has(head.fixture_id))
	);
}

function orderedFixtureTargets({
	fixtures,
	fixtureOrder,
	activeOnly,
	selectedCueList,
	includedHeads,
	groups,
	activeValueTargets,
}: {
	fixtures: readonly PatchedFixture[];
	fixtureOrder: FixtureSheetOrder;
	activeOnly: boolean;
	selectedCueList: CueList | null;
	includedHeads: FixtureSheetIncludedHeads;
	groups: readonly FixtureGroup[];
	activeValueTargets: Parameters<typeof activeProgrammerFixtureIds>[0];
}) {
	const activeIds = activeProgrammerFixtureIds(activeValueTargets, groups);
	const cueIds = cueListFixtureIds(selectedCueList ?? undefined, groups);
	return [...fixtures]
		.filter(fixtureSheetIncludesFixture)
		.sort(compareFixtureIds)
		.flatMap((fixture) => fixtureSheetTargets(fixture, includedHeads))
		.filter((target) => !activeOnly || targetFamilyActive(target, activeIds))
		.filter((target) => cueIds == null || targetFamilyActive(target, cueIds))
		.sort((left, right) => {
			if (fixtureOrder === "active") {
				const difference =
					Number(targetFamilyActive(right, activeIds)) -
					Number(targetFamilyActive(left, activeIds));
				if (difference) return difference;
			}
			return (
				compareFixtureIds(left.fixture, right.fixture) ||
				left.order - right.order
			);
		});
}

function fixtureSheetRow({
	target,
	programmerValues,
	dynamicStack,
	preloadDynamicStack,
	ordinaryValues,
	preloadOrdinaryValues,
	attributeRegistry,
	limitingGroups,
	highlightBypassesGroupMaster,
}: {
	target: FixtureSheetTarget;
	programmerValues: ProgrammingValueIndex;
	dynamicStack: readonly DynamicStackEntry[];
	preloadDynamicStack: readonly DynamicStackEntry[];
	ordinaryValues: ReadonlyMap<string, AttributeValue> | undefined;
	preloadOrdinaryValues: ReadonlyMap<string, AttributeValue> | undefined;
	attributeRegistry: NonNullable<ReturnType<typeof useAttributeRegistry>>;
	limitingGroups: readonly LimitingGroup[];
	highlightBypassesGroupMaster: boolean;
}) {
	const patched = target.fixture;
	const exactFreeze = (patched.freeze_targets ?? []).find(
		(candidate) => candidate.fixture_id === target.fixtureId,
	);
	const containedFreeze =
		target.order === 0
			? (patched.freeze_targets ?? []).filter((candidate) =>
					patched.logical_heads.some(
						(head) => head.fixture_id === candidate.fixture_id,
					),
				)
			: [];
	const freezeTargets = exactFreeze ? [exactFreeze] : containedFreeze;
	const freezeFamilies = [
		...new Set(freezeTargets.flatMap((candidate) => candidate.families)),
	];
	const targetValues = programmerValues.get(target.fixtureId);
	const groupValues = fixtureSheetGroupValues({
		target,
		registry: attributeRegistry,
		values: ordinaryValues,
		preloadValues: preloadOrdinaryValues,
		programmerAttributes: new Set(targetValues?.keys() ?? []),
		dynamicStack,
		preloadDynamicStack,
	});
	const intensityMember = groupValues.intensity.members.find(
		(member) => member.attribute === "intensity",
	);
	const colorMember = (attribute: string) =>
		groupValues.color.members.find((member) => member.attribute === attribute);
	const positionMember = (attribute: string) =>
		groupValues.position.members.find(
			(member) => member.attribute === attribute,
		);
	const intensity = fixtureSheetNormalizedValue(intensityMember) ?? 0;
	const red = fixtureSheetNormalizedValue(colorMember("color.red"));
	const green = fixtureSheetNormalizedValue(colorMember("color.green"));
	const blue = fixtureSheetNormalizedValue(colorMember("color.blue"));
	const pan = fixtureSheetNormalizedValue(positionMember("pan")) ?? 0;
	const tilt = fixtureSheetNormalizedValue(positionMember("tilt")) ?? 0;
	const preloadIntensity =
		intensityMember?.preloadValue?.kind === "normalized"
			? intensityMember.preloadValue.value
			: null;
	const preloadColorValue = (attribute: string) => {
		const value = colorMember(attribute)?.preloadValue;
		return value?.kind === "normalized" ? value.value : null;
	};
	const preloadRed = preloadColorValue("color.red");
	const preloadGreen = preloadColorValue("color.green");
	const preloadBlue = preloadColorValue("color.blue");
	const preloadPositionValue = (attribute: string) => {
		const value = positionMember(attribute)?.preloadValue;
		return value?.kind === "normalized" ? value.value : null;
	};
	const preloadPan = preloadPositionValue("pan");
	const preloadTilt = preloadPositionValue("tilt");
	const color =
		red == null || green == null || blue == null
			? "transparent"
			: `rgb(${Math.round(red * 255)}, ${Math.round(green * 255)}, ${Math.round(blue * 255)})`;
	return {
		id: target.displayId,
		name: target.name,
		type: patched.definition.mode,
		fixtureType: `${patched.definition.manufacturer} · ${patched.definition.mode}`,
		patch:
			patched.universe != null && patched.address != null
				? `U${patched.universe}.${patched.address}`
				: "Unpatched",
		icon: patched.definition.icon_asset ?? null,
		fixtureId: target.fixtureId,
		targetKind: (patched.logical_heads.length
			? target.order === 0
				? "master"
				: "head"
			: "fixture") as "fixture" | "master" | "head",
		parentFixtureId: patched.fixture_id,
		childFixtureIds: patched.logical_heads.map((head) => head.fixture_id),
		indented: target.indented,
		freeze: freezeTargets.length
			? {
					full: freezeTargets.every((candidate) => candidate.full),
					families: freezeFamilies,
					contained: exactFreeze == null && containedFreeze.length > 0,
				}
			: null,
		dimmer: Math.round(intensity * 100),
		color,
		colorAvailable: groupValues.color.available,
		colorLabel: groupValues.color.available
			? groupValues.color.members.map((member) => member.text).join(" / ")
			: "—",
		pan: Math.round(pan * 100),
		tilt: Math.round(tilt * 100),
		positionAvailable: groupValues.position.available,
		preloadDimmer:
			preloadIntensity == null ? null : Math.round(preloadIntensity * 100),
		preloadColor:
			preloadRed == null || preloadGreen == null || preloadBlue == null
				? null
				: `rgb(${Math.round(preloadRed * 255)}, ${Math.round(preloadGreen * 255)}, ${Math.round(preloadBlue * 255)})`,
		preloadPan: preloadPan == null ? null : Math.round(preloadPan * 100),
		preloadTilt: preloadTilt == null ? null : Math.round(preloadTilt * 100),
		sources: {
			dimmer: groupValues.intensity.source,
			color: groupValues.color.source,
			position: groupValues.position.source,
			beam: groupValues.beam.source,
			focus: groupValues.focus.source,
		},
		limitingGroups,
		highlightBypassesGroupMaster,
		positionLabel: groupValues.position.available ? undefined : "—",
		beam: groupValues.beam.available
			? groupValues.beam.members.map((member) => member.text).join(" / ")
			: "—",
		focus: groupValues.focus.available
			? groupValues.focus.members.map((member) => member.text).join(" / ")
			: "—",
		dynamicStack: [...dynamicStack, ...preloadDynamicStack],
		groupValues,
	};
}

export function fixtureSheetProgrammerValueIndex(
	projection:
		| ProgrammerValuesProjection
		| ProgrammerPreloadValuesProjection
		| null
		| undefined,
	groups: readonly FixtureGroup[],
): ProgrammingValueIndex {
	const result: ProgrammingValueIndex = new Map();
	const assign = (
		fixtureId: string,
		attribute: string,
		value: AttributeValue,
		programmerOrder: number,
	) => {
		let fixtureValues = result.get(fixtureId);
		if (!fixtureValues) {
			fixtureValues = new Map();
			result.set(fixtureId, fixtureValues);
		}
		const current = fixtureValues.get(attribute);
		if (!current || current.programmerOrder <= programmerOrder)
			fixtureValues.set(attribute, { value, programmerOrder });
	};
	for (const value of projection?.fixtureValues ?? [])
		assign(
			value.fixtureId,
			value.attribute,
			value.value,
			value.programmerOrder,
		);
	const membership = resolveGroupMembership(groups);
	for (const value of projection?.groupValues ?? []) {
		const fixtureIds = membership.get(value.groupId) ?? [];
		fixtureIds.forEach((fixtureId, index) => {
			assign(
				fixtureId,
				value.attribute,
				programmerValueForFixture(value.value, index, fixtureIds.length),
				value.programmerOrder,
			);
		});
	}
	return result;
}

function programmerValueForFixture(
	value: AttributeValue,
	index: number,
	count: number,
): AttributeValue {
	if (value.kind !== "spread") return value;
	const points = value.value;
	if (!points.length) return { kind: "normalized", value: 0 };
	if (points.length === 1 || count <= 1)
		return { kind: "normalized", value: points[0] ?? 0 };
	const position = (index / (count - 1)) * (points.length - 1);
	const lower = Math.floor(position);
	const upper = Math.min(points.length - 1, lower + 1);
	const mix = position - lower;
	const start = points[lower] ?? 0;
	const finish = points[upper] ?? start;
	return { kind: "normalized", value: start + (finish - start) * mix };
}

function indexDynamicStack(
	...snapshots: Array<VisualizationSnapshot | null>
): DynamicStackIndex {
	const result: DynamicStackIndex = new Map();
	for (const snapshot of snapshots) {
		for (const entry of snapshot?.dynamic_stack ?? []) {
			if (entry.entry_type === "ordinary_static") continue;
			const fixtureEntries = result.get(entry.fixture_id);
			if (fixtureEntries) fixtureEntries.push(entry);
			else result.set(entry.fixture_id, [entry]);
		}
	}
	return result;
}

function indexLimitingGroups(
	groups: readonly FixtureGroup[],
	fixtures: readonly PatchedFixture[],
): Map<string, LimitingGroup[]> {
	const result = new Map<string, LimitingGroup[]>();
	const membership = resolveGroupMembership(groups);
	const participates = new Map<string, boolean>();
	for (const fixture of fixtures) {
		const profileMode =
			fixture.definition.profile_snapshot?.modes.find(
				(mode) => mode.id === fixture.definition.mode_id,
			) ?? fixture.definition.profile_snapshot?.modes[0];
		const eligible = profileMode
			? profileMode.channels.some((channel) => channel.reacts_to_group_master)
			: fixture.definition.heads.some((head) =>
					head.parameters.some((parameter) =>
						parameter.attribute.toLowerCase().includes("intensity"),
					),
				);
		const active = (fixture.group_masters_enabled ?? true) && eligible;
		participates.set(fixture.fixture_id, active);
		for (const head of fixture.logical_heads)
			participates.set(head.fixture_id, active);
	}
	for (const group of groups) {
		if (
			group.runtime == null ||
			group.runtime.playbackNumber == null ||
			(group.runtime.master >= 1 && group.runtime.flashLevel <= 0)
		)
			continue;
		const limitingGroup = group as LimitingGroup;
		for (const fixtureId of membership.get(group.id) ?? []) {
			if (!participates.get(fixtureId)) continue;
			const fixtureGroups = result.get(fixtureId);
			if (fixtureGroups) fixtureGroups.push(limitingGroup);
			else result.set(fixtureId, [limitingGroup]);
		}
	}
	return result;
}

function fixtureSheetHighlightIds(highlight: HighlightState | null | undefined) {
	if (!highlight?.active || !highlight.output_enabled) return new Set<string>();
	if (highlight.mode === "step")
		return new Set(
			highlight.active_fixture ? [highlight.active_fixture.fixture_id] : [],
		);
	return new Set(highlight.remembered.map((fixture) => fixture.fixture_id));
}

function demoFixtureSheetRows() {
	return fixtures.map((fixture) => ({
		...fixture,
		fixtureType: fixture.type,
		patch: "",
		icon: null,
		fixtureId: "",
		colorAvailable: true,
		positionAvailable: true,
		targetKind: "fixture" as const,
		parentFixtureId: "",
		childFixtureIds: [] as string[],
		indented: false,
		limitingGroups: [] as LimitingGroup[],
		highlightBypassesGroupMaster: false,
		freeze: null,
		preloadDimmer: null,
		preloadColor: null,
		preloadPan: null,
		preloadTilt: null,
		dynamicStack: [],
		groupValues: undefined,
	}));
}

export function useFixtureSheetRows({
	visualization,
	preloadVisualization,
	programmerValues,
	fixtureOrder,
	activeOnly,
	selectedCueList,
	includedHeads,
	highlight,
	active = true,
}: {
	visualization: VisualizationSnapshot | null;
	preloadVisualization: VisualizationSnapshot | null;
	programmerValues?: ProgrammerValuesProjection | null;
	preloadProgrammerValues?: ProgrammerPreloadValuesProjection | null;
	fixtureOrder: FixtureSheetOrder;
	activeOnly: boolean;
	selectedCueList: CueList | null;
	includedHeads: FixtureSheetIncludedHeads;
	highlight?: HighlightState | null;
	active?: boolean;
}) {
	const bootstrapReady = useBootstrapReady();
	const activeShowId = useActiveShowId();
	const observesGroupRuntime = active && activeShowId !== null;
	const groupAuthority = useGroupRuntimeAuthority(observesGroupRuntime);
	const attributeRegistry = useAttributeRegistry() ?? [];
	const patchedFixtures = usePatchedFixturesView(active);
	const observesActiveValues =
		active && (activeOnly || fixtureOrder === "active");
	const activeValueTargets = useProgrammerValueTargets(observesActiveValues);
	const activeValuesLoading =
		observesActiveValues && activeValueTargets === null;
	const rows = useMemo(() => {
		if (!bootstrapReady) return demoFixtureSheetRows();
		if (observesGroupRuntime && !groupAuthority.serving) return [];
		const indexedProgrammerValues = fixtureSheetProgrammerValueIndex(
			programmerValues,
			groupAuthority.groups,
		);
		const dynamicStack = indexDynamicStack(visualization);
		const preloadDynamicStack = indexDynamicStack(preloadVisualization);
		const ordinaryValues = fixtureSheetValueIndex(visualization);
		const preloadOrdinaryValues = fixtureSheetValueIndex(preloadVisualization);
		const limitingGroups = indexLimitingGroups(
			groupAuthority.groups,
			patchedFixtures,
		);
		const highlightedFixtureIds = fixtureSheetHighlightIds(highlight);
		return orderedFixtureTargets({
			fixtures: patchedFixtures,
			fixtureOrder,
			activeOnly,
			selectedCueList,
			includedHeads,
			groups: groupAuthority.groups,
			activeValueTargets,
		}).map((target) =>
			fixtureSheetRow({
				target,
				programmerValues: indexedProgrammerValues,
				dynamicStack: dynamicStack.get(target.fixtureId) ?? [],
				preloadDynamicStack: preloadDynamicStack.get(target.fixtureId) ?? [],
				ordinaryValues: ordinaryValues.get(target.fixtureId),
				preloadOrdinaryValues: preloadOrdinaryValues.get(target.fixtureId),
				attributeRegistry,
				limitingGroups: limitingGroups.get(target.fixtureId) ?? [],
				highlightBypassesGroupMaster:
					highlightedFixtureIds.has(target.fixtureId) ||
					(target.order === 0 &&
						target.fixture.logical_heads.some((head) =>
							highlightedFixtureIds.has(head.fixture_id),
						)),
			}),
		);
	}, [
		activeOnly,
		activeValueTargets,
		attributeRegistry,
		bootstrapReady,
		fixtureOrder,
		groupAuthority.groups,
		groupAuthority.serving,
		highlight,
		includedHeads,
		observesGroupRuntime,
		patchedFixtures,
		preloadVisualization,
		programmerValues,
		selectedCueList,
		visualization,
	]);
	return {
		rows,
		activeValuesLoading: bootstrapReady ? activeValuesLoading : false,
		groupRuntimeLoading:
			bootstrapReady && observesGroupRuntime && !groupAuthority.serving,
	};
}

type OptionalFixtureSheetRuntime<T> = T extends {
	dynamicStack: infer Stack;
	groupValues: infer Groups;
	highlightBypassesGroupMaster: infer HighlightBypass;
}
	? Omit<
			T,
			| "dynamicStack"
			| "groupValues"
			| "highlightBypassesGroupMaster"
			| "freeze"
			| "colorAvailable"
			| "positionAvailable"
		> & {
			dynamicStack?: Stack;
			groupValues?: Groups;
			highlightBypassesGroupMaster?: HighlightBypass;
			freeze?: T extends { freeze: infer Freeze } ? Freeze : never;
			/** Omitted by presentation fixtures, which describe lanterns that carry the group. */
			colorAvailable?: boolean;
			positionAvailable?: boolean;
		}
	: T;

export type FixtureSheetRow = OptionalFixtureSheetRuntime<
	| ReturnType<typeof fixtureSheetRow>
	| ReturnType<typeof demoFixtureSheetRows>[number]
>;

export function useFixtureSheetVisualizations(
	preloadActive: boolean,
	active = true,
	fixtureIds: readonly string[] = [],
) {
	const visualization = useEventuallyConsistentFixtureSheetSnapshot(
		"normal",
		active,
		fixtureIds,
	);
	const preloadVisualization = useEventuallyConsistentFixtureSheetSnapshot(
		"preload",
		active && preloadActive,
		fixtureIds,
	);

	return { visualization, preloadVisualization };
}

function useEventuallyConsistentFixtureSheetSnapshot(
	lane: "normal" | "preload",
	enabled: boolean,
	fixtureIds: readonly string[],
) {
	const read = useVisualizationRuntimeRead(lane, {
		dynamicStackOnly: true,
		fixtureIds,
	});
	const [snapshot, setSnapshot] = useState<VisualizationSnapshot | null>(null);
	useEffect(() => {
		if (!enabled || fixtureIds.length === 0) {
			setSnapshot(null);
			return;
		}
		let cancelled = false;
		let inFlight = false;
		const refresh = async () => {
			if (inFlight) return;
			inFlight = true;
			try {
				const next = await read();
				if (!cancelled)
					setSnapshot((current) =>
						fixtureSheetSnapshotsEqual(current, next) ? current : next,
					);
			} catch {
				// The regular connection/error surfaces remain authoritative.
				// Keep the last Dynamic identity projection while reconnecting.
			} finally {
				inFlight = false;
			}
		};
		void refresh();
		const timer = globalThis.setInterval(() => void refresh(), 2_000);
		return () => {
			cancelled = true;
			globalThis.clearInterval(timer);
		};
	}, [enabled, read]);
	return snapshot;
}

export function fixtureSheetSnapshotsEqual(
	left: VisualizationSnapshot | null,
	right: VisualizationSnapshot | null,
) {
	if (left === right) return true;
	if (left == null || right == null) return false;
	return (
		fixtureSheetSnapshotSignature(left) === fixtureSheetSnapshotSignature(right)
	);
}

function fixtureSheetSnapshotSignature(snapshot: VisualizationSnapshot) {
	const values = snapshot.values
		.map((value) => [value.fixture_id, value.attribute, value.value] as const)
		.sort((left, right) =>
			`${left[0]}:${left[1]}`.localeCompare(`${right[0]}:${right[1]}`),
		);
	const dynamics = (snapshot.dynamic_stack ?? [])
		.map((entry) => ({
			fixtureId: entry.fixture_id,
			attribute: entry.attribute,
			type: entry.entry_type,
			source: entry.source,
			dynamicId: entry.dynamic_id ?? null,
			poolNumber: entry.pool_number ?? null,
			name: entry.name,
			runtimeInstanceId: entry.runtime_instance_id ?? null,
			controllerId: entry.controller_id ?? null,
			laneId: entry.lane_id ?? null,
			size: entry.size ?? null,
			paused: entry.paused,
			hidden: entry.hidden,
			pending: entry.pending,
			winning: entry.winning,
		}))
		.sort((left, right) =>
			JSON.stringify(left).localeCompare(JSON.stringify(right)),
		);
	return JSON.stringify({
		showId: snapshot.scope?.show_id ?? null,
		preload: snapshot.preload ?? false,
		values,
		dynamics,
	});
}
