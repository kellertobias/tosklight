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
	type MutableRefObject,
	type ReactNode,
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
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
} from "../../components/control/parameterControls/VisibleEncoderCount";
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
import type { DynamicEditorView } from "./DynamicsEditor";
import {
	clamp,
	defaultRandomGroup,
	isSpatialOrdering,
	keyframeName,
	laneShapeLabel,
	normalizeDegrees,
	primaryInterpolationIndex,
	primaryInterpolationLabel,
	rationalFromNumber,
	rationalValue,
	setPrimaryInterpolation,
	sourceCurrent,
	wrappedIndex,
} from "./DynamicsEditor";

type PresetObject = ShowObject<"preset">;

interface DynamicEncoderDeckProps {
	view: DynamicEditorView;
	page?: number;
	lane?: DynamicLaneProjection;
	dynamic: DynamicDefinitionProjection;
	presets?: readonly PresetObject[];
	keyframeIndex?: number;
	onKeyframeIndex?(index: number): void;
	onLaneChange(
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		mutationGroup?: string,
	): Promise<void>;
	onMutate(intent: DynamicUpdateIntent, mutationGroup?: string): Promise<void>;
}

export function DynamicEncoderDeck({
	view,
	page = 1,
	lane,
	dynamic,
	presets = [],
	keyframeIndex = 0,
	onKeyframeIndex = () => undefined,
	onLaneChange,
	onMutate,
}: DynamicEncoderDeckProps) {
	const hardwareConnected = useDynamicsHardwareConnected();
	const softwareEncoderCount = useVisibleEncoderCount();
	const visibleEncoderCount = resolveVisibleEncoderCount(
		softwareEncoderCount,
		hardwareConnected,
	);
	const hardwareDisplays = useRef<Array<HardwareEncoderDisplayHandle | null>>(
		[],
	);
	const gesture = useRef<{
		key: string;
		id: string;
		lastSampleAt: number;
	} | null>(null);
	const accumulated = useRef(
		new Map<string, { observedBase: number; value: number }>(),
	);
	const { phase, applyPhase } = dynamicPhaseBinding(
		dynamic,
		lane,
		onLaneChange,
		onMutate,
	);
	const allSlots =
		view === "curves"
			? curveEditorEncoderSlots(
					lane,
					dynamic,
					keyframeIndex,
					onKeyframeIndex,
					onLaneChange,
					presets,
				)
			: view === "phase"
				? phaseEncoderSlots(phase, applyPhase)
				: speedEncoderSlots(dynamic, onMutate);
	const pageCount = Math.max(
		1,
		Math.ceil(allSlots.length / visibleEncoderCount),
	);
	const visiblePage = Math.min(Math.max(1, page), pageCount);
	const slots = allSlots.slice(
		(visiblePage - 1) * visibleEncoderCount,
		visiblePage * visibleEncoderCount,
	);
	const items = encoderSectionItems(slots);
	const slotsRef = useRef(slots);
	slotsRef.current = slots;
	const groupFor = useCallback((key: string) => {
		const now = performance.now();
		if (
			!gesture.current ||
			gesture.current.key !== key ||
			now - gesture.current.lastSampleAt > 250
		)
			gesture.current = {
				key,
				id: crypto.randomUUID(),
				lastSampleAt: now,
			};
		else gesture.current.lastSampleAt = now;
		return gesture.current.id;
	}, []);
	const applyRelative = useCallback(
		(id: string, delta: number, undoGroup?: string | null) => {
			const slot = slotsRef.current.find((candidate) => candidate.id === id);
			if (!slot || slot.disabled) return;
			const key = `${view}:${id}`;
			const current = accumulated.current.get(key);
			const externalChange =
				current &&
				slot.value !== current.observedBase &&
				slot.value !== current.value;
			const base = !current || externalChange ? slot.value : current.value;
			const next = clamp(base + delta, slot.minimum, slot.maximum);
			accumulated.current.set(key, {
				observedBase: slot.value,
				value: next,
			});
			const group = undoGroup ?? groupFor(id);
			void slot.apply(next, group);
		},
		[groupFor, view],
	);
	const applyAbsolute = useCallback(
		(id: string, value: number) => {
			const slot = slotsRef.current.find((candidate) => candidate.id === id);
			if (!slot || slot.disabled) return;
			const next = clamp(value, slot.minimum, slot.maximum);
			accumulated.current.set(`${view}:${id}`, {
				observedBase: slot.value,
				value: next,
			});
			void slot.apply(next, crypto.randomUUID());
		},
		[view],
	);
	const applyRange = useCallback((id: string, points: number[]) => {
		const slot = slotsRef.current.find((candidate) => candidate.id === id);
		if (!slot?.applyRange || slot.disabled) return;
		const scale = slot.inputScale || 1;
		void slot.applyRange(
			points.map((point) => clamp(point / scale, slot.minimum, slot.maximum)),
			crypto.randomUUID(),
		);
	}, []);
	const selectPreset = useCallback((id: string, value: string) => {
		const slot = slotsRef.current.find((candidate) => candidate.id === id);
		if (!slot?.selectPreset || slot.disabled) return;
		void slot.selectPreset(value, crypto.randomUUID());
	}, []);
	useEffect(() => {
		accumulated.current.clear();
		gesture.current = null;
	}, [dynamic.id]);
	useHardwareEncoderActions(
		hardwareConnected,
		slotsRef,
		hardwareDisplays,
		applyRelative,
		groupFor,
	);
	return (
		<div className="dynamic-encoder-deck">
			<EncoderSection
				showHeader={false}
				model={{
					id: `dynamics-${view}-${visiblePage}`,
					label: `${view === "curves" ? "Lanes" : view === "phase" ? "Phase" : "Speed"} encoders`,
					description: "Turn fine · press-turn coarse · center Set Value",
					encoders: items,
				}}
				surface={hardwareConnected ? "hardware" : "touch"}
				callbacks={{
					onRelativeChange: applyRelative,
					onAbsoluteChange: applyAbsolute,
					onRangeChange: applyRange,
					onPresetSelect: selectPreset,
					onHardwareDisplayRef: (slot, handle) => {
						hardwareDisplays.current[slot - 1] = handle;
					},
				}}
			/>
		</div>
	);
}

