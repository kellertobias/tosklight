import { useMemo } from "react";
import { useServer } from "../../../api/ServerContext";
import type { VisualizationSnapshot } from "../../../api/types";
import { capturesProgrammerWrites } from "../../../features/programmerCaptureMode/contracts";
import { useProgrammerCaptureModeView } from "../../../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerFadeMillis } from "../../../features/configuration/ConfigurationState";
import { useSelectedPatchedFixtures } from "../../../features/patch/PatchState";
import { selectedGroupId } from "../../../features/programmingInteraction/contracts";
import { useProgrammingSelectionView } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { useVisualizationRuntimeSnapshot } from "../../../features/visualizationRuntime/VisualizationRuntimeView";
import { useApp } from "../../../state/AppContext";
import {
	directProgrammerChoices,
	type ParameterFamily,
	parameterFamilies,
} from "./model";
import { useParameterPreloadValues } from "./useParameterPreloadValues";
import { useParameterProgrammerValues } from "./useParameterProgrammerValues";
import {
	selectedGroupSupportedAttributes,
	useSelectedPortableGroup,
} from "./useSelectedPortableGroup";

const EMPTY_FIXTURE_IDS: readonly string[] = [];
const EMPTY_PROGRAMMER_VALUES: readonly never[] = [];

function useVisualization(
	active: boolean,
	selectedFixtureIds: readonly string[],
) {
	return useVisualizationRuntimeSnapshot({
		enabled: active && selectedFixtureIds.length > 0,
		intervalMillis: 400,
	});
}

function useSupportedAttributes(
	selectedFixtureIds: readonly string[],
	groupId: string | null,
	active: boolean,
) {
	const group = useSelectedPortableGroup(groupId, active);
	const fixtures = useSelectedPatchedFixtures(selectedFixtureIds, active);
	return useMemo(() => {
		const result = new Set<string>();
		for (const fixture of fixtures)
			for (const head of fixture.definition.heads ?? [])
				for (const parameter of head.parameters)
					result.add(parameter.attribute);
		for (const attribute of selectedGroupSupportedAttributes(groupId, group))
			result.add(attribute);
		return result;
	}, [fixtures, groupId, group]);
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
	const captureMode = useProgrammerCaptureModeView(active);
	const preloadCaptureActive = capturesProgrammerWrites(captureMode);
	const normalValuesView = useParameterProgrammerValues(
		selectedFixtureIds,
		selectedGroup,
		active && captureMode !== null && !preloadCaptureActive,
	);
	const preloadValuesView = useParameterPreloadValues(
		selectedFixtureIds,
		selectedGroup,
		active && preloadCaptureActive,
	);
	const programmerValuesView = captureMode
		? preloadCaptureActive
			? preloadValuesView
			: normalValuesView
		: null;
	const visualization = useVisualization(active, selectedFixtureIds);
	const supported = useSupportedAttributes(
		selectedFixtureIds,
		selectedGroup,
		active,
	);
	const programmerFadeMillis = useProgrammerFadeMillis();
	const values = useResolvedValues(visualization, selectedFixtureIds);
	const selectedFixtures = useSelectedPatchedFixtures(
		selectedFixtureIds,
		active,
	);
	const directChoices = useMemo(
		() => directProgrammerChoices(selectedFixtures, selectedFixtureIds),
		[selectedFixtures, selectedFixtureIds],
	);
	const attributes = parameterFamilies[family].filter((attribute) =>
		supported.has(attribute),
	);
	return {
		server,
		state,
		active,
		programmerFadeMillis: programmerFadeMillis ?? undefined,
		selectedFixtureIds,
		selectedGroupId: selectedGroup,
		programmerValuesRoute: captureMode
			? preloadCaptureActive
				? ("preload" as const)
				: ("normal" as const)
			: null,
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
