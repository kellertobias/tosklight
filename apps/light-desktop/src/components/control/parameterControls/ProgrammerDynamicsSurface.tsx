import { Button, SelectField } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	DynamicDefinitionProjection,
	DynamicRuntimeControllerProjection,
	DynamicRuntimeInstanceProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicUpdateIntent,
} from "../../../api/generated/light-wire";
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
import { HardwareEncoderDisplay } from "../HardwareEncoderDisplay";
import type { ParameterController } from "./useParameterController";

interface DynamicControllerChoice {
	instance: DynamicRuntimeInstanceProjection;
	controller: DynamicRuntimeControllerProjection;
	definition: DynamicDefinitionProjection | null;
}

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
	const [runtime, setRuntime] = useState(EMPTY_RUNTIME);
	const [selectedControllerId, setSelectedControllerId] = useState<
		string | null
	>(null);
	const [selectedLaneId, setSelectedLaneId] = useState<string | null>(null);
	const [view, setView] = useState<"instance" | DynamicEditorView>("instance");
	const [error, setError] = useState<string | null>(null);
	const [controllerOverrides, setControllerOverrides] = useState<
		Record<string, ControllerValueOverride>
	>({});
	const latestControllerWrites = useRef(new Map<string, string>());
	const gesture = useRef<{ field: string; id: string; at: number } | null>(
		null,
	);
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
			setError(cause instanceof Error ? cause.message : String(cause)),
		);
		if (!controller.active) return;
		const timer = window.setInterval(
			() => void refresh().catch(() => undefined),
			500,
		);
		return () => window.clearInterval(timer);
	}, [controller.active, refresh]);
	const choices = useMemo(
		() =>
			dynamicChoices(
				runtime,
				definitions.map((definition) => definition.body),
				controller.selectedFixtureIds,
				programmer?.dynamicValues ?? [],
			),
		[runtime, definitions, controller.selectedFixtureIds, programmer],
	);
	const selectedAuthoritative =
		choices.find(
			(choice) => choice.controller.controller_id === selectedControllerId,
		) ??
		choices[0] ??
		null;
	const selected = selectedAuthoritative
		? applyControllerOverride(
				selectedAuthoritative,
				controllerOverrides[selectedAuthoritative.controller.controller_id],
			)
		: null;
	useEffect(() => {
		if (selected && selected.controller.controller_id !== selectedControllerId)
			setSelectedControllerId(selected.controller.controller_id);
	}, [selected, selectedControllerId]);
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
	}, [selectedLane, selectedLaneId]);
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
		async (field: ControllerValueField, value: number) => {
			if (!actions || !selected) return;
			const controllerId = selected.controller.controller_id;
			const writeKey = `${controllerId}:${field}`;
			const writeId = crypto.randomUUID();
			latestControllerWrites.current.set(writeKey, writeId);
			setControllerOverrides((current) => ({
				...current,
				[controllerId]: {
					...current[controllerId],
					[field]: value,
				},
			}));
			setError(null);
			try {
				await actions.dynamics.setControllerValueLive(
					controllerId,
					field,
					value,
					groupFor(field),
				);
				await refresh();
			} catch (cause) {
				setError(cause instanceof Error ? cause.message : String(cause));
			} finally {
				if (latestControllerWrites.current.get(writeKey) === writeId) {
					latestControllerWrites.current.delete(writeKey);
					setControllerOverrides((current) =>
						clearControllerOverride(current, controllerId, field),
					);
				}
			}
		},
		[actions, groupFor, refresh, selected],
	);
	const off = useCallback(async () => {
		if (!actions || !selected) return;
		setError(null);
		try {
			await actions.dynamics.offLive(selected.controller.controller_id);
			await refresh();
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		}
	}, [actions, refresh, selected]);
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
			setSelectedLaneId(null);
		},
		[choices, selected],
	);
	const cycleLane = useCallback(
		(delta: number) => {
			if (!selectedLane || lanes.length < 2) return;
			const current = lanes.indexOf(selectedLane);
			const next = (current + Math.sign(delta) + lanes.length) % lanes.length;
			setSelectedLaneId(lanes[next].id);
		},
		[lanes, selectedLane],
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
				setError(cause instanceof Error ? cause.message : String(cause));
			}
		},
		[mutationWriter, selectedObject, showId],
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

	useEffect(() => {
		if (!controller.hardwareConnected || !selected) return;
		const handle = (event: Event) => {
			const detail = (event as CustomEvent<{ control: string; value?: string }>)
				.detail;
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
				);
			else if (slot === 4 && direction)
				void update(
					"speed",
					clamp(
						selected.controller.speed_multiplier + direction * 0.05,
						0.0625,
						16,
					),
				);
			else if (slot === 5 && direction)
				void update(
					"phase",
					clamp(
						selected.controller.phase_offset_degrees + direction * 5,
						-360,
						360,
					),
				);
			else if (slot === 6 && detail.value === "press") void off();
		};
		window.addEventListener("light:encoder-action", handle);
		return () => window.removeEventListener("light:encoder-action", handle);
	}, [
		controller.hardwareConnected,
		cycleChoice,
		cycleLane,
		off,
		selected,
		update,
	]);

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
	const dynamicLabel = selected.definition
		? `Dynamic ${selected.definition.pool_number} · ${selected.definition.name}`
		: `Dynamic ${selected.instance.pool_number} · ${selected.instance.name}`;
	const status = [
		selected.controller.source,
		selected.controller.winning ? "Winning" : "Hidden",
		selected.controller.paused || selected.instance.paused
			? "Paused"
			: "Active",
		selected.instance.pending ? "Pending" : null,
	]
		.filter(Boolean)
		.join(" · ");
	const toolbar = (
		<div className="programmer-dynamics-toolbar">
			<SelectField
				ariaLabel="Dynamic instance"
				value={selected.controller.controller_id}
				options={choices.map((choice) => ({
					value: choice.controller.controller_id,
					label: `Dynamic ${choice.instance.pool_number} · ${choice.instance.name} · ${choice.controller.source}`,
				}))}
				onChange={(controllerId) => {
					setSelectedControllerId(controllerId);
					setSelectedLaneId(null);
				}}
			/>
			<SelectField
				ariaLabel="Dynamic lane"
				value={selectedLane?.id ?? ""}
				options={lanes.map((lane) => ({
					value: lane.id,
					label: lane.attribute,
				}))}
				onChange={setSelectedLaneId}
			/>
			{(["instance", "curves", "phase", "speed"] as const).map((candidate) => (
				<Button
					key={candidate}
					className={view === candidate ? "active" : ""}
					onClick={() => setView(candidate)}
				>
					{candidate === "instance"
						? "Instance"
						: candidate === "phase"
							? "Phase"
							: candidate === "curves"
								? "Lanes"
								: candidate[0].toUpperCase() + candidate.slice(1)}
				</Button>
			))}
		</div>
	);

	if (view !== "instance" && selectedObject)
		return (
			<>
				{toolbar}
				<div className="programmer-dynamics-editor-deck">
					<DynamicEncoderDeck
						view={view}
						page={editor.session?.encoderPage ?? 1}
						lane={selectedLane ?? undefined}
						dynamic={selectedObject.body}
						presets={presets}
						keyframeIndex={editor.session?.primaryKeyframeIndex ?? 0}
						onKeyframeIndex={(primaryKeyframeIndex) =>
							editor.update({ primaryKeyframeIndex })
						}
						onLaneChange={changeLane}
						onMutate={mutateDefinition}
					/>
				</div>
			</>
		);

	if (controller.hardwareConnected)
		return (
			<>
				{toolbar}
				<HardwareEncoderDisplay
					slot={1}
					target={{ label: dynamicLabel, value: status }}
				/>
				<HardwareEncoderDisplay
					slot={2}
					target={{
						label: selectedLane?.attribute ?? "Lane",
						value: `${lanes.length} lane${lanes.length === 1 ? "" : "s"}`,
					}}
				/>
				<HardwareEncoderDisplay
					slot={3}
					target={{
						label: "Instance Size",
						value: `${Math.round(selected.controller.size * 100)}%`,
					}}
				/>
				<HardwareEncoderDisplay
					slot={4}
					target={{
						label: "Instance Speed",
						value: `${selected.controller.speed_multiplier.toFixed(2)}×`,
					}}
				/>
				<HardwareEncoderDisplay
					slot={5}
					target={{
						label: "Instance Phase",
						value: `${selected.controller.phase_offset_degrees.toFixed(0)}°`,
					}}
				/>
				<HardwareEncoderDisplay
					slot={6}
					activateOnHardwarePress
					target={{ label: "Dynamic Off", value: "Press" }}
					onHardwarePress={() => {
						void off();
						return true;
					}}
				/>
			</>
		);

	return (
		<>
			{toolbar}
			<TouchEncoder
				label={`Enc 1 · ${dynamicLabel}`}
				slot={1}
				attributeLabel={dynamicLabel}
				value={Math.max(
					0,
					choices.findIndex(
						(choice) =>
							choice.controller.controller_id ===
							selected.controller.controller_id,
					),
				)}
				display={status}
				indexed
				onStep={(delta) => cycleChoice(delta)}
				onSet={(value) => {
					const choice = choices[Math.round(value)];
					if (choice) setSelectedControllerId(choice.controller.controller_id);
				}}
			/>
			<TouchEncoder
				label={`Enc 2 · ${selectedLane?.attribute ?? "Lane"}`}
				slot={2}
				attributeLabel={selectedLane?.attribute ?? "Lane"}
				value={selectedLane ? Math.max(0, lanes.indexOf(selectedLane)) : 0}
				display={`${lanes.length} lane${lanes.length === 1 ? "" : "s"}`}
				indexed
				disabled={!selectedLane}
				onStep={(delta) => cycleLane(delta)}
				onSet={(value) => {
					const lane = lanes[Math.round(value)];
					if (lane) setSelectedLaneId(lane.id);
				}}
			/>
			<TouchEncoder
				label="Enc 3 · Instance Size"
				slot={3}
				attributeLabel="Instance Size"
				value={selected.controller.size}
				display={`${Math.round(selected.controller.size * 100)}%`}
				mode="Dynamics"
				onStep={(delta) =>
					void update("size", clamp(selected.controller.size + delta, 0, 2))
				}
				onSet={(value) => void update("size", clamp(value, 0, 2))}
			/>
			<TouchEncoder
				label="Enc 4 · Instance Speed"
				slot={4}
				attributeLabel="Instance Speed"
				value={selected.controller.speed_multiplier}
				display={`${selected.controller.speed_multiplier.toFixed(2)}×`}
				mode="Dynamics"
				onStep={(delta) =>
					void update(
						"speed",
						clamp(selected.controller.speed_multiplier + delta, 0.0625, 16),
					)
				}
				onSet={(value) => void update("speed", clamp(value, 0.0625, 16))}
			/>
			<TouchEncoder
				label="Enc 5 · Instance Phase"
				slot={5}
				attributeLabel="Instance Phase"
				value={selected.controller.phase_offset_degrees}
				display={`${selected.controller.phase_offset_degrees.toFixed(0)}°`}
				mode="Dynamics"
				onStep={(delta) =>
					void update(
						"phase",
						clamp(selected.controller.phase_offset_degrees + delta, -360, 360),
					)
				}
				onSet={(value) => void update("phase", clamp(value, -360, 360))}
			/>
			<div className="parameter-placeholder programmer-dynamics-off">
				<b>Dynamic Off</b>
				<small>{error ?? "Stops only this exact instance."}</small>
				<Button onClick={() => void off()}>Off</Button>
			</div>
		</>
	);
}

