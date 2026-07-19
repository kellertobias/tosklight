import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { VisualizationSnapshot } from "../../../api/types";
import { selectedGroupId } from "../../../features/programmingInteraction/contracts";
import { useProgrammingSelectionView } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { useGroups } from "../../../features/server/useShowObjectsState";
import { usePollingResource } from "../../../hooks/usePollingResource";
import { useApp } from "../../../state/AppContext";
import {
	directProgrammerChoices,
	type ParameterFamily,
	parameterFamilies,
} from "./model";
import { useParameterProgrammerValues } from "./useParameterProgrammerValues";

const EMPTY_FIXTURE_IDS: readonly string[] = [];
const EMPTY_PROGRAMMER_VALUES: readonly never[] = [];

function useVisualization(
	active: boolean,
	selectedFixtureIds: readonly string[],
) {
	const server = useServer();
	const [visualization, setVisualization] =
		useState<VisualizationSnapshot | null>(null);
	useEffect(() => {
		if (active && selectedFixtureIds.length) return;
		setVisualization(null);
	}, [active, selectedFixtureIds.length]);
	usePollingResource({
		enabled: active && selectedFixtureIds.length > 0,
		intervalMillis: 400,
		load: server.readVisualization,
		onValue: setVisualization,
	});
	return visualization;
}

function useSupportedAttributes(
	selectedFixtureIds: readonly string[],
	groupId: string | null,
) {
	const server = useServer();
	const groups = useGroups(server.playbacks);
	return useMemo(() => {
		const result = new Set<string>();
		const selected = new Set(selectedFixtureIds);
		for (const fixture of server.patch?.fixtures ?? []) {
			const fixtureSelected =
				selected.has(fixture.fixture_id) ||
				fixture.logical_heads.some((head) => selected.has(head.fixture_id));
			if (!fixtureSelected) continue;
			for (const head of fixture.definition.heads ?? [])
				for (const parameter of head.parameters)
					result.add(parameter.attribute);
		}
		if (groupId) {
			result.add("intensity");
			const group = groups.find((candidate) => candidate.id === groupId);
			for (const attribute of Object.keys(group?.body.programming ?? {}))
				result.add(attribute);
		}
		return result;
	}, [server.patch, selectedFixtureIds, groupId, groups]);
}

function useResolvedValues(
	visualization: VisualizationSnapshot | null,
	selectedFixtureIds: readonly string[],
) {
	return useMemo(() => {
		const selected = new Set(selectedFixtureIds);
		const normalized = new Map<string, number>();
		const normalizedByFixture = new Map<string, Map<string, number>>();
		const discrete = new Map<string, string>();
		const discreteByFixture = new Map<string, Map<string, string>>();
		for (const entry of visualization?.values ?? []) {
			if (!selected.has(entry.fixture_id)) continue;
			if (entry.value.kind === "normalized") {
				if (!normalized.has(entry.attribute))
					normalized.set(entry.attribute, entry.value.value);
				const values = normalizedByFixture.get(entry.fixture_id) ?? new Map();
				values.set(entry.attribute, entry.value.value);
				normalizedByFixture.set(entry.fixture_id, values);
			} else if (entry.value.kind === "discrete") {
				if (!discrete.has(entry.attribute))
					discrete.set(entry.attribute, entry.value.value);
				const values = discreteByFixture.get(entry.fixture_id) ?? new Map();
				values.set(entry.attribute, entry.value.value);
				discreteByFixture.set(entry.fixture_id, values);
			}
		}
		return { normalized, normalizedByFixture, discrete, discreteByFixture };
	}, [visualization, selectedFixtureIds]);
}

export function useParameterProjection(family: ParameterFamily, active = true) {
	const server = useServer();
	const { state } = useApp();
	const selection = useProgrammingSelectionView(active);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	const selectedGroup = selectedGroupId(selection);
	const programmerValuesView = useParameterProgrammerValues(
		selectedFixtureIds,
		selectedGroup,
		active,
	);
	const visualization = useVisualization(active, selectedFixtureIds);
	const supported = useSupportedAttributes(selectedFixtureIds, selectedGroup);
	const values = useResolvedValues(visualization, selectedFixtureIds);
	const directChoices = useMemo(
		() =>
			directProgrammerChoices(server.patch?.fixtures ?? [], selectedFixtureIds),
		[server.patch, selectedFixtureIds],
	);
	const attributes = parameterFamilies[family].filter((attribute) =>
		supported.has(attribute),
	);
	return {
		server,
		state,
		active,
		selectedFixtureIds,
		selectedGroupId: selectedGroup,
		programmerValuesReady: programmerValuesView?.ready ?? false,
		programmerValues:
			programmerValuesView?.fixtureValues ?? EMPTY_PROGRAMMER_VALUES,
		groupProgrammerValues:
			programmerValuesView?.groupValues ?? EMPTY_PROGRAMMER_VALUES,
		...values,
		directChoices,
		encoderSlots: Array.from(
			{ length: 6 },
			(_, index) => attributes[index] ?? null,
		),
		hardwareConnected: Boolean(
			server.bootstrap?.hardware_connected || state.midiProfile,
		),
	};
}

export type ParameterProjection = ReturnType<typeof useParameterProjection>;
