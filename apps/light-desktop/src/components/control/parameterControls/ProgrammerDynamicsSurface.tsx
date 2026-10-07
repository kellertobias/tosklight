import { formatErrorDetails } from "@tosklight/ui";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	DynamicDefinitionProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicUpdateIntent,
} from "../../../api/types";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import { DynamicMutationWriter } from "../../../features/dynamics/DynamicMutationWriter";
import { useDynamicsActions } from "../../../features/dynamics/DynamicsActionsContext";
import type { ProgrammerDynamicValue } from "../../../features/programmerValues/contracts";
import { useProgrammerValuesView } from "../../../features/programmerValues/ProgrammerValuesView";
import type { ShowObject } from "../../../features/showObjects/contracts";
import {
	useDynamics,
	usePresets,
	useShowObjectsStore,
} from "../../../features/showObjects/ShowObjectsState";
import {
	type DynamicEditorView,
	DynamicEncoderDeck,
} from "../../../windows/DynamicsWindow";
import {
	type DynamicControllerChoice,
	ProgrammerDynamicsInstanceContent,
} from "./ProgrammerDynamicsInstanceContent";
import type { ParameterController } from "./useParameterController";
import { dynamicChoices } from "./programmerDynamicChoices";

type ControllerValueField = "size" | "speed" | "phase";

interface ControllerValueOverride {
	size?: number;
	speed?: number;
	phase?: number;
}

const EMPTY_RUNTIME: DynamicRuntimeSnapshotProjection = {
	global_paused: false,
	instances: [],
	definitions: [],
	speed_groups: [],
};

export function DynamicDefinitionEncoderSurface({
	dynamic,
	presets = [],
	lane,
	view,
	page = 1,
	keyframeIndex = 0,
	onKeyframeIndex,
	onLaneChange,
	onMutate,
}: {
	dynamic: DynamicDefinitionProjection;
	presets?: readonly ShowObject<"preset">[];
	lane: DynamicDefinitionProjection["lanes"][number] | null;
	view: DynamicEditorView;
	page?: number;
	keyframeIndex?: number;
	onKeyframeIndex?(index: number): void;
	onLaneChange(
		change: (
			lane: DynamicDefinitionProjection["lanes"][number],
		) => DynamicDefinitionProjection["lanes"][number],
		mutationGroup?: string,
	): Promise<void>;
	onMutate(intent: DynamicUpdateIntent, mutationGroup?: string): Promise<void>;
}) {
	return (
		<div className="programmer-dynamics-editor-deck">
			<DynamicEncoderDeck
				view={view}
				page={page}
				lane={lane ?? undefined}
				dynamic={dynamic}
				presets={presets}
				keyframeIndex={keyframeIndex}
				onKeyframeIndex={onKeyframeIndex}
				onLaneChange={onLaneChange}
				onMutate={onMutate}
			/>
		</div>
	);
}

