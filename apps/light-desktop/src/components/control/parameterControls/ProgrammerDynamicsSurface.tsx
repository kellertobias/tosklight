import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApiRequestError } from "../../../api/ApiRequestError";
import type {
	DynamicDefinitionProjection,
	DynamicRuntimeControllerProjection,
	DynamicRuntimeInstanceProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicUpdateIntent,
} from "../../../api/generated/light-wire";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";
import { useDynamicsActions } from "../../../features/dynamics/DynamicsActionsContext";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import type { ProgrammerDynamicValue } from "../../../features/programmerValues/contracts";
import { useProgrammerValuesView } from "../../../features/programmerValues/ProgrammerValuesView";
import { useDynamics } from "../../../features/showObjects/ShowObjectsState";
import {
	DynamicEncoderDeck,
	type DynamicEditorView,
} from "../../../windows/DynamicsWindow";
import { HardwareEncoderDisplay } from "../HardwareEncoderDisplay";
import type { ParameterController } from "./useParameterController";

interface DynamicControllerChoice {
	instance: DynamicRuntimeInstanceProjection;
	controller: DynamicRuntimeControllerProjection;
	definition: DynamicDefinitionProjection | null;
}

const EMPTY_RUNTIME: DynamicRuntimeSnapshotProjection = {
	global_paused: false,
	instances: [],
	definitions: [],
};

export function DynamicDefinitionEncoderSurface({
	dynamic,
	lane,
	view,
	onLaneChange,
	onMutate,
}: {
	dynamic: DynamicDefinitionProjection;
	lane: DynamicDefinitionProjection["lanes"][number] | null;
	view: DynamicEditorView;
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
				lane={lane ?? undefined}
				dynamic={dynamic}
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
	const programmer = useProgrammerValuesView(controller.active);
	const [runtime, setRuntime] = useState(EMPTY_RUNTIME);
	const [selectedControllerId, setSelectedControllerId] = useState<
		string | null
	>(null);
	const [selectedLaneId, setSelectedLaneId] = useState<string | null>(null);
	const [view, setView] = useState<"instance" | DynamicEditorView>("instance");
	const [error, setError] = useState<string | null>(null);
	const gesture = useRef<{ field: string; id: string; at: number } | null>(
		null,
	);
	const refresh = useCallback(async () => {
		if (!actions || !showId) return setRuntime(EMPTY_RUNTIME);
		setRuntime(await actions.dynamics.runtime(showId));
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
	const selected =
		choices.find(
			(choice) => choice.controller.controller_id === selectedControllerId,
		) ??
		choices[0] ??
		null;
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
		async (field: "size" | "speed" | "phase", value: number) => {
			if (!actions || !selected) return;
			setError(null);
			try {
				await actions.dynamics.setControllerValueLive(
					selected.controller.controller_id,
					field,
					value,
					groupFor(field),
				);
				await refresh();
			} catch (cause) {
				setError(cause instanceof Error ? cause.message : String(cause));
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
			const current = choices.indexOf(selected);
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
			if (!actions || !showId || !selectedObject) return;
			setError(null);
			try {
				await actions.showObjects.updateDynamic(
					showId,
					selectedObject.id,
					selectedObject.revision,
					intent,
					mutationGroup,
				);
			} catch (cause) {
				if (!(cause instanceof ApiRequestError) || cause.status !== 409) {
					setError(cause instanceof Error ? cause.message : String(cause));
					return;
				}
				const current =
					await actions.showObjects.object<DynamicDefinitionProjection>(
						showId,
						"dynamic",
						selectedObject.id,
					);
				await actions.showObjects.updateDynamic(
					showId,
					selectedObject.id,
					current.revision,
					intent,
					mutationGroup,
				);
			}
		},
		[actions, selectedObject, showId],
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
			<label>
				<span className="sr-only">Dynamic instance</span>
				<select
					aria-label="Dynamic instance"
					value={selected.controller.controller_id}
					onChange={(event) => {
						setSelectedControllerId(event.target.value);
						setSelectedLaneId(null);
					}}
				>
					{choices.map((choice) => (
						<option
							key={choice.controller.controller_id}
							value={choice.controller.controller_id}
						>
							Dynamic {choice.instance.pool_number} · {choice.instance.name} ·{" "}
							{choice.controller.source}
						</option>
					))}
				</select>
			</label>
			<label>
				<span className="sr-only">Dynamic lane</span>
				<select
					aria-label="Dynamic lane"
					value={selectedLane?.id ?? ""}
					onChange={(event) => setSelectedLaneId(event.target.value)}
				>
					{lanes.map((lane) => (
						<option key={lane.id} value={lane.id}>
							{lane.attribute}
						</option>
					))}
				</select>
			</label>
			{(["instance", "curves", "phase", "speed"] as const).map((candidate) => (
				<Button
					key={candidate}
					className={view === candidate ? "active" : ""}
					onClick={() => setView(candidate)}
				>
					{candidate === "instance"
						? "Instance"
						: candidate === "phase"
							? "Phase Spread"
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
						lane={selectedLane ?? undefined}
						dynamic={selectedObject.body}
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
				value={Math.max(0, choices.indexOf(selected))}
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
				value={Math.max(0, lanes.indexOf(selectedLane!))}
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

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}
