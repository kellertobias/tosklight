import {
	Button,
	ColorPickerField,
	CyclingValueToggle,
	FadedDivider,
	FormLayout,
	GroupedSelectionField,
	IconPickerField,
	MultiValueToggle,
	MultiValueToggleField,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import {
	EncoderSection,
	type EncoderSectionItem,
	type HardwareEncoderDisplayHandle,
} from "@tosklight/ui/encoders";
import { ModalFrame } from "@tosklight/ui/modals";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import {
	type CSSProperties,
	type Dispatch,
	type ReactNode,
	type SetStateAction,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { createLightApi } from "../../api/client/api";
import type {
	DynamicDefinitionProjection,
	DynamicDefinitionStatusProjection,
	DynamicLaneModeProjection,
	DynamicLaneProjection,
	DynamicPeriodicFunctionProjection,
	DynamicPhaseOrderingProjection,
	DynamicRandomGroupProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicScalarSourceProjection,
	DynamicUpdateIntent,
	SpeedGroupId,
} from "../../api/types";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { monotonicEpochMillis } from "../../components/control/soundToLightAnalyzer";
import { useSoundToLight } from "../../components/control/useSoundToLight";
import {
	useActiveShowId,
	useAttributeRegistry,
	useHardwareConnected,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useDynamicEditorSession } from "../../features/dynamics/DynamicEditorSessionContext";
import { DynamicMutationWriter } from "../../features/dynamics/DynamicMutationWriter";
import { useDynamicsActions } from "../../features/dynamics/DynamicsActionsContext";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../../features/programmingInteraction/ProgrammingInteractionView";
import type { ShowObject } from "../../features/showObjects/contracts";
import {
	useDynamics,
	usePresets,
	useShowObjectsStore,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useSpeedGroupRuntimeView } from "../../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../../state/AppContext";
import { useStageLayout } from "../stageWindow/useStageLayout";
import { DynamicEditorSurface } from "./DynamicEditorSurface";

type DynamicObject = ShowObject<"dynamic">;
type PresetObject = ShowObject<"preset">;
export type DynamicEditorView = "curves" | "phase" | "speed";

export const sourceCurrent: DynamicScalarSourceProjection = { type: "current" };
const sourceZero: DynamicScalarSourceProjection = { type: "value", value: 0 };
const sourceFull: DynamicScalarSourceProjection = { type: "value", value: 1 };
export const curveComposerMethods = [
	{ value: "keyframes", label: "Keyframes" },
	{ value: "max_min", label: "Max / min" },
	{ value: "middle_amplitude", label: "Middle / amplitude" },
] as const;

export interface DynamicEditorProps {
	dynamic: DynamicObject;
	compact: boolean;
	busy: boolean;
	error: string | null;
	attributes: readonly { id: string; label: string; family: string }[];
	presets: readonly PresetObject[];
	runtime: DynamicRuntimeSnapshotProjection | null;
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>;
	selection: readonly string[];
	selectedGroupId: string | null;
	view?: DynamicEditorView;
	onViewChange?(view: DynamicEditorView): void;
	onBack(): void;
	onMutate(
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void>;
	onSpeedGroupTap?(group: SpeedGroupId): void;
	onDelete(): void;
	onMove(poolNumber: number): void;
	onCopy(poolNumber: number): void;
}

/**
 * The production Dynamic editor composition boundary. The connected window owns
 * persistence and runtime refreshes; deterministic renderers can provide those
 * values and callbacks without creating a second version of the editor UI.
 */
export function DynamicEditor({
	dynamic,
	compact,
	busy,
	error,
	attributes,
	runtime,
	speedGroupBpms,
	selection,
	selectedGroupId,
	view: controlledView,
	onViewChange,
	onBack,
	onMutate,
	onSpeedGroupTap,
}: DynamicEditorProps) {
	const { state: appState, dispatch } = useApp();
	const stageLayout = useStageLayout();
	const {
		session,
		open: openEditor,
		update: updateEditor,
	} = useDynamicEditorSession();
	const view: DynamicEditorView =
		controlledView ??
		(session?.dynamicId === dynamic.id ? session.task : "curves");
	const [primaryLane, setPrimaryLane] = useState(
		dynamic.body.lanes[0]?.id ?? null,
	);
	const [selectedLanes, setSelectedLanes] = useState<Set<string>>(
		new Set(primaryLane ? [primaryLane] : []),
	);
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const [addingLane, setAddingLane] = useState(false);
	const [previewing, setPreviewing] = useState(false);
	const [previewPhase, setPreviewPhase] = useState(0);
	const encoderPage =
		session?.dynamicId === dynamic.id ? session.encoderPage : 1;
	const primaryKeyframeIndex =
		session?.dynamicId === dynamic.id ? session.primaryKeyframeIndex : 0;
	const setPrimaryKeyframeIndex = (index: number) =>
		updateEditor({ primaryKeyframeIndex: index });
	const lane =
		dynamic.body.lanes.find((candidate) => candidate.id === primaryLane) ??
		dynamic.body.lanes[0];
	const replaceLane = (next: DynamicLaneProjection, group?: string) =>
		onMutate(
			dynamic,
			{ type: "replace_lane", lane_id: next.id, lane: next },
			group,
		);
	const selectLane = useLaneSelection(
		setPrimaryLane,
		setSelectedLanes,
		updateEditor,
		appState.shiftArmed,
		() => dispatch({ type: "SET_SHIFT_ARMED", value: false }),
	);
	const running = runningCount(runtime, dynamic.id) > 0;
	const status = definitionStatus(runtime, dynamic.id);
	const previewCycleMillis = dynamicPreviewCycleMillis(
		dynamic.body,
		speedGroupBpms,
	);
	const changeView = (next: DynamicEditorView) => {
		onViewChange?.(next);
		updateEditor({ task: next, encoderPage: 1 });
	};
	const addLane = createAddLaneAction(dynamic, setAddingLane, onMutate);
	const { takeSelection, clearSelection } = useTargetBindingActions(
		dynamic,
		selectedGroupId,
		selection,
		onMutate,
	);
	const { contentSidebar, contentFooter } = editorSupplementalContent({
		dynamic,
		view,
		previewPhase: previewing ? previewPhase : 0,
		selection,
		positions: stageLayout.positions,
		positions3d: stageLayout.positions3d,
		running,
		takeSelection,
		clearSelection,
		onMutate,
	});
	useDynamicLaneSynchronization({
		dynamic,
		session,
		primaryLane,
		setPrimaryLane,
		setSelectedLanes,
		updateEditor,
	});
	useDynamicPreviewAnimation({
		previewing,
		previewPhase,
		previewCycleMillis,
		setPreviewPhase,
	});
	useDynamicEditorSessionSync({
		dynamicId: dynamic.id,
		view,
		encoderPage,
		primaryLane,
		primaryKeyframeIndex,
		openEditor,
	});

	return (
		<DynamicEditorSurface
			dynamic={dynamic}
			compact={compact}
			busy={busy}
			error={error}
			attributes={attributes}
			runtime={runtime}
			speedGroupBpms={speedGroupBpms}
			selection={selection}
			view={view}
			lane={lane}
			selectedLanes={selectedLanes}
			shiftArmed={appState.shiftArmed}
			primaryKeyframeIndex={primaryKeyframeIndex}
			previewing={previewing}
			previewPhase={previewPhase}
			settingsAnchor={settingsAnchor}
			addingLane={addingLane}
			running={running}
			status={status}
			contentSidebar={contentSidebar}
			contentFooter={contentFooter}
			onBack={onBack}
			onChangeView={changeView}
			onPreviewing={setPreviewing}
			onPreviewPhase={setPreviewPhase}
			onSettingsAnchor={setSettingsAnchor}
			onAddingLane={setAddingLane}
			onAddLane={addLane}
			onTakeSelection={takeSelection}
			onClearSelection={clearSelection}
			onPrimaryKeyframeIndex={setPrimaryKeyframeIndex}
			onSelectLane={selectLane}
			onReplaceLane={replaceLane}
			onMutate={onMutate}
			onSpeedGroupTap={onSpeedGroupTap}
		/>
	);
}

function editorSupplementalContent({
	dynamic,
	view,
	previewPhase,
	selection,
	positions,
	positions3d,
	running,
	takeSelection,
	clearSelection,
	onMutate,
}: {
	dynamic: DynamicObject;
	view: DynamicEditorView;
	previewPhase: number;
	selection: readonly string[];
	positions: ReturnType<typeof useStageLayout>["positions"];
	positions3d: ReturnType<typeof useStageLayout>["positions3d"];
	running: boolean;
	takeSelection(): unknown;
	clearSelection(): unknown;
	onMutate: DynamicEditorProps["onMutate"];
}) {
	const contentSidebar = (
		<DynamicSelectionPreview
			dynamic={dynamic}
			previewPhase={previewPhase}
			selection={selection}
			positions={positions}
			positions3d={positions3d}
		/>
	);
	const contentFooter =
		view === "phase" ? (
			<DynamicPhaseQuickControls
				phase={dynamic.body.phase}
				running={running}
				selectionCount={selection.length}
				targetless={dynamic.body.target_binding.type === "targetless"}
				onPhasePatch={(patch) =>
					void onMutate(dynamic, {
						type: "set_phase",
						phase: { ...dynamic.body.phase, ...patch },
					})
				}
				onTakeSelection={() => void takeSelection()}
				onClearSelection={() => void clearSelection()}
			/>
		) : null;
	return { contentSidebar, contentFooter };
}

function createAddLaneAction(
	dynamic: DynamicObject,
	setAddingLane: (adding: boolean) => void,
	onMutate: DynamicEditorProps["onMutate"],
) {
	return (attribute: string) => {
		setAddingLane(false);
		void onMutate(dynamic, {
			type: "add_lane",
			lane: createDefaultDynamicLane(attribute),
			index: null,
		});
	};
}

function useTargetBindingActions(
	dynamic: DynamicObject,
	selectedGroupId: string | null,
	selection: readonly string[],
	onMutate: DynamicEditorProps["onMutate"],
) {
	const takeSelection = () =>
		onMutate(dynamic, {
			type: "set_target_binding",
			target_binding: selectedGroupId
				? { type: "live_group", group_id: selectedGroupId }
				: { type: "frozen_targets", targets: [...selection] },
		});
	const clearSelection = () =>
		onMutate(dynamic, {
			type: "set_target_binding",
			target_binding: { type: "targetless" },
		});
	return { takeSelection, clearSelection };
}

function useLaneSelection(
	setPrimaryLane: Dispatch<SetStateAction<string>>,
	setSelectedLanes: Dispatch<SetStateAction<Set<string>>>,
	updateEditor: ReturnType<typeof useDynamicEditorSession>["update"],
	shiftArmed: boolean,
	clearShift: () => void,
) {
	return (id: string, additive: boolean) => {
		setPrimaryLane(id);
		updateEditor({ primaryLaneId: id, primaryKeyframeIndex: 0 });
		setSelectedLanes((current) => {
			if (!additive) return new Set([id]);
			const next = new Set(current);
			if (next.has(id) && next.size > 1) next.delete(id);
			else next.add(id);
			return next;
		});
		if (shiftArmed) clearShift();
	};
}

function useDynamicLaneSynchronization({
	dynamic,
	session,
	primaryLane,
	setPrimaryLane,
	setSelectedLanes,
	updateEditor,
}: {
	dynamic: DynamicObject;
	session: ReturnType<typeof useDynamicEditorSession>["session"];
	primaryLane: string;
	setPrimaryLane(value: string): void;
	setSelectedLanes(value: Set<string>): void;
	updateEditor: ReturnType<typeof useDynamicEditorSession>["update"];
}) {
	useEffect(() => {
		if (
			primaryLane &&
			dynamic.body.lanes.some((candidate) => candidate.id === primaryLane)
		)
			return;
		const nextLane = dynamic.body.lanes[0]?.id ?? null;
		setPrimaryLane(nextLane);
		setSelectedLanes(new Set(nextLane ? [nextLane] : []));
		updateEditor({ primaryLaneId: nextLane, primaryKeyframeIndex: 0 });
	}, [
		dynamic.body.lanes,
		primaryLane,
		setPrimaryLane,
		setSelectedLanes,
		updateEditor,
	]);
	useEffect(() => {
		const sessionLane =
			session?.dynamicId === dynamic.id ? session.primaryLaneId : null;
		if (
			!sessionLane ||
			sessionLane === primaryLane ||
			!dynamic.body.lanes.some((candidate) => candidate.id === sessionLane)
		)
			return;
		setPrimaryLane(sessionLane);
		setSelectedLanes(new Set([sessionLane]));
	}, [
		dynamic.body.lanes,
		dynamic.id,
		primaryLane,
		session,
		setPrimaryLane,
		setSelectedLanes,
	]);
}

function useDynamicPreviewAnimation({
	previewing,
	previewPhase,
	previewCycleMillis,
	setPreviewPhase,
}: {
	previewing: boolean;
	previewPhase: number;
	previewCycleMillis: number;
	setPreviewPhase(value: number): void;
}) {
	useEffect(() => {
		if (!previewing) return;
		let frame = 0;
		const startedAt = performance.now() - previewPhase * previewCycleMillis;
		const animate = (now: number) => {
			setPreviewPhase(
				((now - startedAt) % previewCycleMillis) / previewCycleMillis,
			);
			frame = requestAnimationFrame(animate);
		};
		frame = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(frame);
	}, [previewing, previewCycleMillis]);
}

function useDynamicEditorSessionSync({
	dynamicId,
	view,
	encoderPage,
	primaryLane,
	primaryKeyframeIndex,
	openEditor,
}: {
	dynamicId: string;
	view: DynamicEditorView;
	encoderPage: number;
	primaryLane: string | null;
	primaryKeyframeIndex: number;
	openEditor: ReturnType<typeof useDynamicEditorSession>["open"];
}) {
	useEffect(() => {
		openEditor({
			dynamicId,
			task: view,
			encoderPage,
			primaryLaneId: primaryLane,
			primaryKeyframeIndex,
		});
	}, [
		dynamicId,
		encoderPage,
		openEditor,
		primaryKeyframeIndex,
		primaryLane,
		view,
	]);
}

import {
	DynamicPhaseQuickControls,
	DynamicSelectionPreview,
	dynamicPreviewCycleMillis,
	periodicPreviewValue,
} from "./DynamicPreview";
import { CurvesView } from "./CurvesView";
import { PhaseView, SpeedView } from "./PhaseSpeedViews";
export { DynamicEncoderDeck } from "./DynamicEncoderDeck";
import { DynamicEncoderDeck } from "./DynamicEncoderDeck";
export function LaneAttributeModal({
	id,
	title,
	details,
	currentAttribute,
	attributes,
	busy = false,
	onClose,
	onChoose,
}: {
	id: string;
	title: string;
	details: string;
	currentAttribute?: string;
	attributes: readonly { id: string; label: string; family: string }[];
	busy?: boolean;
	onClose(): void;
	onChoose(attribute: string): void;
}) {
	const groups = attributes.reduce<
		Array<{
			family: string;
			attributes: Array<{ id: string; label: string; family: string }>;
		}>
	>((grouped, attribute) => {
		const family = attribute.family || "Other";
		const group = grouped.find((candidate) => candidate.family === family);
		if (group) group.attributes.push(attribute);
		else grouped.push({ family, attributes: [attribute] });
		return grouped;
	}, []);
	return (
		<ModalFrame
			id={id}
			ariaLabel={title}
			title={title}
			details={details}
			dialogClassName="dynamic-attribute-choice-modal"
			onClose={onClose}
		>
			<div className="dynamic-attribute-choice-scroll">
				<div className="ui-grouped-selection-groups dynamic-attribute-choice-groups">
					{groups.map((group) => (
						<section key={group.family}>
							<h3>{group.family}</h3>
							<div className="ui-grouped-selection-options">
								{group.attributes.map((attribute) => {
									const selected = attribute.id === currentAttribute;
									return (
										<Button
											key={attribute.id}
											active={selected}
											aria-pressed={selected}
											disabled={busy}
											contentAlign="left"
											onClick={() =>
												selected ? onClose() : onChoose(attribute.id)
											}
										>
											<span className="ui-grouped-selection-option has-no-icon">
												<span className="ui-grouped-selection-copy">
													<b>{attribute.label}</b>
												</span>
											</span>
										</Button>
									);
								})}
							</div>
						</section>
					))}
				</div>
				{groups.length === 0 && (
					<p className="dynamic-attribute-choice-empty" role="alert">
						No continuous scalar attributes are available.
					</p>
				)}
			</div>
		</ModalFrame>
	);
}

export function createDefaultDynamicDefinition(
	poolNumber: number,
	attribute: string,
	ids: { definition?: string; lane?: string } = {},
): DynamicDefinitionProjection {
	return {
		id: ids.definition ?? crypto.randomUUID(),
		pool_number: poolNumber,
		revision: 0,
		name: `Dynamic ${poolNumber}`,
		color: "#4edcff",
		icon: "∿",
		target_binding: { type: "targetless" },
		lanes: [createDefaultDynamicLane(attribute, ids.lane)],
		random_groups: [],
		phase_mode: "uniform",
		phase: {
			ordering: { type: "selection" },
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: 4000 },
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

export function createDefaultDynamicLane(
	attribute: string,
	id: string = crypto.randomUUID(),
): DynamicLaneProjection {
	return {
		id,
		attribute,
		mode: "max_min",
		keyframes: {
			points: [
				{
					position: 0,
					source: sourceZero,
					interpolation: "ease_in_out",
				},
				{
					position: 0.5,
					source: sourceFull,
					interpolation: "ease_in_out",
				},
			],
			size: 1,
		},
		max_min: {
			minimum: sourceZero,
			maximum: sourceFull,
			function: "sinus",
			size: 1,
			pwm: defaultPwm(),
		},
		middle_amplitude: {
			middle: { type: "value", value: 0.5 },
			amplitude: 0.5,
			function: "sinus",
			size: 1,
			pwm: defaultPwm(),
		},
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
	};
}

function defaultPwm() {
	return {
		attack: 0,
		on: 0.5,
		decay: 0,
		off: 0.5,
		attack_interpolation: "linear" as const,
		decay_interpolation: "linear" as const,
	};
}

export function defaultRandomGroup(): DynamicRandomGroupProjection {
	return {
		id: crypto.randomUUID(),
		seed: crypto.getRandomValues(new Uint32Array(1))[0] ?? 0,
		low: sourceZero,
		high: sourceFull,
		decision_interval_millis: 250,
		start_probability: 0.25,
		mean_duration_millis: 500,
		duration_spread_millis: 100,
		attack_ratio: 0.1,
		decay_ratio: 0.1,
	};
}

export function largestKeyframeGapMidpoint(
	points: readonly { position: number }[],
): number {
	const positions = [...points.map((point) => point.position), 1].sort(
		(left, right) => left - right,
	);
	let midpoint = 0.5;
	let largest = -1;
	for (let index = 1; index < positions.length; index += 1) {
		const gap = positions[index] - positions[index - 1];
		if (gap > largest) {
			largest = gap;
			midpoint = positions[index - 1] + gap / 2;
		}
	}
	return clamp(midpoint, 0.01, 0.99);
}

function greatestCommonDivisor(left: number, right: number) {
	let a = Math.abs(Math.round(left));
	let b = Math.abs(Math.round(right));
	while (b !== 0) [a, b] = [b, a % b];
	return Math.max(1, a);
}

export function targetSummary(definition: DynamicDefinitionProjection) {
	switch (definition.target_binding.type) {
		case "live_group":
			return `Live Group · ${definition.target_binding.group_id}`;
		case "frozen_targets":
			return `${definition.target_binding.targets.length} frozen targets`;
		case "targetless":
			return "Targetless · resolves at start";
	}
}

export function definitionStatus(
	runtime: DynamicRuntimeSnapshotProjection | null,
	dynamicId: string,
) {
	return (
		runtime?.definitions.find((status) => status.dynamic_id === dynamicId) ??
		null
	);
}

export function coverageSummary(status: DynamicDefinitionStatusProjection) {
	const addresses = status.target_count * status.lane_count;
	return `${status.compatible_target_count}/${status.target_count} compatible targets · ${status.supported_address_count}/${addresses} lane addresses · ${status.unpatched_target_count} unpatched · ${status.missing_target_count} missing`;
}

export function modeLabel(mode: DynamicLaneModeProjection) {
	switch (mode) {
		case "keyframes":
			return "Keyframes";
		case "max_min":
			return "Max / min";
		case "middle_amplitude":
			return "Middle / amplitude";
		case "random":
			return "Random";
	}
}

export function lanePreview(
	lane: DynamicLaneProjection,
	lanes: readonly DynamicLaneProjection[],
) {
	const slowest = Math.min(
		...lanes.map((candidate) =>
			Math.max(0.0001, rationalValue(candidate.speed_multiplier)),
		),
	);
	const repetitions = clamp(
		rationalValue(lane.speed_multiplier) / slowest,
		1,
		16,
	);
	if (lane.mode === "keyframes") {
		const cyclePath = (cycle: number) => {
			const points = [
				...lane.keyframes.points,
				{
					position: 1,
					source: lane.keyframes.points[0]?.source ?? sourceZero,
				},
			]
				.map((point) => ({
					...point,
					timelinePosition: (cycle + point.position) / repetitions,
				}))
				.filter((point) => point.timelinePosition <= 1.0001);
			return points
				.map(
					(point, index) =>
						`${index === 0 ? "M" : "L"}${Math.round(8 + point.timelinePosition * 984)} ${keyframeY(point.source)}`,
				)
				.join(" ");
		};
		return {
			repetitions,
			primaryPath: cyclePath(0),
			repeatedPath: Array.from(
				{ length: Math.max(0, Math.ceil(repetitions) - 1) },
				(_, index) => cyclePath(index + 1),
			)
				.filter(Boolean)
				.join(" "),
		};
	}
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.max_min.function;
	const path = (start: number, end: number) =>
		Array.from({ length: 121 }, (_, index) => {
			const progress = start + ((end - start) * index) / 120;
			const intervalPhase = (progress * repetitions) % 1;
			const width = functionName === "pwm" ? 1 : clamp(lane.width, 0.05, 1);
			const phase = clamp((intervalPhase - (1 - width) / 2) / width, 0, 1);
			const shape = periodicPreviewValue(
				functionName,
				phase,
				lane.mode === "middle_amplitude"
					? lane.middle_amplitude.pwm
					: lane.max_min.pwm,
			);
			const minimum =
				lane.mode === "middle_amplitude"
					? scalarSourceCurveValue(lane.middle_amplitude.middle) -
						lane.middle_amplitude.amplitude
					: scalarSourceCurveValue(lane.max_min.minimum);
			const maximum =
				lane.mode === "middle_amplitude"
					? scalarSourceCurveValue(lane.middle_amplitude.middle) +
						lane.middle_amplitude.amplitude
					: scalarSourceCurveValue(lane.max_min.maximum);
			const value = clamp(minimum + (maximum - minimum) * shape, 0, 1);
			return `${index === 0 ? "M" : "L"}${Math.round(progress * 1000)} ${Math.round(190 - value * 180)}`;
		}).join(" ");
	const firstEnd = 1 / repetitions;
	return {
		repetitions,
		primaryPath: path(0, firstEnd),
		repeatedPath: repetitions > 1 ? path(firstEnd, 1) : "",
	};
}

export function laneSpeedLabel(lane: DynamicLaneProjection) {
	const value = rationalValue(lane.speed_multiplier);
	if (Math.abs(value - 1) < 0.0001) return "";
	return ` · ${lane.speed_multiplier.numerator}/${lane.speed_multiplier.denominator} speed`;
}

export function keyframePreviewPercent(position: number, repetitions: number) {
	return 0.8 + (clamp(position, 0, 1) * 98.4) / repetitions;
}

export function keyframePreviewTop(
	source: DynamicScalarSourceProjection | undefined,
) {
	return 9 + (1 - scalarSourceCurveValue(source)) * 70;
}

function scalarSourceCurveValue(
	source: DynamicScalarSourceProjection | undefined,
) {
	return source?.type === "value" ? source.value : 0.5;
}

export function keyframeName(index: number) {
	return String.fromCharCode(65 + (index % 26));
}

function keyframeY(source: DynamicScalarSourceProjection | undefined) {
	return Math.round(190 - scalarSourceCurveValue(source) * 180);
}

export function rationalValue(value: {
	numerator: number;
	denominator: number;
}) {
	return value.numerator / value.denominator;
}

export function rationalFromNumber(value: number) {
	const denominator = 10_000;
	const numerator = Math.max(1, Math.round(value * denominator));
	const divisor = greatestCommonDivisor(numerator, denominator);
	return {
		numerator: numerator / divisor,
		denominator: denominator / divisor,
	};
}

export function wrappedIndex(value: number, length: number) {
	const rounded = Math.round(value);
	return ((rounded % length) + length) % length;
}

export function laneShapeLabel(lane: DynamicLaneProjection) {
	if (lane.mode === "keyframes") return "Keyframes";
	if (lane.mode === "random") return "Random";
	const value =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.max_min.function;
	switch (value) {
		case "sinus":
			return "Sinus";
		case "cosinus":
			return "Cosinus";
		case "linear_up":
			return "Linear +";
		case "linear_down":
			return "Linear −";
		case "pwm":
			return "PWM";
	}
}

const interpolations = [
	"linear",
	"ease_in",
	"ease_out",
	"ease_in_out",
	"hold",
	"drop",
] as const;

export function primaryInterpolationIndex(lane: DynamicLaneProjection) {
	return interpolations.indexOf(
		lane.keyframes.points[0]?.interpolation ?? "ease_in_out",
	);
}

export function primaryInterpolationLabel(lane: DynamicLaneProjection) {
	if (lane.mode !== "keyframes") return "Unavailable";
	const value = lane.keyframes.points[0]?.interpolation ?? "ease_in_out";
	switch (value) {
		case "linear":
			return "Linear";
		case "ease_in":
			return "Ease in";
		case "ease_out":
			return "Ease out";
		case "ease_in_out":
			return "Ease in + out";
		case "hold":
			return "Hold";
		case "drop":
			return "Drop";
	}
}

export function setPrimaryInterpolation(
	lane: DynamicLaneProjection,
	value: number,
): DynamicLaneProjection {
	if (lane.mode !== "keyframes") return lane;
	const interpolation =
		interpolations[wrappedIndex(value, interpolations.length)];
	return {
		...lane,
		keyframes: {
			...lane.keyframes,
			points: lane.keyframes.points.map((point, index) =>
				index === 0 ? { ...point, interpolation } : point,
			),
		},
	};
}

export function normalizeDegrees(value: number) {
	return ((value % 360) + 360) % 360;
}

export function orderingFor(
	type: string,
	current?: DynamicPhaseOrderingProjection,
): DynamicPhaseOrderingProjection {
	switch (type) {
		case "grid_linear":
			return {
				type,
				angle_degrees:
					current?.type === "grid_linear" ? current.angle_degrees : 90,
			};
		case "radial_out":
		case "radial_in":
		case "axial":
			return {
				type,
				center_x: current && isSpatialOrdering(current) ? current.center_x : 0,
				center_z: current && isSpatialOrdering(current) ? current.center_z : 0,
			};
		case "random_each_loop":
			return {
				type,
				seed: current?.type === "random_each_loop" ? current.seed : Date.now(),
			};
		default:
			return { type: "selection" };
	}
}

export function isSpatialOrdering(
	ordering: DynamicPhaseOrderingProjection,
): ordering is Extract<
	DynamicPhaseOrderingProjection,
	{ type: "radial_out" | "radial_in" | "axial" }
> {
	return (
		ordering.type === "radial_out" ||
		ordering.type === "radial_in" ||
		ordering.type === "axial"
	);
}

export function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}

export function runningCount(
	runtime: DynamicRuntimeSnapshotProjection | null,
	dynamicId: string,
) {
	return (
		runtime?.instances.filter((instance) => instance.dynamic_id === dynamicId)
			.length ?? 0
	);
}
