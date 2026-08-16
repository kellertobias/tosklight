import { useMemo } from "react";
import type { VisualizationSnapshot } from "../../../api/types";
import {
	useDirectEntryUsesProgrammerFade,
	useProgrammerFadeMillis,
} from "../../../features/configuration/ConfigurationState";
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
	projectPushTurnPlacements,
	resolveAnchoredEncoderPage,
} from "./attributeEncoderPages";
import { type ParameterFamily, parameterFamilies } from "./model";
import {
	positionAxisRepresentation,
	positionMovementRepresentation,
} from "./positionMovement";
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
	selectedFixtureIds: readonly string[],
	groupId: string | null,
	active: boolean,
) {
	const group = useSelectedPortableGroup(groupId, active);
	return useMemo(() => {
		const byAttribute = fixtureParameterTargets(selectedFixtureIds, fixtures);
		const result = new Set(byAttribute.keys());
		for (const attribute of selectedGroupSupportedAttributes(groupId, group))
			result.add(attribute);
		return { attributes: result, fixtureIdsByAttribute: byAttribute };
	}, [fixtures, selectedFixtureIds, groupId, group]);
}

export function fixtureParameterTargets(
	selectedFixtureIds: readonly string[],
	fixtures: ReturnType<typeof useSelectedPatchedFixtures>,
) {
	const result = new Map<string, string[]>();
	for (const selectedFixtureId of selectedFixtureIds) {
		const fixture = fixtures.find(
			(candidate) =>
				candidate.fixture_id === selectedFixtureId ||
				candidate.logical_heads.some(
					(head) => head.fixture_id === selectedFixtureId,
				),
		);
		if (!fixture) continue;
		const logicalHead = fixture.logical_heads.find(
			(head) => head.fixture_id === selectedFixtureId,
		);
		const heads = logicalHead
			? fixture.definition.heads.filter(
					(head) => head.index === logicalHead.head_index,
				)
			: fixture.definition.heads.filter((head) => head.shared);
		for (const parameter of heads.flatMap((head) => head.parameters)) {
			const targets = result.get(parameter.attribute) ?? [];
			if (!targets.includes(selectedFixtureId)) targets.push(selectedFixtureId);
			result.set(parameter.attribute, targets);
		}
	}
	return result;
}

function useResolvedValues(
	visualization: VisualizationSnapshot | null,
	selectedFixtureIds: readonly string[],
	fixtures: ReturnType<typeof useSelectedPatchedFixtures>,
) {
	return useMemo(() => {
		const selected = new Set(selectedFixtureIds);
		const normalized = new Map<string, number>();
		const normalizedByFixture = new Map<string, Map<string, number>>();
		const discrete = new Map<string, string>();
		const discreteByFixture = new Map<string, Map<string, string>>();
		const defaultsByFixture = fixtureParameterDefaults(fixtures);
		for (const fixtureId of selectedFixtureIds) {
			const defaults = defaultsByFixture.get(fixtureId);
			if (!defaults) continue;
			normalizedByFixture.set(fixtureId, new Map(defaults));
		}
		for (const entry of visualization?.values ?? []) {
			if (!selected.has(entry.fixture_id)) continue;
			if (entry.value.kind === "normalized") {
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
		for (const fixtureId of selectedFixtureIds)
			for (const [attribute, value] of normalizedByFixture.get(fixtureId) ?? [])
				if (!normalized.has(attribute)) normalized.set(attribute, value);
		return { normalized, normalizedByFixture, discrete, discreteByFixture };
	}, [visualization, selectedFixtureIds, fixtures]);
}

function fixtureParameterDefaults(
	fixtures: ReturnType<typeof useSelectedPatchedFixtures>,
) {
	const defaults = new Map<string, Map<string, number>>();
	for (const fixture of fixtures) {
		for (const head of fixture.definition.heads) {
			const owner = head.shared
				? fixture.fixture_id
				: fixture.logical_heads.find(
						(logical) => logical.head_index === head.index,
					)?.fixture_id;
			if (!owner) continue;
			const values = defaults.get(owner) ?? new Map<string, number>();
			for (const parameter of head.parameters)
				if (typeof parameter.default === "number")
					values.set(parameter.attribute, parameter.default);
			defaults.set(owner, values);
		}
	}
	return defaults;
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
	const placed = (registry ?? []).flatMap((descriptor) =>
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
	return projectPushTurnPlacements(placed);
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
	const movementRepresentation = useMemo(
		() => positionMovementRepresentation(selectedFixtures),
		[selectedFixtures],
	);
	const panRepresentation = useMemo(
		() => positionAxisRepresentation(selectedFixtures, "pan"),
		[selectedFixtures],
	);
	const tiltRepresentation = useMemo(
		() => positionAxisRepresentation(selectedFixtures, "tilt"),
		[selectedFixtures],
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
		selectedFixtureIds,
		selectedGroup,
		active,
	);
	const programmerFadeMillis = useProgrammerFadeMillis();
	const directEntryUsesProgrammerFade = useDirectEntryUsesProgrammerFade();
	const registry = useAttributeRegistry();
	const values = useResolvedValues(
		visualization,
		selectedFixtureIds,
		selectedFixtures,
	);
	const encoderGroups = useMemo(
		() =>
			attributeEncoderGroups(
				placedRegistry(registry),
				supported.attributes,
				visibleEncoderCount,
			),
		[registry, supported.attributes, visibleEncoderCount],
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
		supported.attributes.has(attribute),
	);
	const hasConfiguredFamily = Boolean(configuredGroup?.pages.length);
	const encoderSlots = hasConfiguredFamily
		? (configuredPage?.slots.map((descriptor) => descriptor?.id ?? null) ??
			Array.from<null>({ length: visibleEncoderCount }).fill(null))
		: Array.from(
				{ length: visibleEncoderCount },
				(_, index) => fallbackAttributes[index] ?? null,
			);
	const encoderPushTurnSlots = hasConfiguredFamily
		? (configuredPage?.slots.map(
				(descriptor) => descriptor?.push_turn_attribute ?? null,
			) ?? Array.from<null>({ length: visibleEncoderCount }).fill(null))
		: Array.from<null>({ length: visibleEncoderCount }).fill(null);
	const attributeLabels = new Map(
		(registry ?? []).map((descriptor) => [descriptor.id, descriptor.label]),
	);
	return {
		programmerActions,
		state,
		active,
		programmerFadeMillis: programmerFadeMillis ?? undefined,
		directEntryUsesProgrammerFade,
		selectedFixtureIds,
		selectedFixtures,
		supportedFixtureIdsByAttribute: supported.fixtureIdsByAttribute,
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
		dynamicProgrammerValues:
			programmerValuesView?.dynamicValues ?? EMPTY_PROGRAMMER_VALUES,
		...values,
		encoderGroups,
		encoderPage: resolvedPage,
		encoderPageCount: Math.max(1, configuredGroup?.pages.length ?? 0),
		encoderSlots,
		encoderPushTurnSlots,
		visibleEncoderCount,
		attributeLabels,
		movementRepresentation,
		panRepresentation,
		tiltRepresentation,
		hardwareConnected,
	};
}

export type ParameterProjection = ReturnType<typeof useParameterProjection>;
