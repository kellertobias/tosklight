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

import {
	clamp,
	curveComposerMethods,
	defaultRandomGroup,
	keyframeName,
	keyframePreviewPercent,
	keyframePreviewTop,
	lanePreview,
	laneShapeLabel,
	laneSpeedLabel,
	LaneAttributeModal,
	largestKeyframeGapMidpoint,
	modeLabel,
	sourceCurrent,
} from "./DynamicsEditor";
import {
	curveFunctionSelectionGroups,
	normalizePwmLane,
	scalarSourceEncoderDisplay,
} from "./DynamicEncoderDeck";
import { CurvesViewSurface } from "./CurvesViewSurface";

type DynamicObject = ShowObject<"dynamic">;

export function CurvesView({
	dynamic,
	lane,
	selectedLanes,
	shiftArmed,
	attributes,
	primaryKeyframeIndex,
	previewPhase,
	contentSidebar,
	onPrimaryKeyframeIndex,
	onSelect,
	onReplace,
	onMutate,
}: {
	dynamic: DynamicObject;
	lane: DynamicLaneProjection;
	selectedLanes: ReadonlySet<string>;
	shiftArmed: boolean;
	attributes: readonly { id: string; label: string; family: string }[];
	primaryKeyframeIndex: number;
	previewPhase: number | null;
	contentSidebar?: ReactNode;
	onPrimaryKeyframeIndex(index: number): void;
	onSelect(id: string, additive: boolean): void;
	onReplace(next: DynamicLaneProjection): Promise<void>;
	onMutate(
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<void>;
}) {
	const [attributeLaneId, setAttributeLaneId] = useState<string | null>(null);
	const [openLaneMenuId, setOpenLaneMenuId] = useState<string | null>(null);
	const [draggingKeyframe, setDraggingKeyframe] = useState<{
		laneId: string;
		index: number;
		pointerId: number;
		mutationGroup: string;
		grabOffsetX: number;
	} | null>(null);
	const [randomMethod, setRandomMethod] = useState<
		"max_min" | "middle_amplitude"
	>("max_min");
	const setMode = async (mode: DynamicLaneModeProjection) => {
		if (mode !== "random") {
			await onReplace({ ...lane, mode });
			return;
		}
		let groupId = lane.random_group_id;
		if (
			!groupId ||
			!dynamic.body.random_groups.some((group) => group.id === groupId)
		) {
			const group = defaultRandomGroup();
			await onMutate(dynamic, { type: "add_random_group", group });
			groupId = group.id;
		}
		await onReplace({ ...lane, mode, random_group_id: groupId });
	};
	const displayedMethod = lane.mode === "random" ? randomMethod : lane.mode;
	const chooseMethod = (mode: "keyframes" | "max_min" | "middle_amplitude") => {
		if (lane.mode === "random" && mode !== "keyframes") {
			setRandomMethod(mode);
			return;
		}
		void setMode(mode);
	};
	const chooseFunction = (
		functionName: DynamicPeriodicFunctionProjection | "random",
	) => {
		if (functionName === "random") {
			if (displayedMethod !== "keyframes") setRandomMethod(displayedMethod);
			void setMode("random");
			return;
		}
		const method =
			displayedMethod === "middle_amplitude" ? "middle_amplitude" : "max_min";
		void onReplace(
			normalizePwmLane(
				method === "middle_amplitude"
					? {
							...lane,
							mode: method,
							middle_amplitude: {
								...lane.middle_amplitude,
								function: functionName,
							},
						}
					: {
							...lane,
							mode: method,
							max_min: { ...lane.max_min, function: functionName },
						},
			),
		);
	};
	const attributeLane = attributeLaneId
		? dynamic.body.lanes.find((candidate) => candidate.id === attributeLaneId)
		: undefined;
	const keyframeIndex = Math.min(
		primaryKeyframeIndex,
		Math.max(0, lane.keyframes.points.length - 1),
	);
	const selectedFunction =
		lane.mode === "random"
			? "random"
			: lane.mode === "middle_amplitude"
				? lane.middle_amplitude.function
				: lane.max_min.function;
	const moveKeyframe = createMoveKeyframeAction(dynamic, onMutate);
	return (
		<CurvesViewSurface
			dynamic={dynamic}
			lane={lane}
			selectedLanes={selectedLanes}
			shiftArmed={shiftArmed}
			attributes={attributes}
			keyframeIndex={keyframeIndex}
			previewPhase={previewPhase}
			contentSidebar={contentSidebar}
			attributeLane={attributeLane}
			openLaneMenuId={openLaneMenuId}
			draggingKeyframe={draggingKeyframe}
			displayedMethod={displayedMethod}
			selectedFunction={selectedFunction}
			onPrimaryKeyframeIndex={onPrimaryKeyframeIndex}
			onSelect={onSelect}
			onReplace={onReplace}
			onMutate={onMutate}
			onAttributeLane={setAttributeLaneId}
			onToggleLaneMenu={(id) =>
				setOpenLaneMenuId((current) => (current === id ? null : id))
			}
			onCloseLaneMenu={() => setOpenLaneMenuId(null)}
			onDraggingKeyframe={setDraggingKeyframe}
			onMoveKeyframe={moveKeyframe}
			onChooseMethod={chooseMethod}
			onChooseFunction={chooseFunction}
		/>
	);
}

function createMoveKeyframeAction(
	dynamic: DynamicObject,
	onMutate: (
		dynamic: DynamicObject,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	) => Promise<void>,
) {
	return (
		candidate: DynamicLaneProjection,
		index: number,
		clientX: number,
		timeline: HTMLElement,
		mutationGroup: string,
		repetitions: number,
		grabOffsetX: number,
	) => {
		if (index === 0) return;
		const bounds = timeline.getBoundingClientRect();
		const previous = candidate.keyframes.points[index - 1]?.position ?? 0;
		const next = candidate.keyframes.points[index + 1]?.position ?? 0.999;
		const rendered =
			((clientX - grabOffsetX - bounds.left) / Math.max(1, bounds.width)) * 100;
		const position = clamp(
			((rendered - 0.8) / 98.4) * repetitions,
			previous + 0.01,
			next - 0.01,
		);
		const points = candidate.keyframes.points.map((point, pointIndex) =>
			pointIndex === index ? { ...point, position } : point,
		);
		void onMutate(
			dynamic,
			{
				type: "replace_lane",
				lane_id: candidate.id,
				lane: {
					...candidate,
					keyframes: { ...candidate.keyframes, points },
				},
			},
			mutationGroup,
		);
	};
}

export function addKeyframeToLane(
	lane: DynamicLaneProjection,
): DynamicLaneProjection {
	const points = [...lane.keyframes.points];
	const position = largestKeyframeGapMidpoint(points);
	points.push({
		position,
		source: sourceCurrent,
		interpolation: "ease_in_out",
	});
	points.sort((left, right) => left.position - right.position);
	return {
		...lane,
		keyframes: { ...lane.keyframes, points },
	};
}

export function deleteKeyframeFromLane(
	lane: DynamicLaneProjection,
	index: number,
): DynamicLaneProjection {
	if (index <= 0 || lane.keyframes.points.length <= 2) return lane;
	return {
		...lane,
		keyframes: {
			...lane.keyframes,
			points: lane.keyframes.points.filter(
				(_, pointIndex) => pointIndex !== index,
			),
		},
	};
}
