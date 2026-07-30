import { useEffect, useMemo, useState } from "react";
import type {
	CueList,
	PatchedFixture,
	VisualizationSnapshot,
} from "../api/types";
import type { AttributeValue } from "../api/types/playback";
import { fixtures } from "../data/mockData";
import {
	useActiveShowId,
	useBootstrapReady,
} from "../features/deskSnapshot/DeskSnapshotState";
import {
	type RuntimeGroup,
	useGroupRuntimeAuthority,
} from "../features/groupRuntime/groupRuntimeAuthority";
import { usePatchedFixturesView } from "../features/patch/PatchState";
import type { ProgrammerPreloadValuesProjection } from "../features/programmerPreloadValues/contracts";
import type { ProgrammerValuesProjection } from "../features/programmerValues/contracts";
import { useProgrammerValueTargets } from "../features/programmerValues/useProgrammerValueTargets";
import { useVisualizationRuntimeRead } from "../features/visualizationRuntime/VisualizationRuntimeView";
import type { FixtureSheetIncludedHeads, FixtureSheetOrder } from "../types";
import {
	activeProgrammerFixtureIds,
	compareFixtureIds,
	cueListFixtureIds,
} from "./fixtureSheetFilters";
import {
	fixtureSheetTargets,
	targetDefault,
	targetHasAttribute,
} from "./fixtureSheetTargets";