function useDynamicsHardwareConnected() {
	const { state } = useApp();
	const attached = useHardwareConnected();
	return Boolean(attached || state.midiProfile);
}

function dynamicPhaseBinding(
	dynamic: DynamicDefinitionProjection,
	lane: DynamicLaneProjection | undefined,
	onLaneChange: (
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		group?: string,
	) => Promise<void>,
	onMutate: (
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) => Promise<void>,
) {
	const phase =
		dynamic.phase_mode === "per_lane"
			? (lane?.phase ?? dynamic.phase)
			: dynamic.phase;
	const applyPhase = (
		nextPhase: DynamicDefinitionProjection["phase"],
		group?: string,
	) =>
		dynamic.phase_mode === "per_lane" && lane
			? onLaneChange((current) => ({ ...current, phase: nextPhase }), group)
			: onMutate({ type: "set_phase", phase: nextPhase }, group);
	return { phase, applyPhase };
}

function encoderSectionItems(
	slots: DynamicEncoderSlot[],
): EncoderSectionItem[] {
	return slots.map((slot, index) => ({
		id: slot.id,
		slot: index + 1,
		target: { label: slot.label, display: slot.display },
		value: slot.value,
		minimum: slot.minimum,
		maximum: slot.maximum,
		inputScale: slot.inputScale,
		slowStep: slot.fineStep,
		fastStep: slot.coarseStep,
		repeatSeconds: 0.08,
		disabled: slot.disabled,
		presets: slot.choices ?? slot.presets,
		range: Boolean(slot.applyRange),
		touchInteraction: slot.choices ? "choices" : undefined,
	}));
}