export function ProgrammerDynamicsSurface({
	controller,
}: {
	controller: ParameterController;
}) {
	const actions = useDynamicsActions();
	const editor = useDynamicEditorSession();
	const showId = useActiveShowId();
	const definitions = useDynamics();
	const presets = usePresets();
	const showObjectsStore = useShowObjectsStore();
	const mutationWriter = useMemo(
		() =>
			actions
				? new DynamicMutationWriter(showObjectsStore, actions.showObjects)
				: null,
		[actions, showObjectsStore],
	);
	const programmer = useProgrammerValuesView(controller.active);
	const [selectedLaneId, setSelectedLaneId] = useState<string | null>(null);
	const [view, setView] = useState<"instance" | DynamicEditorView>("instance");
	const [error, setError] = useState<string | null>(null);
	const { choices, refresh } = useRuntimeChoices(
		actions,
		showId,
		controller,
		definitions.map((definition) => definition.body),
		programmer?.dynamicValues ?? [],
		setError,
	);
	const { selected, setSelectedControllerId, update, off, cycleChoice } =
		useSelectedController(actions, choices, refresh, setError, () =>
			setSelectedLaneId(null),
		);
	const {
		lanes,
		selectedObject,
		selectedLane,
		cycleLane,
		mutateDefinition,
		changeLane,
	} = useDefinitionEditor(
		editor,
		definitions,
		selected,
		selectedLaneId,
		setSelectedLaneId,
		mutationWriter,
		showId,
		setError,
	);

	useHardwareControllerActions(
		controller.hardwareConnected,
		selected,
		cycleChoice,
		cycleLane,
		update,
		off,
	);

	if (editor.session && selectedObject)
		return (
			<DynamicDefinitionEncoderSurface
				dynamic={selectedObject.body}
				lane={selectedLane}
				view={editor.session.task}
				page={editor.session.encoderPage}
				keyframeIndex={editor.session.primaryKeyframeIndex}
				onKeyframeIndex={(primaryKeyframeIndex) =>
					editor.update({ primaryKeyframeIndex })
				}
				onLaneChange={changeLane}
				onMutate={mutateDefinition}
			/>
		);

	if (!controller.selectedFixtureIds.length && !controller.selectedGroupId)
		return (
			<div className="parameter-empty">
				<b>No fixtures selected</b>
				<small>Select fixtures to inspect their Dynamic instances.</small>
			</div>
		);
	if (!selected)
		return (
			<div className="parameter-empty">
				<b>No matching Dynamic instances</b>
				<small>
					Start a Dynamic from its pool, Cue, or Playback for this selection.
				</small>
			</div>
		);
	return (
		<ProgrammerDynamicsInstanceContent
			controller={controller}
			editor={editor}
			choices={choices}
			selected={selected}
			selectedLane={selectedLane}
			lanes={lanes}
			selectedObject={selectedObject}
			presets={presets}
			view={view}
			error={error}
			onView={setView}
			onController={(controllerId) => {
				setSelectedControllerId(controllerId);
				setSelectedLaneId(null);
			}}
			onLane={setSelectedLaneId}
			onCycleChoice={cycleChoice}
			onCycleLane={cycleLane}
			onUpdate={update}
			onOff={off}
			onLaneChange={changeLane}
			onMutate={mutateDefinition}
		/>
	);
}

function useDefinitionEditor(
	editor: ReturnType<typeof useDynamicEditorSession>,
	definitions: readonly ShowObject<"dynamic">[],
	selected: DynamicControllerChoice | null,
	selectedLaneId: string | null,
	setSelectedLaneId: (id: string | null) => void,
	mutationWriter: DynamicMutationWriter | null,
	showId: ReturnType<typeof useActiveShowId>,
	setError: (error: string | null) => void,
) {
	const lanes = selected?.definition?.lanes ?? [];
	const editorObject = editor.session
		? definitions.find(
				(definition) => definition.id === editor.session?.dynamicId,
			)
		: null;
	const selectedObject =
		editorObject ??
		definitions.find(
			(definition) => definition.id === selected?.instance.dynamic_id,
		);
	const resolvedLanes = editorObject?.body.lanes ?? lanes;
	const selectedLane =
		resolvedLanes.find(
			(lane) => lane.id === (editor.session?.primaryLaneId ?? selectedLaneId),
		) ??
		resolvedLanes[0] ??
		null;
	useEffect(() => {
		if (selectedLane && selectedLane.id !== selectedLaneId)
			setSelectedLaneId(selectedLane.id);
	}, [selectedLane, selectedLaneId, setSelectedLaneId]);
	const cycleLane = useCallback(
		(delta: number) => {
			if (!selectedLane || lanes.length < 2) return;
			const current = lanes.indexOf(selectedLane);
			const next = (current + Math.sign(delta) + lanes.length) % lanes.length;
			setSelectedLaneId(lanes[next].id);
		},
		[lanes, selectedLane, setSelectedLaneId],
	);
	const mutateDefinition = useCallback(
		async (intent: DynamicUpdateIntent, mutationGroup?: string) => {
			if (!mutationWriter || !showId || !selectedObject) return;
			setError(null);
			try {
				await mutationWriter.update(
					showId,
					selectedObject.id,
					intent,
					mutationGroup,
				);
			} catch (cause) {
				setError(formatErrorDetails(cause));
			}
		},
		[mutationWriter, selectedObject, setError, showId],
	);
	const changeLane = useCallback(
		async (
			change: (
				lane: DynamicDefinitionProjection["lanes"][number],
			) => DynamicDefinitionProjection["lanes"][number],
			mutationGroup?: string,
		) => {
			if (!selectedLane) return;
			const next = change(selectedLane);
			await mutateDefinition(
				{ type: "replace_lane", lane_id: next.id, lane: next },
				mutationGroup,
			);
		},
		[mutateDefinition, selectedLane],
	);
	return {
		lanes,
		selectedObject,
		selectedLane,
		cycleLane,
		mutateDefinition,
		changeLane,
	};
}

