import { useMemo } from "react";
import type { VisualizationSnapshot } from "../../../api/types";
import { useProgrammerFadeMillis } from "../../../features/configuration/ConfigurationState";
import {
	useAttributeRegistry,
	useHardwareConnected,
} from "../../../features/deskSnapshot/DeskSnapshotState";
import { useSelectedPatchedFixtures } from "../../../features/patch/PatchState";
import { useProgrammerActions } from "../../../features/programmerActions/ProgrammerActionsContext";
import { capturesProgrammerWrites } from "../../../features/programmerCaptureMode/contracts";
import { useProgrammerCaptureModeView } from "../../../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { selectedGroupId } from "../../../features/programmingInteraction/contracts";
import { useProgrammingSelectionView } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { useVisualizationRuntimeSnapshot } from "../../../features/visualizationRuntime/VisualizationRuntimeView";
import { useApp } from "../../../state/AppContext";
import {
	type AttributeEncoderPlacement,
	attributeEncoderGroups,
	resolveAnchoredEncoderPage,
} from "./attributeEncoderPages";
import { type ParameterFamily, parameterFamilies } from "./model";
import { useParameterPreloadValues } from "./useParameterPreloadValues";
import { useParameterProgrammerValues } from "./useParameterProgrammerValues";
import {
	selectedGroupSupportedAttributes,
	useSelectedPortableGroup,
} from "./useSelectedPortableGroup";
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
} from "./VisibleEncoderCount";

const EMPTY_FIXTURE_IDS: readonly string[] = [];
const EMPTY_PROGRAMMER_VALUES: readonly never[] = [];

function useVisualization(
	active: boolean,
	selectedFixtureIds: readonly string[],
) {
	return useVisualizationRuntimeSnapshot({
		enabled: active && selectedFixtureIds.length > 0,
		intervalMillis: 400,
		consumerId: "parameter-controls",
	});
}

function useSupportedAttributes(
	fixtures: ReturnType<typeof useSelectedPatchedFixtures>,
	groupId: string | null,
	active: boolean,
) {
	const group = useSelectedPortableGroup(groupId, active);
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

const FAMILY_GROUPS: Record<
	ParameterFamily,
	AttributeEncoderPlacement["encoder_group"]
> = {
	Intensity: "intensity",
	Color: "color",
	Position: "position",
	Beam: "beam",
	Shapers: "shapers",
	Focus: "focus",
	Control: "control",
	Media: "media",
};

function placedRegistry(
	registry: ReturnType<typeof useAttributeRegistry>,
): AttributeEncoderPlacement[] {
	return (registry ?? []).flatMap((descriptor) =>
		descriptor.encoder_group &&
		descriptor.encoder_page != null &&
		descriptor.encoder_slot != null
			? [
					{
						...descriptor,
						encoder_group: descriptor.encoder_group,
						encoder_page: descriptor.encoder_page,
						encoder_slot: descriptor.encoder_slot,
					},
				]
			: [],
	);
}

export function useParameterProjection(
	family: ParameterFamily,
	page = 1,
	active = true,
	pageAnchor: string | null = null,
) {
	const programmerActions = useProgrammerActions();
	const hardwareAttached = useHardwareConnected();
	const { state } = useApp();
	const softwareEncoderCount = useVisibleEncoderCount();
	const hardwareConnected = Boolean(hardwareAttached || state.midiProfile);
	const visibleEncoderCount = resolveVisibleEncoderCount(
		softwareEncoderCount,
		hardwareConnected,
	);
	const selection = useProgrammingSelectionView(active);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	const selectedGroup = selectedGroupId(selection);
	const selectedFixtures = useSelectedPatchedFixtures(
		selectedFixtureIds,
		active,
	);
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
		selectedFixtures,
		selectedGroup,
		active,
	);
	const programmerFadeMillis = useProgrammerFadeMillis();
	const registry = useAttributeRegistry();
	const values = useResolvedValues(visualization, selectedFixtureIds);
	const encoderGroups = useMemo(
		() =>
			attributeEncoderGroups(
				placedRegistry(registry),
				supported,
				visibleEncoderCount,
			),
		[registry, supported, visibleEncoderCount],
	);
	const configuredGroup = encoderGroups.find(
		(group) => group.id === FAMILY_GROUPS[family],
	);
	const resolvedPage = resolveAnchoredEncoderPage(
		configuredGroup,
		page,
		pageAnchor,
	);
	const configuredPage = configuredGroup?.pages[resolvedPage - 1];
	const fallbackAttributes = parameterFamilies[family].filter((attribute) =>
		supported.has(attribute),
	);
	const hasConfiguredFamily = Boolean(configuredGroup?.pages.length);
	const encoderSlots = hasConfiguredFamily
		? (configuredPage?.slots.map((descriptor) => descriptor?.id ?? null) ??
			Array.from<null>({ length: visibleEncoderCount }).fill(null))
		: Array.from(
				{ length: visibleEncoderCount },
				(_, index) => fallbackAttributes[index] ?? null,
			);
	const attributeLabels = new Map(
		(registry ?? []).map((descriptor) => [descriptor.id, descriptor.label]),
	);
	return {
		programmerActions,
		state,
		active,
		programmerFadeMillis: programmerFadeMillis ?? undefined,
		selectedFixtureIds,
		selectedFixtures,
		selectionRevision: selection?.revision ?? 0,
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
		encoderGroups,
		encoderPage: resolvedPage,
		encoderPageCount: Math.max(1, configuredGroup?.pages.length ?? 0),
		encoderSlots,
		visibleEncoderCount,
		attributeLabels,
		hardwareConnected,
	};
}

export type ParameterProjection = ReturnType<typeof useParameterProjection>;
