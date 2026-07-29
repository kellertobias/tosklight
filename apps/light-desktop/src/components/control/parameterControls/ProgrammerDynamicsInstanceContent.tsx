import { Button, SelectField } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import type {
	DynamicDefinitionProjection,
	DynamicRuntimeControllerProjection,
	DynamicRuntimeInstanceProjection,
	DynamicUpdateIntent,
} from "../../../api/types";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import type { ShowObject } from "../../../features/showObjects/contracts";
import {
	type DynamicEditorView,
	DynamicEncoderDeck,
} from "../../../windows/DynamicsWindow";
import { HardwareEncoderDisplay } from "../HardwareEncoderDisplay";
import type { ParameterController } from "./useParameterController";

export interface DynamicControllerChoice {
	instance: DynamicRuntimeInstanceProjection;
	controller: DynamicRuntimeControllerProjection;
	definition: DynamicDefinitionProjection | null;
}

interface InstanceContentProps {
	controller: ParameterController;
	editor: ReturnType<typeof useDynamicEditorSession>;
	choices: DynamicControllerChoice[];
	selected: DynamicControllerChoice;
	selectedLane: DynamicDefinitionProjection["lanes"][number] | null;
	lanes: DynamicDefinitionProjection["lanes"];
	selectedObject: ShowObject<"dynamic"> | undefined;
	presets: readonly ShowObject<"preset">[];
	view: "instance" | DynamicEditorView;
	error: string | null;
	onView(view: "instance" | DynamicEditorView): void;
	onController(controllerId: string): void;
	onLane(laneId: string): void;
	onCycleChoice(delta: number): void;
	onCycleLane(delta: number): void;
	onUpdate(field: "size" | "speed" | "phase", value: number): Promise<void>;
	onOff(): Promise<void>;
	onLaneChange(
		change: (
			lane: DynamicDefinitionProjection["lanes"][number],
		) => DynamicDefinitionProjection["lanes"][number],
		mutationGroup?: string,
	): Promise<void>;
	onMutate(intent: DynamicUpdateIntent, mutationGroup?: string): Promise<void>;
}

export function ProgrammerDynamicsInstanceContent(props: InstanceContentProps) {
	const { selected, selectedLane, lanes, view, selectedObject } = props;
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
	return (
		<>
			<DynamicsToolbar {...props} />
			{view !== "instance" && selectedObject ? (
				<DefinitionDeck {...props} selectedObject={selectedObject} />
			) : props.controller.hardwareConnected ? (
				<HardwareInstanceControls
					{...props}
					dynamicLabel={dynamicLabel}
					status={status}
				/>
			) : (
				<TouchInstanceControls
					{...props}
					dynamicLabel={dynamicLabel}
					status={status}
				/>
			)}
		</>
	);
}

function DynamicsToolbar({
	choices,
	selected,
	selectedLane,
	lanes,
	view,
	onView,
	onController,
	onLane,
}: InstanceContentProps) {
	return (
		<div className="programmer-dynamics-toolbar">
			<SelectField
				ariaLabel="Dynamic instance"
				value={selected.controller.controller_id}
				options={choices.map((choice) => ({
					value: choice.controller.controller_id,
					label: `Dynamic ${choice.instance.pool_number} · ${choice.instance.name} · ${choice.controller.source}`,
				}))}
				onChange={onController}
			/>
			<SelectField
				ariaLabel="Dynamic lane"
				value={selectedLane?.id ?? ""}
				options={lanes.map((lane) => ({
					value: lane.id,
					label: lane.attribute,
				}))}
				onChange={onLane}
			/>
			{(["instance", "curves", "phase", "speed"] as const).map((candidate) => (
				<Button
					key={candidate}
					className={view === candidate ? "active" : ""}
					onClick={() => onView(candidate)}
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
}

function DefinitionDeck({
	editor,
	selectedLane,
	selectedObject,
	presets,
	view,
	onLaneChange,
	onMutate,
}: InstanceContentProps & { selectedObject: ShowObject<"dynamic"> }) {
	return (
		<div className="programmer-dynamics-editor-deck">
			<DynamicEncoderDeck
				view={view === "instance" ? "curves" : view}
				page={editor.session?.encoderPage ?? 1}
				lane={selectedLane ?? undefined}
				dynamic={selectedObject.body}
				presets={presets}
				keyframeIndex={editor.session?.primaryKeyframeIndex ?? 0}
				onKeyframeIndex={(primaryKeyframeIndex) =>
					editor.update({ primaryKeyframeIndex })
				}
				onLaneChange={onLaneChange}
				onMutate={onMutate}
			/>
		</div>
	);
}

function HardwareInstanceControls({
	selected,
	selectedLane,
	lanes,
	dynamicLabel,
	status,
	onOff,
}: InstanceContentProps & { dynamicLabel: string; status: string }) {
	return (
		<>
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
					void onOff();
					return true;
				}}
			/>
		</>
	);
}

function TouchInstanceControls({
	choices,
	selected,
	selectedLane,
	lanes,
	dynamicLabel,
	status,
	error,
	onController,
	onCycleChoice,
	onLane,
	onCycleLane,
	onUpdate,
	onOff,
}: InstanceContentProps & { dynamicLabel: string; status: string }) {
	return (
		<>
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
				onStep={onCycleChoice}
				onSet={(value) => {
					const choice = choices[Math.round(value)];
					if (choice) onController(choice.controller.controller_id);
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
				onStep={onCycleLane}
				onSet={(value) => {
					const lane = lanes[Math.round(value)];
					if (lane) onLane(lane.id);
				}}
			/>
			<ControllerValueEncoders selected={selected} onUpdate={onUpdate} />
			<div className="parameter-placeholder programmer-dynamics-off">
				<b>Dynamic Off</b>
				<small>{error ?? "Stops only this exact instance."}</small>
				<Button onClick={() => void onOff()}>Off</Button>
			</div>
		</>
	);
}

function ControllerValueEncoders({
	selected,
	onUpdate,
}: Pick<InstanceContentProps, "selected" | "onUpdate">) {
	return (
		<>
			<TouchEncoder
				label="Enc 3 · Instance Size"
				slot={3}
				attributeLabel="Instance Size"
				value={selected.controller.size}
				display={`${Math.round(selected.controller.size * 100)}%`}
				mode="Dynamics"
				onStep={(delta) =>
					void onUpdate("size", clamp(selected.controller.size + delta, 0, 2))
				}
				onSet={(value) => void onUpdate("size", clamp(value, 0, 2))}
			/>
			<TouchEncoder
				label="Enc 4 · Instance Speed"
				slot={4}
				attributeLabel="Instance Speed"
				value={selected.controller.speed_multiplier}
				display={`${selected.controller.speed_multiplier.toFixed(2)}×`}
				mode="Dynamics"
				onStep={(delta) =>
					void onUpdate(
						"speed",
						clamp(selected.controller.speed_multiplier + delta, 0.0625, 16),
					)
				}
				onSet={(value) => void onUpdate("speed", clamp(value, 0.0625, 16))}
			/>
			<TouchEncoder
				label="Enc 5 · Instance Phase"
				slot={5}
				attributeLabel="Instance Phase"
				value={selected.controller.phase_offset_degrees}
				display={`${selected.controller.phase_offset_degrees.toFixed(0)}°`}
				mode="Dynamics"
				onStep={(delta) =>
					void onUpdate(
						"phase",
						clamp(selected.controller.phase_offset_degrees + delta, -360, 360),
					)
				}
				onSet={(value) => void onUpdate("phase", clamp(value, -360, 360))}
			/>
		</>
	);
}

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}