function useRuntimeChoices(
	actions: ReturnType<typeof useDynamicsActions>,
	showId: ReturnType<typeof useActiveShowId>,
	controller: ParameterController,
	definitions: readonly DynamicDefinitionProjection[],
	stagedValues: readonly ProgrammerDynamicValue[],
	setError: (error: string | null) => void,
) {
	const [runtime, setRuntime] = useState(EMPTY_RUNTIME);
	const refresh = useCallback(async () => {
		if (!actions || !showId) {
			setRuntime(EMPTY_RUNTIME);
			return EMPTY_RUNTIME;
		}
		const next = await actions.dynamics.runtime(showId);
		setRuntime(next);
		return next;
	}, [actions, showId]);
	useEffect(() => {
		void refresh().catch((cause) =>
			setError(formatErrorDetails(cause)),
		);
		if (!controller.active) return;
		const timer = window.setInterval(
			() => void refresh().catch(() => undefined),
			500,
		);
		return () => window.clearInterval(timer);
	}, [controller.active, refresh, setError]);
	const choices = useMemo(
		() =>
			dynamicChoices(
				runtime,
				definitions,
				controller.selectedFixtureIds,
				stagedValues,
			),
		[runtime, definitions, controller.selectedFixtureIds, stagedValues],
	);
	return { choices, refresh };
}

function useSelectedController(
	actions: ReturnType<typeof useDynamicsActions>,
	choices: DynamicControllerChoice[],
	refresh: () => Promise<DynamicRuntimeSnapshotProjection>,
	setError: (error: string | null) => void,
	resetLane: () => void,
) {
	const [selectedControllerId, setSelectedControllerId] = useState<
		string | null
	>(null);
	const [overrides, setOverrides] = useState<
		Record<string, ControllerValueOverride>
	>({});
	const latestWrites = useRef(new Map<string, string>());
	const gesture = useRef<{ field: string; id: string; at: number } | null>(
		null,
	);
	const authoritative =
		choices.find(
			(choice) => choice.controller.controller_id === selectedControllerId,
		) ??
		choices[0] ??
		null;
	const selected = authoritative
		? applyControllerOverride(
				authoritative,
				overrides[authoritative.controller.controller_id],
			)
		: null;
	useEffect(() => {
		if (selected && selected.controller.controller_id !== selectedControllerId)
			setSelectedControllerId(selected.controller.controller_id);
	}, [selected, selectedControllerId]);
	const groupFor = useCallback((field: string) => {
		const now = performance.now();
		if (
			!gesture.current ||
			gesture.current.field !== field ||
			now - gesture.current.at > 250
		)
			gesture.current = { field, id: crypto.randomUUID(), at: now };
		else gesture.current.at = now;
		return gesture.current.id;
	}, []);
	const update = useCallback(
		async (field: ControllerValueField, value: number, requestId?: string) => {
			if (!actions || !selected) return;
			const controllerId = selected.controller.controller_id;
			const writeKey = `${controllerId}:${field}`;
			const writeId = crypto.randomUUID();
			latestWrites.current.set(writeKey, writeId);
			setOverrides((current) => ({
				...current,
				[controllerId]: { ...current[controllerId], [field]: value },
			}));
			setError(null);
			try {
				await actions.dynamics.setControllerValueLive(
					controllerId,
					field,
					value,
					groupFor(field),
					requestId,
				);
				await refresh();
			} catch (cause) {
				setError(formatErrorDetails(cause));
			} finally {
				if (latestWrites.current.get(writeKey) === writeId) {
					latestWrites.current.delete(writeKey);
					setOverrides((current) =>
						clearControllerOverride(current, controllerId, field),
					);
				}
			}
		},
		[actions, groupFor, refresh, selected, setError],
	);
	const off = useCallback(async (requestId?: string) => {
		if (!actions || !selected) return;
		setError(null);
		try {
			await actions.dynamics.offLive(
				selected.controller.controller_id,
				undefined,
				requestId,
			);
			await refresh();
		} catch (cause) {
			setError(formatErrorDetails(cause));
		}
	}, [actions, refresh, selected, setError]);
	const cycleChoice = useCallback(
		(delta: number) => {
			if (!selected || choices.length < 2) return;
			const current = choices.findIndex(
				(choice) =>
					choice.controller.controller_id === selected.controller.controller_id,
			);
			const next =
				(current + Math.sign(delta) + choices.length) % choices.length;
			setSelectedControllerId(choices[next].controller.controller_id);
			resetLane();
		},
		[choices, resetLane, selected],
	);
	return { selected, setSelectedControllerId, update, off, cycleChoice };
}

