import {
	Button,
	FormLayout,
	MultiValueToggle,
	MultiValueToggleField,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useRef } from "react";
import type {
	DynamicDefinitionProjection,
	DynamicLaneProjection,
	DynamicPhaseOrderingProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicUpdateIntent,
	SpeedGroupId,
} from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";

import {
	clamp,
	isSpatialOrdering,
	orderingFor,
	rationalFromNumber,
	rationalValue,
} from "./DynamicsEditor";

type DynamicObject = ShowObject<"dynamic">;

export function PhaseView({
	dynamic,
	lane,
	running,
	selectionCount,
	onSelectLane,
	onTakeSelection,
	onClearSelection,
	onMutate,
}: {
	dynamic: DynamicObject;
	lane?: DynamicLaneProjection;
	running: boolean;
	selectionCount: number;
	onSelectLane(id: string): void;
	onTakeSelection(): void;
	onClearSelection(): void;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	const phaseMode = dynamic.body.phase_mode;
	const phase =
		phaseMode === "per_lane"
			? (lane?.phase ?? dynamic.body.phase)
			: dynamic.body.phase;
	const spatialOrdering = isSpatialOrdering(phase.ordering)
		? phase.ordering
		: null;
	const update = (patch: Partial<typeof phase>) => {
		const nextPhase = { ...phase, ...patch };
		if (phaseMode === "per_lane" && lane) {
			onMutate(dynamic, {
				type: "replace_lane",
				lane_id: lane.id,
				lane: { ...lane, phase: nextPhase },
			});
			return;
		}
		onMutate(dynamic, { type: "set_phase", phase: nextPhase });
	};
	return (
		<div className="dynamic-phase-view">
			<PhaseControls
				dynamic={dynamic}
				lane={lane}
				phase={phase}
				phaseMode={phaseMode}
				spatialOrdering={spatialOrdering}
				onSelectLane={onSelectLane}
				onUpdate={update}
			/>
			<PhaseFooter
				dynamic={dynamic}
				phase={phase}
				running={running}
				selectionCount={selectionCount}
				onTakeSelection={onTakeSelection}
				onClearSelection={onClearSelection}
				onUpdate={update}
			/>
		</div>
	);
}

type DynamicSpeed = DynamicDefinitionProjection["speed"];
type RuntimeInstance = DynamicRuntimeSnapshotProjection["instances"][number];

function SpeedTransport({
	dynamic,
	speed,
	beatPhase,
	fixedBpm,
	displayedBpm,
	runtimeState,
	primaryRuntime,
	onTapTempo,
	onMutate,
}: {
	dynamic: DynamicObject;
	speed: DynamicSpeed;
	beatPhase: number;
	fixedBpm: number | null;
	displayedBpm: number;
	runtimeState: string;
	primaryRuntime: RuntimeInstance | undefined;
	onTapTempo(): void;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	return (
		<section className="dynamic-speed-transport">
			<div
				className="dynamic-beat-grid"
				role="img"
				aria-label={`Speed transport beat grid, phase ${Math.round(clamp(beatPhase, 0, 1) * 100)}%`}
			>
				<i style={{ left: `${clamp(beatPhase, 0, 1) * 100}%` }} />
				<b className="dynamic-beat-phase">
					Phase {Math.round(clamp(beatPhase, 0, 1) * 100)}%
				</b>
				{Array.from({ length: 16 }, (_, index) => (
					<span key={index} className={index % 4 === 0 ? "bar-start" : ""}>
						{(index % 4) + 1}
					</span>
				))}
			</div>
			<MultiValueToggle
				ariaLabel="Speed source"
				value={speed.type}
				options={[
					{ value: "fixed", label: "Fixed BPM" },
					{ value: "speed_group", label: "Speed Group" },
				]}
				onChange={(type) =>
					onMutate(dynamic, {
						type: "set_speed",
						speed:
							type === "fixed"
								? { type: "fixed", duration_millis: 500 }
								: {
										type: "speed_group",
										group: "A",
										beats_per_cycle: { numerator: 4, denominator: 1 },
									},
					})
				}
			/>
			<SpeedSourceFields
				dynamic={dynamic}
				speed={speed}
				fixedBpm={fixedBpm}
				onMutate={onMutate}
			/>
			<Button
				className="dynamic-tap-tempo"
				aria-label={
					speed.type === "fixed"
						? `Tap fixed tempo, ${Math.round(displayedBpm)} BPM`
						: `Tap Speed Group ${speed.group} tempo, ${Math.round(displayedBpm)} BPM`
				}
				onClick={onTapTempo}
			>
				<strong className="speed-group-value">
					{Math.round(displayedBpm)} BPM
				</strong>
				<span className="speed-group-label">
					{speed.type === "fixed" ? "TAP" : `TAP GROUP ${speed.group}`}
				</span>
			</Button>
			<footer>
				<span>
					{runtimeState} · transport {Math.round(beatPhase * 100)}%
				</span>
				{primaryRuntime?.aliasing_warning && (
					<small>{primaryRuntime.aliasing_warning}</small>
				)}
			</footer>
		</section>
	);
}

function SpeedSourceFields({
	dynamic,
	speed,
	fixedBpm,
	onMutate,
}: {
	dynamic: DynamicObject;
	speed: DynamicSpeed;
	fixedBpm: number | null;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	if (speed.type === "fixed")
		return (
			<div className="dynamic-fixed-speed-fields">
				<NumberField
					label="Tempo"
					value={fixedBpm ?? 120}
					min={1}
					max={999}
					unit="BPM"
					onValueChange={(bpm) =>
						onMutate(dynamic, {
							type: "set_speed",
							speed: {
								type: "fixed",
								duration_millis: Math.max(
									1,
									Math.round(60_000 / Math.max(1, Number(bpm))),
								),
							},
						})
					}
				/>
			</div>
		);
	return (
		<div className="dynamic-speed-source-fields">
			<SelectField
				label="Speed Group"
				value={speed.group}
				options={["A", "B", "C", "D", "E"].map((group) => ({
					value: group,
					label: `Speed Group ${group}`,
				}))}
				onChange={(group) =>
					onMutate(dynamic, {
						type: "set_speed",
						speed: { ...speed, group },
					})
				}
			/>
			<NumberField
				label="Beats per cycle"
				value={speed.beats_per_cycle.numerator}
				min={1}
				onValueChange={(numerator) =>
					onMutate(dynamic, {
						type: "set_speed",
						speed: {
							...speed,
							beats_per_cycle: {
								numerator: Math.max(1, Math.round(Number(numerator))),
								denominator: 1,
							},
						},
					})
				}
			/>
		</div>
	);
}

function SpeedControls({
	dynamic,
	speed,
	instances,
	primaryRuntime,
	fixedBpm,
	onMutate,
}: {
	dynamic: DynamicObject;
	speed: DynamicSpeed;
	instances: RuntimeInstance[];
	primaryRuntime: RuntimeInstance | undefined;
	fixedBpm: number | null;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	return (
		<section className="dynamic-speed-controls">
			<FormLayout labelPlacement="top">
				<NumberField
					label="Multiplier"
					value={rationalValue(dynamic.body.overall_speed_multiplier)}
					min={0.0625}
					max={16}
					step={0.25}
					allowDecimal
					unit="×"
					onValueChange={(multiplier) =>
						onMutate(dynamic, {
							type: "set_overall_speed_multiplier",
							multiplier: rationalFromNumber(Number(multiplier)),
						})
					}
				/>
				<SelectField
					label="Run mode"
					description="Loop repeats continuously. One-shot stops after one complete cycle."
					value={dynamic.body.run_mode}
					options={[
						{ value: "loop", label: "Loop" },
						{ value: "one_shot", label: "One-shot" },
					]}
					onChange={(run_mode) =>
						onMutate(dynamic, { type: "set_run_mode", run_mode })
					}
				/>
				<ActivationFields dynamic={dynamic} speed={speed} onMutate={onMutate} />
			</FormLayout>
			<small>
				{instances.length} active{" "}
				{instances.length === 1 ? "instance" : "instances"} ·{" "}
				{primaryRuntime?.speed_source ??
					(speed.type === "speed_group"
						? `Speed Group ${speed.group}`
						: `${fixedBpm} BPM`)}
			</small>
		</section>
	);
}

function ActivationFields({
	dynamic,
	speed,
	onMutate,
}: {
	dynamic: DynamicObject;
	speed: DynamicSpeed;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
}) {
	return (
		<>
			<SelectField
				label="Activation"
				description="Chooses when a newly started Dynamic instance enters its first cycle."
				value={dynamic.body.default_activation}
				options={[
					{ value: "start_now", label: "Start now" },
					{
						value: "join_sync_now",
						label: "Join sync now",
						disabled: speed.type !== "speed_group",
					},
					{
						value: "next_boundary",
						label: "Next boundary",
						disabled: speed.type !== "speed_group",
					},
				]}
				onChange={(activation) =>
					onMutate(dynamic, { type: "set_activation", activation })
				}
			/>
			<SelectField
				label="Boundary"
				description="For Next boundary, wait for the next beat or the next four-beat bar."
				value={dynamic.body.activation_boundary}
				disabled={
					speed.type !== "speed_group" ||
					dynamic.body.default_activation !== "next_boundary"
				}
				options={[
					{ value: "beat", label: "Next beat" },
					{ value: "bar", label: "Next bar (4 beats)" },
				]}
				onChange={(boundary) =>
					onMutate(dynamic, {
						type: "set_activation_boundary",
						boundary,
					})
				}
			/>
		</>
	);
}

type DynamicPhase = DynamicDefinitionProjection["phase"];
type SpatialOrdering = Extract<
	DynamicPhaseOrderingProjection,
	{ type: "radial_out" | "radial_in" | "axial" }
>;

function PhaseControls({
	dynamic,
	lane,
	phase,
	phaseMode,
	spatialOrdering,
	onSelectLane,
	onUpdate,
}: {
	dynamic: DynamicObject;
	lane?: DynamicLaneProjection;
	phase: DynamicPhase;
	phaseMode: DynamicDefinitionProjection["phase_mode"];
	spatialOrdering: SpatialOrdering | null;
	onSelectLane(id: string): void;
	onUpdate(patch: Partial<DynamicPhase>): void;
}) {
	return (
		<section className="dynamic-phase-controls">
			{phaseMode === "per_lane" && lane && (
				<SelectField
					className="dynamic-phase-lane"
					label="Lane"
					ariaLabel="Dynamic phase lane"
					value={lane.id}
					options={dynamic.body.lanes.map((candidate, index) => ({
						value: candidate.id,
						label: `Lane ${index + 1} · ${candidate.attribute}`,
					}))}
					onChange={onSelectLane}
				/>
			)}
			<FormLayout labelPlacement="top">
				<div className="dynamic-phase-ordering-field">
					<MultiValueToggleField
						className="dynamic-phase-ordering"
						label="Ordering mode"
						value={
							phase.ordering.type === "radial_in"
								? "radial_out"
								: phase.ordering.type
						}
						options={[
							{ value: "selection", label: "Linear" },
							{ value: "grid_linear", label: "Grid" },
							{ value: "radial_out", label: "Radial" },
							{ value: "axial", label: "Radar" },
							{ value: "random_each_loop", label: "Random" },
						]}
						onChange={(ordering) =>
							onUpdate({
								ordering: orderingFor(ordering, phase.ordering),
							})
						}
					/>
				</div>
				<PhaseOrderingFields
					phase={phase}
					spatialOrdering={spatialOrdering}
					onUpdate={onUpdate}
				/>
				<SharedPhaseFields phase={phase} onUpdate={onUpdate} />
			</FormLayout>
		</section>
	);
}

function PhaseOrderingFields({
	phase,
	spatialOrdering,
	onUpdate,
}: {
	phase: DynamicPhase;
	spatialOrdering: SpatialOrdering | null;
	onUpdate(patch: Partial<DynamicPhase>): void;
}) {
	if (phase.ordering.type === "grid_linear")
		return (
			<div className="dynamic-phase-field dynamic-phase-direction-field">
				<NumberField
					label="Direction"
					value={phase.ordering.angle_degrees}
					allowDecimal
					unit="°"
					onValueChange={(angle_degrees) =>
						onUpdate({
							ordering: {
								type: "grid_linear",
								angle_degrees: Number(angle_degrees),
							},
						})
					}
				/>
			</div>
		);
	if (!spatialOrdering) return null;
	return (
		<>
			<div className="dynamic-phase-field dynamic-phase-center-x-field">
				<NumberField
					label="Center X"
					value={spatialOrdering.center_x}
					allowDecimal
					onValueChange={(center_x) =>
						onUpdate({
							ordering: {
								...spatialOrdering,
								center_x: Number(center_x),
							},
						})
					}
				/>
			</div>
			<div className="dynamic-phase-field dynamic-phase-center-z-field">
				<NumberField
					label="Center Z"
					value={spatialOrdering.center_z}
					allowDecimal
					onValueChange={(center_z) =>
						onUpdate({
							ordering: {
								...spatialOrdering,
								center_z: Number(center_z),
							},
						})
					}
				/>
			</div>
		</>
	);
}

function SharedPhaseFields({
	phase,
	onUpdate,
}: {
	phase: DynamicPhase;
	onUpdate(patch: Partial<DynamicPhase>): void;
}) {
	return (
		<div className="dynamic-phase-shared-fields">
			<div className="dynamic-phase-field dynamic-phase-offset-field">
				<NumberField
					label="Offset"
					value={phase.offset_degrees}
					allowDecimal
					unit="°"
					onValueChange={(value) => onUpdate({ offset_degrees: Number(value) })}
				/>
			</div>
			<div className="dynamic-phase-field dynamic-phase-span-field">
				<NumberField
					label="Span"
					value={phase.span_degrees}
					allowDecimal
					unit="°"
					onValueChange={(value) => onUpdate({ span_degrees: Number(value) })}
				/>
			</div>
			<div className="dynamic-phase-field dynamic-phase-blocks-field">
				<NumberField
					label="Blocks"
					value={phase.block_size}
					min={1}
					onValueChange={(value) =>
						onUpdate({ block_size: Math.max(1, Number(value)) })
					}
				/>
			</div>
			<div className="dynamic-phase-field dynamic-phase-repeats-field">
				<NumberField
					label="Repeats"
					value={phase.repeats}
					min={1}
					onValueChange={(value) =>
						onUpdate({ repeats: Math.max(1, Number(value)) })
					}
				/>
			</div>
			<div className="dynamic-phase-field dynamic-phase-wings-field">
				<SwitchField
					label="Wings"
					checked={phase.wings}
					offLabel="Off"
					onLabel="On"
					onChange={(event) => onUpdate({ wings: event.target.checked })}
				/>
			</div>
			<div className="dynamic-phase-field dynamic-phase-anchors-field">
				<TextField
					className="dynamic-phase-anchors"
					key={phase.anchors_degrees.join(",")}
					label="Explicit anchors"
					defaultValue={phase.anchors_degrees.join(" THRU ")}
					placeholder="Automatic"
					onBlur={(event) => {
						const text = event.target.value.trim();
						if (!text) return onUpdate({ anchors_degrees: [] });
						const anchors = text.split(/\s*(?:THRU|,)\s*/i).map(Number);
						if (anchors.every(Number.isFinite))
							onUpdate({ anchors_degrees: anchors });
					}}
				/>
			</div>
		</div>
	);
}

function PhaseFooter({
	dynamic,
	phase,
	running,
	selectionCount,
	onTakeSelection,
	onClearSelection,
	onUpdate,
}: {
	dynamic: DynamicObject;
	phase: DynamicPhase;
	running: boolean;
	selectionCount: number;
	onTakeSelection(): void;
	onClearSelection(): void;
	onUpdate(patch: Partial<DynamicPhase>): void;
}) {
	return (
		<footer className="dynamic-phase-footer">
			<div className="dynamic-phase-target-actions">
				<Button
					disabled={running || selectionCount === 0}
					onClick={onTakeSelection}
				>
					Take Selection
				</Button>
				<Button
					disabled={
						running || dynamic.body.target_binding.type === "targetless"
					}
					onClick={onClearSelection}
				>
					Clear Selection
				</Button>
			</div>
			<fieldset className="button-group" aria-label="Phase span presets">
				{[180, 360, 720].map((span) => (
					<Button
						key={span}
						active={phase.span_degrees === span}
						onClick={() => onUpdate({ span_degrees: span })}
					>
						{span}°
					</Button>
				))}
			</fieldset>
		</footer>
	);
}

export function SpeedView({
	dynamic,
	runtime,
	previewPhase,
	speedGroupBpms,
	onMutate,
	onSpeedGroupTap,
}: {
	dynamic: DynamicObject;
	runtime: DynamicRuntimeSnapshotProjection | null;
	previewPhase: number | null;
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>;
	onMutate(dynamic: DynamicObject, intent: DynamicUpdateIntent): void;
	onSpeedGroupTap?(group: SpeedGroupId): void;
}) {
	const speed = dynamic.body.speed;
	const instances =
		runtime?.instances.filter(
			(instance) => instance.dynamic_id === dynamic.id,
		) ?? [];
	const primaryRuntime =
		instances.find((instance) =>
			instance.controllers.some((controller) => controller.winning),
		) ?? instances[0];
	const runtimeState = primaryRuntime
		? primaryRuntime.pending
			? "Pending"
			: primaryRuntime.paused || !primaryRuntime.phase_advancing
				? "Paused"
				: "Running"
		: "Off";
	const fixedBpm =
		speed.type === "fixed"
			? Math.max(1, Math.round(60_000 / speed.duration_millis))
			: null;
	const beatPhase = previewPhase ?? primaryRuntime?.beat_phase ?? 0;
	const displayedBpm =
		speed.type === "fixed"
			? (fixedBpm ?? 120)
			: (speedGroupBpms?.[speed.group] ?? primaryRuntime?.effective_bpm ?? 120);
	const tapTimes = useRef<number[]>([]);
	const tapTempo = () => {
		if (speed.type === "speed_group") {
			onSpeedGroupTap?.(speed.group);
			return;
		}
		const now = performance.now();
		const previous = tapTimes.current.at(-1);
		if (previous == null || now - previous > 2_000) tapTimes.current = [now];
		else tapTimes.current = [...tapTimes.current.slice(-4), now];
		if (tapTimes.current.length < 2) return;
		const intervals = tapTimes.current
			.slice(1)
			.map((time, index) => time - tapTimes.current[index]);
		const average =
			intervals.reduce((sum, interval) => sum + interval, 0) / intervals.length;
		onMutate(dynamic, {
			type: "set_speed",
			speed: {
				type: "fixed",
				duration_millis: Math.max(1, Math.round(average)),
			},
		});
	};
	return (
		<div className="dynamic-speed-view">
			<SpeedTransport
				dynamic={dynamic}
				speed={speed}
				beatPhase={beatPhase}
				fixedBpm={fixedBpm}
				displayedBpm={displayedBpm}
				runtimeState={runtimeState}
				primaryRuntime={primaryRuntime}
				onTapTempo={tapTempo}
				onMutate={onMutate}
			/>
			<SpeedControls
				dynamic={dynamic}
				speed={speed}
				instances={instances}
				primaryRuntime={primaryRuntime}
				fixedBpm={fixedBpm}
				onMutate={onMutate}
			/>
		</div>
	);
}
