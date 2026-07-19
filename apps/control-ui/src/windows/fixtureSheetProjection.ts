import { useEffect, useState } from "react";
import { useServer } from "../api/ServerContext";
import type { VisualizationSnapshot } from "../api/types";
import { fixtures } from "../data/mockData";
import type { ShowObject } from "../features/showObjects/contracts";
import { useGroups } from "../features/server/useShowObjectsState";
import type { FixtureSheetIncludedHeads, FixtureSheetOrder } from "../types";
import {
	activeProgrammerFixtureIds,
	compareFixtureIds,
	cueListFixtureIds,
} from "./fixtureSheetFilters";
import {
	fixtureSheetTargets,
	targetHasAttribute,
	targetValue,
} from "./fixtureSheetTargets";

type FixtureSheetTarget = ReturnType<typeof fixtureSheetTargets>[number];
type FixtureGroup = ShowObject<"group">;

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
	server,
	fixtureOrder,
	activeOnly,
	cueListId,
	includedHeads,
	groups,
}: {
	server: ReturnType<typeof useServer>;
	fixtureOrder: FixtureSheetOrder;
	activeOnly: boolean;
	cueListId: string;
	includedHeads: FixtureSheetIncludedHeads;
	groups: readonly FixtureGroup[];
}) {
	const ownProgrammer = server.bootstrap?.active_programmers.find(
		(programmer) => programmer.session_id === server.session?.session_id,
	);
	const activeIds = activeProgrammerFixtureIds(ownProgrammer, groups);
	const selectedCueList = server.playbacks?.cue_lists.find(
		(cueList) => cueList.id === cueListId,
	);
	const cueIds = cueListFixtureIds(selectedCueList, groups);
	return [...(server.patch?.fixtures ?? [])]
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
	visualization,
	preloadVisualization,
	groups,
}: {
	target: FixtureSheetTarget;
	index: number;
	visualization: VisualizationSnapshot | null;
	preloadVisualization: VisualizationSnapshot | null;
	groups: readonly FixtureGroup[];
}) {
	const patched = target.fixture;
	const intensity = targetValue(visualization, target, "intensity");
	const red = targetValue(visualization, target, "color.red", 1);
	const green = targetValue(visualization, target, "color.green", 1);
	const blue = targetValue(visualization, target, "color.blue", 1);
	const pan = targetValue(visualization, target, "pan");
	const tilt = targetValue(visualization, target, "tilt");
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
		preloadVisualization && hasIntensity
			? targetValue(preloadVisualization, target, "intensity")
			: null;
	const preloadRed =
		preloadVisualization && hasColor
			? targetValue(preloadVisualization, target, "color.red", 1)
			: null;
	const preloadGreen =
		preloadVisualization && hasColor
			? targetValue(preloadVisualization, target, "color.green", 1)
			: null;
	const preloadBlue =
		preloadVisualization && hasColor
			? targetValue(preloadVisualization, target, "color.blue", 1)
			: null;
	const preloadPan =
		preloadVisualization && hasPosition
			? targetValue(preloadVisualization, target, "pan")
			: null;
	const preloadTilt =
		preloadVisualization && hasPosition
			? targetValue(preloadVisualization, target, "tilt")
			: null;
	const hasLiveColor =
		visualization?.values.some(
			(entry) =>
				entry.fixture_id === target.fixtureId &&
				entry.attribute.startsWith("color."),
		) ?? false;
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
				hasIntensity &&
				visualization?.values.some(
					(entry) =>
						entry.fixture_id === target.fixtureId &&
						entry.attribute === "intensity",
				)
					? ("programmer" as const)
					: ("default" as const),
			color:
				hasColor && hasLiveColor
					? ("programmer" as const)
					: ("default" as const),
			position:
				hasPosition &&
				visualization?.values.some(
					(entry) =>
						entry.fixture_id === target.fixtureId &&
						(entry.attribute === "pan" || entry.attribute === "tilt"),
				)
					? ("programmer" as const)
					: ("default" as const),
		},
		limitingGroups: groups.filter(
			(group) =>
				group.body.playback_fader != null &&
				group.body.fixtures.includes(target.fixtureId) &&
				(group.body.master ?? 1) < 1,
		),
		positionLabel: hasPosition ? undefined : "—",
	};
}

export function useFixtureSheetRows({
	visualization,
	preloadVisualization,
	fixtureOrder,
	activeOnly,
	cueListId,
	includedHeads,
}: {
	visualization: VisualizationSnapshot | null;
	preloadVisualization: VisualizationSnapshot | null;
	fixtureOrder: FixtureSheetOrder;
	activeOnly: boolean;
	cueListId: string;
	includedHeads: FixtureSheetIncludedHeads;
}) {
	const server = useServer();
	const groups = useGroups(server.playbacks);
	if (!server.bootstrap) {
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
		}));
	}
	return orderedFixtureTargets({
		server,
		fixtureOrder,
		activeOnly,
		cueListId,
		includedHeads,
		groups,
	}).map((target, index) =>
		fixtureSheetRow({
			target,
			index,
			visualization,
			preloadVisualization,
			groups,
		}),
	);
}

export type FixtureSheetRow = ReturnType<typeof useFixtureSheetRows>[number];

export function useFixtureSheetVisualizations(preloadActive: boolean) {
	const server = useServer();
	const [visualization, setVisualization] =
		useState<VisualizationSnapshot | null>(null);
	const [preloadVisualization, setPreloadVisualization] =
		useState<VisualizationSnapshot | null>(null);

	useEffect(() => {
		let cancelled = false;
		const refresh = () =>
			void Promise.all([
				server.readVisualization(),
				preloadActive ? server.readVisualization(true) : Promise.resolve(null),
			])
				.then(([next, preload]) => {
					if (cancelled) return;
					setVisualization(next);
					setPreloadVisualization(preload);
				})
				.catch(() => undefined);
		refresh();
		const timer = window.setInterval(refresh, 250);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [preloadActive, server.readVisualization]);

	return { visualization, preloadVisualization };
}