function useHardwareEncoderActions(
	hardwareConnected: boolean,
	slotsRef: MutableRefObject<DynamicEncoderSlot[]>,
	hardwareDisplays: MutableRefObject<
		Array<HardwareEncoderDisplayHandle | null>
	>,
	applyRelative: (id: string, delta: number, group?: string | null) => void,
	groupFor: (id: string) => string,
) {
	useEffect(() => {
		if (!hardwareConnected) return;
		const handleEncoder = (event: Event) => {
			const detail = (event as CustomEvent<{ control: string; value?: string }>)
				.detail;
			const slotNumber = Number(detail.control.split("/")[1]);
			const slot = slotsRef.current[slotNumber - 1];
			if (!slot || slot.disabled) return;
			if (detail.value === "press") {
				hardwareDisplays.current[slotNumber - 1]?.activate();
				return;
			}
			const direction =
				detail.value === "up" || detail.value === "right"
					? 1
					: detail.value === "down" || detail.value === "left"
						? -1
						: 0;
			if (!direction) return;
			const coarse = detail.value === "left" || detail.value === "right";
			applyRelative(
				slot.id,
				direction * (coarse ? slot.coarseStep : slot.fineStep),
				groupFor(slot.id),
			);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	}, [applyRelative, groupFor, hardwareConnected, hardwareDisplays, slotsRef]);
}

function phaseEncoderSlots(
	phase: DynamicDefinitionProjection["phase"],
	applyPhase: (
		phase: DynamicDefinitionProjection["phase"],
		group?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	return [
		{
			id: "offset",
			label: "Offset",
			display: `${phase.offset_degrees}°`,
			value: phase.offset_degrees,
			minimum: -10_000,
			maximum: 10_000,
			inputScale: 1,
			fineStep: 5,
			coarseStep: 45,
			apply: (value, group) =>
				applyPhase({ ...phase, offset_degrees: value }, group),
			applyRange: (values, group) =>
				applyPhase(phaseWithExplicitRange(phase, values), group),
		},
		{
			id: "span",
			label: "Span",
			display: `${phase.span_degrees}°`,
			value: phase.span_degrees,
			minimum: -10_000,
			maximum: 10_000,
			inputScale: 1,
			fineStep: 5,
			coarseStep: 45,
			apply: (value, group) =>
				applyPhase(
					{ ...phase, span_degrees: value, anchors_degrees: [] },
					group,
				),
			applyRange: (values, group) =>
				applyPhase(phaseWithExplicitRange(phase, values), group),
		},
		{
			id: "blocks",
			label: "Blocks",
			display: String(phase.block_size),
			value: phase.block_size,
			minimum: 1,
			maximum: 10_000,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 5,
			apply: (value, group) =>
				applyPhase(
					{ ...phase, block_size: Math.max(1, Math.round(value)) },
					group,
				),
		},
		{
			id: "repeats",
			label: "Repeats",
			display: String(phase.repeats),
			value: phase.repeats,
			minimum: 1,
			maximum: 10_000,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 5,
			apply: (value, group) =>
				applyPhase(
					{ ...phase, repeats: Math.max(1, Math.round(value)) },
					group,
				),
		},
		{
			id: "wings",
			label: "Wings",
			display: phase.wings ? "On" : "Off",
			value: phase.wings ? 1 : 0,
			minimum: 0,
			maximum: 1,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			choices: encoderChoices("Wings", phase.wings ? 1 : 0, [
				{ label: "Off" },
				{ label: "On" },
			]),
			apply: (value, group) =>
				applyPhase({ ...phase, wings: value >= 0.5 }, group),
		},
		phaseDirectionSlot(phase, applyPhase),
	];
}

export interface DynamicEncoderSlot {
	id: string;
	label: string;
	display: string;
	value: number;
	minimum: number;
	maximum: number;
	inputScale: number;
	fineStep: number;
	coarseStep: number;
	disabled?: boolean;
	choices?: NonNullable<EncoderSectionItem["presets"]>;
	presets?: NonNullable<EncoderSectionItem["presets"]>;
	apply(value: number, mutationGroup: string): Promise<void>;
	applyRange?(values: number[], mutationGroup: string): Promise<void>;
	selectPreset?(value: string, mutationGroup: string): Promise<void>;
}

export function encoderChoices(
	groupLabel: string,
	selectedValue: number,
	options: readonly {
		label: string;
		description?: string;
		disabled?: boolean;
	}[],
): NonNullable<EncoderSectionItem["presets"]> {
	return {
		selectedValue: String(selectedValue),
		groups: [
			{
				label: groupLabel,
				options: options.map((option, index) => ({
					value: String(index),
					label: option.label,
					description: option.description,
					disabled: option.disabled,
				})),
			},
		],
	};
}

export {
	curveFunctionSelectionGroups,
	normalizePwmLane,
	scalarSourceEncoderDisplay,
} from "./CurveEncoderSlots";

import { curveEditorEncoderSlots } from "./CurveEncoderSlots";

function phaseWithExplicitRange(
	phase: DynamicDefinitionProjection["phase"],
	values: number[],
): DynamicDefinitionProjection["phase"] {
	if (values.length < 2 || values.some((value) => !Number.isFinite(value)))
		return phase;
	const offset = values[0];
	const anchors = values.map((value) => value - offset);
	return {
		...phase,
		offset_degrees: offset,
		span_degrees: anchors.at(-1) ?? phase.span_degrees,
		anchors_degrees: anchors,
	};
}

function phaseDirectionSlot(
	phase: DynamicDefinitionProjection["phase"],
	applyPhase: (
		phase: DynamicDefinitionProjection["phase"],
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot {
	const ordering = phase.ordering;
	const direction = ordering.type === "grid_linear";
	const spatial = isSpatialOrdering(ordering);
	const value = direction
		? ordering.angle_degrees
		: spatial
			? ordering.center_x
			: 0;
	return {
		id: "direction-center",
		label: direction
			? "Direction"
			: spatial
				? "Center X"
				: "Direction / center",
		display: direction ? `${value}°` : spatial ? String(value) : "Unavailable",
		value,
		minimum: direction ? 0 : -10_000,
		maximum: direction ? 359 : 10_000,
		inputScale: 1,
		fineStep: direction ? 5 : 0.1,
		coarseStep: direction ? 45 : 1,
		disabled: !direction && !spatial,
		apply: (next, group) =>
			applyPhase(
				{
					...phase,
					ordering: direction
						? { ...ordering, angle_degrees: normalizeDegrees(next) }
						: spatial
							? { ...ordering, center_x: next }
							: ordering,
				},
				group,
			),
	};
}

function speedEncoderSlots(
	dynamic: DynamicDefinitionProjection,
	onMutate: DynamicMutation,
): DynamicEncoderSlot[] {
	const speedGroups = ["A", "B", "C", "D", "E"] as const;
	const speed = dynamic.speed;
	const fixed = speed.type === "fixed";
	const source = fixed ? 0 : speedGroups.indexOf(speed.group) + 1;
	const activations = ["start_now", "join_sync_now", "next_boundary"] as const;
	return [
		speedSourceSlot(source, speedGroups, onMutate),
		cycleDurationSlot(speed, fixed, onMutate),
		overallSpeedSlot(dynamic, onMutate),
		activationSlot(dynamic, fixed, activations, onMutate),
		boundarySlot(dynamic, onMutate),
		runModeSlot(dynamic, onMutate),
	];
}

type DynamicMutation = (
	intent: DynamicUpdateIntent,
	mutationGroup?: string,
) => Promise<void>;

function speedSourceSlot(
	value: number,
	groups: readonly SpeedGroupId[],
	onMutate: DynamicMutation,
): DynamicEncoderSlot {
	return {
		id: "speed-source",
		label: "Speed source",
		display: value === 0 ? "Fixed" : `Group ${groups[value - 1]}`,
		value,
		minimum: 0,
		maximum: groups.length,
		inputScale: 1,
		fineStep: 1,
		coarseStep: 1,
		choices: encoderChoices("Speed source", value, [
			{ label: "Fixed" },
			...groups.map((group) => ({ label: `Group ${group}` })),
		]),
		apply: (next, mutationGroup) =>
			onMutate(
				{
					type: "set_speed",
					speed:
						wrappedIndex(next, groups.length + 1) === 0
							? { type: "fixed", duration_millis: 4000 }
							: {
									type: "speed_group",
									group: groups[wrappedIndex(next, groups.length + 1) - 1],
									beats_per_cycle: { numerator: 4, denominator: 1 },
								},
				},
				mutationGroup,
			),
	};
}

function cycleDurationSlot(
	speed: DynamicDefinitionProjection["speed"],
	fixed: boolean,
	onMutate: DynamicMutation,
): DynamicEncoderSlot {
	const value =
		speed.type === "fixed"
			? speed.duration_millis / 1000
			: rationalValue(speed.beats_per_cycle);
	const display =
		speed.type === "fixed"
			? `${(speed.duration_millis / 1000).toFixed(2)} s`
			: `${speed.beats_per_cycle.numerator}/${speed.beats_per_cycle.denominator}`;
	return {
		id: fixed ? "duration" : "beats-cycle",
		label: fixed ? "Duration" : "Beats / cycle",
		display,
		value,
		minimum: fixed ? 0.001 : 0.0625,
		maximum: fixed ? 3600 : 64,
		inputScale: 1,
		fineStep: fixed ? 0.1 : 0.25,
		coarseStep: 1,
		apply: (next, group) =>
			onMutate(
				{
					type: "set_speed",
					speed:
						speed.type === "fixed"
							? {
									type: "fixed",
									duration_millis: Math.max(1, Math.round(next * 1000)),
								}
							: { ...speed, beats_per_cycle: rationalFromNumber(next) },
				},
				group,
			),
	};
}

function overallSpeedSlot(
	dynamic: DynamicDefinitionProjection,
	onMutate: DynamicMutation,
): DynamicEncoderSlot {
	const multiplier = dynamic.overall_speed_multiplier;
	return {
		id: "overall-speed",
		label: "Overall speed",
		display: `${multiplier.numerator}/${multiplier.denominator}`,
		value: rationalValue(multiplier),
		minimum: 0.0625,
		maximum: 16,
		inputScale: 1,
		fineStep: 0.0625,
		coarseStep: 0.5,
		apply: (value, group) =>
			onMutate(
				{
					type: "set_overall_speed_multiplier",
					multiplier: rationalFromNumber(value),
				},
				group,
			),
	};
}

function activationSlot(
	dynamic: DynamicDefinitionProjection,
	fixed: boolean,
	modes: readonly DynamicDefinitionProjection["default_activation"][],
	onMutate: DynamicMutation,
): DynamicEncoderSlot {
	const value = modes.indexOf(dynamic.default_activation);
	return {
		id: "activation",
		label: "Activation",
		display: dynamic.default_activation.replaceAll("_", " "),
		value,
		minimum: 0,
		maximum: modes.length - 1,
		inputScale: 1,
		fineStep: 1,
		coarseStep: 1,
		choices: encoderChoices("Activation", value, [
			{ label: "Start now" },
			{ label: "Join sync now", disabled: fixed },
			{ label: "Next boundary", disabled: fixed },
		]),
		apply: (next, group) =>
			onMutate(
				{
					type: "set_activation",
					activation: modes[fixed ? 0 : wrappedIndex(next, modes.length)],
				},
				group,
			),
	};
}

function boundarySlot(
	dynamic: DynamicDefinitionProjection,
	onMutate: DynamicMutation,
): DynamicEncoderSlot {
	const value = dynamic.activation_boundary === "bar" ? 1 : 0;
	return {
		id: "boundary",
		label: "Boundary",
		display:
			dynamic.default_activation === "next_boundary"
				? dynamic.activation_boundary
				: "Unavailable",
		value,
		minimum: 0,
		maximum: 1,
		inputScale: 1,
		fineStep: 1,
		coarseStep: 1,
		disabled: dynamic.default_activation !== "next_boundary",
		choices: encoderChoices("Boundary", value, [
			{ label: "Next beat" },
			{ label: "Next bar", description: "Four beats" },
		]),
		apply: (next, group) =>
			onMutate(
				{
					type: "set_activation_boundary",
					boundary: next >= 0.5 ? "bar" : "beat",
				},
				group,
			),
	};
}

function runModeSlot(
	dynamic: DynamicDefinitionProjection,
	onMutate: DynamicMutation,
): DynamicEncoderSlot {
	const value = dynamic.run_mode === "one_shot" ? 1 : 0;
	return {
		id: "run-mode",
		label: "Run mode",
		display: value ? "One-shot" : "Loop",
		value,
		minimum: 0,
		maximum: 1,
		inputScale: 1,
		fineStep: 1,
		coarseStep: 1,
		choices: encoderChoices("Run mode", value, [
			{ label: "Loop" },
			{ label: "One-shot" },
		]),
		apply: (next, group) =>
			onMutate(
				{
					type: "set_run_mode",
					run_mode: next >= 0.5 ? "one_shot" : "loop",
				},
				group,
			),
	};
}