function dynamicChoices(
	runtime: DynamicRuntimeSnapshotProjection,
	definitions: readonly DynamicDefinitionProjection[],
	selectedFixtureIds: readonly string[],
	stagedValues: readonly ProgrammerDynamicValue[],
): DynamicControllerChoice[] {
	const selected = new Set(selectedFixtureIds);
	const running = runtime.instances
		.filter(
			(instance) =>
				selected.size === 0 ||
				instance.targets.some((target) => selected.has(target)),
		)
		.flatMap((instance) =>
			instance.controllers.map((controller) => ({
				instance,
				controller,
				definition:
					definitions.find(
						(definition) => definition.id === instance.dynamic_id,
					) ?? null,
			})),
		);
	const runningControllerIds = new Set(
		running.map((choice) => choice.controller.controller_id),
	);
	const stagedByInstance = new Map<
		string,
		{
			values: ProgrammerDynamicValue[];
			on: Extract<ProgrammerDynamicValue["value"], { type: "dynamic_on" }>;
		}
	>();
	for (const value of stagedValues) {
		if (value.value.type !== "dynamic_on") continue;
		if (selected.size > 0 && !selected.has(value.fixtureId)) continue;
		if (runningControllerIds.has(value.value.instance_link)) continue;
		const group = stagedByInstance.get(value.value.instance_link);
		if (group) group.values.push(value);
		else
			stagedByInstance.set(value.value.instance_link, {
				values: [value],
				on: value.value,
			});
	}
	const staged = [...stagedByInstance.entries()].map(
		([instanceLink, { values, on }]) => {
			const definition =
				definitions.find(
					(candidate) =>
						candidate.id ===
						(on.dynamic.dynamic_id ?? on.dynamic.embedded_fallback.id),
				) ?? on.dynamic.embedded_fallback;
			const targets = [...new Set(values.map((value) => value.fixtureId))];
			const controllerProjection: DynamicRuntimeControllerProjection = {
				controller_id: instanceLink,
				source: "Programmer staged",
				priority: 0,
				size: on.overrides.size,
				speed_multiplier:
					on.overrides.speed_multiplier.numerator /
					on.overrides.speed_multiplier.denominator,
				phase_offset_degrees: on.overrides.phase_offset_degrees,
				paused: false,
				winning: false,
				releasing: false,
				activation_mix: 0,
			};
			const instanceProjection: DynamicRuntimeInstanceProjection = {
				instance_id: instanceLink,
				dynamic_id: definition.id,
				pool_number: on.dynamic.last_known_pool_number,
				name: definition.name,
				targets,
				pending: true,
				pending_until_millis: null,
				paused: false,
				speed_source: "Staged",
				activation_boundary: "beat",
				effective_cycle_millis: 0n,
				effective_bpm: null,
				beat_phase: null,
				phase_advancing: false,
				aliasing_warning: null,
				controllers: [controllerProjection],
			};
			return {
				instance: instanceProjection,
				controller: controllerProjection,
				definition,
			};
		},
	);
	return [...running, ...staged].sort(
		(left, right) =>
			Number(right.controller.winning) - Number(left.controller.winning) ||
			left.instance.pool_number - right.instance.pool_number ||
			left.controller.source.localeCompare(right.controller.source),
	);
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