type FixtureSheetTarget = ReturnType<typeof fixtureSheetTargets>[number];
type FixtureGroup = RuntimeGroup;
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
	index,
	programmerValues,
	preloadValues,
	dynamicStack,
	limitingGroups,
}: {
	target: FixtureSheetTarget;
	index: number;
	programmerValues: ProgrammingValueIndex;
	preloadValues: ProgrammingValueIndex;
	dynamicStack: readonly DynamicStackEntry[];
	limitingGroups: readonly FixtureGroup[];
}) {
	const patched = target.fixture;
	const targetValues = programmerValues.get(target.fixtureId);
	const targetPreloadValues = preloadValues.get(target.fixtureId);
	const intensity = indexedTargetValue(targetValues, target, "intensity");
	const red = indexedTargetValue(targetValues, target, "color.red", 1);
	const green = indexedTargetValue(targetValues, target, "color.green", 1);
	const blue = indexedTargetValue(targetValues, target, "color.blue", 1);
	const pan = indexedTargetValue(targetValues, target, "pan");
	const tilt = indexedTargetValue(targetValues, target, "tilt");
	const base = fixtures[index % fixtures.length];
	const hasIntensity = targetHasAttribute(target, "intensity");
	const hasColor = target.heads.some((head) =>
		head.parameters.some((parameter) =>
			parameter.attribute.startsWith("color."),
		),
	);
	const hasPosition =
		targetHasAttribute(target, "pan") || targetHasAttribute(target, "tilt");
	const preloadIntensity =
		hasIntensity && targetPreloadValues
			? indexedTargetValue(targetPreloadValues, target, "intensity")
			: null;
	const preloadRed =
		hasColor && targetPreloadValues
			? indexedTargetValue(targetPreloadValues, target, "color.red", 1)
			: null;
	const preloadGreen =
		hasColor && targetPreloadValues
			? indexedTargetValue(targetPreloadValues, target, "color.green", 1)
			: null;
	const preloadBlue =
		hasColor && targetPreloadValues
			? indexedTargetValue(targetPreloadValues, target, "color.blue", 1)
			: null;
	const preloadPan =
		hasPosition && targetPreloadValues
			? indexedTargetValue(targetPreloadValues, target, "pan")
			: null;
	const preloadTilt =
		hasPosition && targetPreloadValues
			? indexedTargetValue(targetPreloadValues, target, "tilt")
			: null;
	const hasLiveColor =
		targetValues != null &&
		[...targetValues.keys()].some((attribute) =>
			attribute.startsWith("color."),
		);
	const color = `rgb(${Math.round(red * 255)}, ${Math.round(green * 255)}, ${Math.round(blue * 255)})`;
	return {
		...base,
		id: target.displayId,
		name: target.name,
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
		dimmer: Math.round(intensity * 100),
		color,
		colorLabel: hasColor ? color : "White",
		pan: Math.round(pan * 100),
		tilt: Math.round(tilt * 100),
		preloadDimmer:
			preloadIntensity == null ? null : Math.round(preloadIntensity * 100),
		preloadColor:
			preloadRed == null || preloadGreen == null || preloadBlue == null
				? null
				: `rgb(${Math.round(preloadRed * 255)}, ${Math.round(preloadGreen * 255)}, ${Math.round(preloadBlue * 255)})`,
		preloadPan: preloadPan == null ? null : Math.round(preloadPan * 100),
		preloadTilt: preloadTilt == null ? null : Math.round(preloadTilt * 100),
		sources: {
			...base.sources,
			dimmer:
				hasIntensity && targetValues?.has("intensity")
					? ("programmer" as const)
					: ("default" as const),
			color:
				hasColor && hasLiveColor
					? ("programmer" as const)
					: ("default" as const),
			position:
				hasPosition &&
				(targetValues?.has("pan") === true ||
					targetValues?.has("tilt") === true)
					? ("programmer" as const)
					: ("default" as const),
		},
		limitingGroups,
		positionLabel: hasPosition ? undefined : "—",
		dynamicStack,
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
	const groupsById = new Map(groups.map((group) => [group.id, group]));
	for (const value of projection?.groupValues ?? []) {
		const fixtureIds = groupsById.get(value.groupId)?.body.fixtures ?? [];
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
): Map<string, FixtureGroup[]> {
	const result = new Map<string, FixtureGroup[]>();
	for (const group of groups) {
		if (group.runtime.playbackNumber == null || group.runtime.master >= 1)
			continue;
		for (const fixtureId of group.body.fixtures) {
			const fixtureGroups = result.get(fixtureId);
			if (fixtureGroups) fixtureGroups.push(group);
			else result.set(fixtureId, [group]);
		}
	}
	return result;
}

function indexedTargetValue(
	values: Map<string, ProgrammingValue> | undefined,
	target: FixtureSheetTarget,
	attribute: string,
	fallback = 0,
) {
	if (!targetHasAttribute(target, attribute)) return fallback;
	const value = values?.get(attribute)?.value;
	return value?.kind === "normalized"
		? value.value
		: targetDefault(target, attribute, fallback);
}

function demoFixtureSheetRows() {
	return fixtures.map((fixture) => ({
		...fixture,
		fixtureType: fixture.type,
		patch: "",
		icon: null,
		fixtureId: "",
		targetKind: "fixture" as const,
		parentFixtureId: "",
		childFixtureIds: [] as string[],
		indented: false,
		limitingGroups: [] as FixtureGroup[],
		preloadDimmer: null,
		preloadColor: null,
		preloadPan: null,
		preloadTilt: null,
		dynamicStack: [],
	}));
}

export function useFixtureSheetRows({
	visualization,
	preloadVisualization,
	programmerValues,
	preloadProgrammerValues,
	fixtureOrder,
	activeOnly,
	selectedCueList,
	includedHeads,
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
	active?: boolean;
}) {
	const bootstrapReady = useBootstrapReady();
	const activeShowId = useActiveShowId();
	const observesGroupRuntime = active && activeShowId !== null;
	const groupAuthority = useGroupRuntimeAuthority(observesGroupRuntime);
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
		const preloadValues = fixtureSheetProgrammerValueIndex(
			preloadProgrammerValues,
			groupAuthority.groups,
		);
		const dynamicStack = indexDynamicStack(visualization, preloadVisualization);
		const limitingGroups = indexLimitingGroups(groupAuthority.groups);
		return orderedFixtureTargets({
			fixtures: patchedFixtures,
			fixtureOrder,
			activeOnly,
			selectedCueList,
			includedHeads,
			groups: groupAuthority.groups,
			activeValueTargets,
		}).map((target, index) =>
			fixtureSheetRow({
				target,
				index,
				programmerValues: indexedProgrammerValues,
				preloadValues,
				dynamicStack: dynamicStack.get(target.fixtureId) ?? [],
				limitingGroups: limitingGroups.get(target.fixtureId) ?? [],
			}),
		);
	}, [
		activeOnly,
		activeValueTargets,
		bootstrapReady,
		fixtureOrder,
		groupAuthority.groups,
		groupAuthority.serving,
		includedHeads,
		observesGroupRuntime,
		patchedFixtures,
		preloadProgrammerValues,
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

type OptionalDynamicStack<T> = T extends { dynamicStack: infer Stack }
	? Omit<T, "dynamicStack"> & { dynamicStack?: Stack }
	: T;

export type FixtureSheetRow = OptionalDynamicStack<
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
				if (!cancelled) setSnapshot(next);
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