function useHardwareControllerActions(
	hardwareConnected: boolean,
	selected: DynamicControllerChoice | null,
	cycleChoice: (delta: number) => void,
	cycleLane: (delta: number) => void,
	update: (
		field: ControllerValueField,
		value: number,
		requestId?: string,
	) => Promise<void>,
	off: (requestId?: string) => Promise<void>,
) {
	useEffect(() => {
		if (!hardwareConnected || !selected) return;
		const handle = (event: Event) => {
			const detail = (
				event as CustomEvent<{
					control: string;
					value?: string;
					request_id?: string;
				}>
			).detail;
			const slot = Number(detail.control.split("/")[1]);
			const direction =
				detail.value === "up" || detail.value === "right"
					? 1
					: detail.value === "down" || detail.value === "left"
						? -1
						: 0;
			if (slot === 1 && direction) cycleChoice(direction);
			else if (slot === 2 && direction) cycleLane(direction);
			else if (slot === 3 && direction)
				void update(
					"size",
					clamp(selected.controller.size + direction * 0.01, 0, 2),
					detail.request_id,
				);
			else if (slot === 4 && direction)
				void update(
					"speed",
					clamp(
						selected.controller.speed_multiplier + direction * 0.05,
						0.0625,
						16,
					),
					detail.request_id,
				);
			else if (slot === 5 && direction)
				void update(
					"phase",
					clamp(
						selected.controller.phase_offset_degrees + direction * 5,
						-360,
						360,
					),
					detail.request_id,
				);
			else if (slot === 6 && detail.value === "press")
				void off(detail.request_id);
		};
		window.addEventListener("light:encoder-action", handle);
		return () => window.removeEventListener("light:encoder-action", handle);
	}, [hardwareConnected, cycleChoice, cycleLane, off, selected, update]);
}

function applyControllerOverride(
	choice: DynamicControllerChoice,
	override: ControllerValueOverride | undefined,
): DynamicControllerChoice {
	if (!override) return choice;
	return {
		...choice,
		controller: {
			...choice.controller,
			size: override.size ?? choice.controller.size,
			speed_multiplier: override.speed ?? choice.controller.speed_multiplier,
			phase_offset_degrees:
				override.phase ?? choice.controller.phase_offset_degrees,
		},
	};
}

function clearControllerOverride(
	current: Record<string, ControllerValueOverride>,
	controllerId: string,
	field: ControllerValueField,
) {
	const override = current[controllerId];
	if (!override || override[field] === undefined) return current;
	const nextOverride = { ...override };
	delete nextOverride[field];
	const next = { ...current };
	if (Object.keys(nextOverride).length === 0) delete next[controllerId];
	else next[controllerId] = nextOverride;
	return next;
}

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}
