import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useMemo, useRef, useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type {
	DynamicRuntimeSnapshotProjection,
	DynamicUpdateIntent,
	SpeedGroupId,
} from "../api/types";
import { DynamicEditorTaskTabs } from "../components/control/parameterControls/ParameterFamilyTabs";
import { DynamicDefinitionEncoderSurface } from "../components/control/parameterControls/ProgrammerDynamicsSurface";
import { AppShellView } from "../components/shell/AppShell";
import { LeftDock } from "../components/shell/LeftDock";
import { useDynamicEditorSession } from "../features/dynamics/DynamicEditorSessionContext";
import { applyDynamicUpdateIntent } from "../features/dynamics/dynamicUpdateIntent";
import type { ShowObject } from "../features/showObjects/contracts";
import {
	createDefaultDynamicDefinition,
	createDefaultDynamicLane,
	DynamicEditor,
	type DynamicEditorView,
} from "./DynamicsWindow";

const meta = {
	title: "ToskLight/Windows/Dynamics",
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: {
			description: {
				component:
					"Offline discussion surface composed from the production application shell, Dynamic editor, and six-encoder deck. Every change stays in Storybook memory.",
			},
		},
	},
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;
type DynamicObject = ShowObject<"dynamic">;

const attributes = [
	{ id: "intensity", label: "Intensity", family: "Intensity" },
	{ id: "color.red", label: "Red", family: "Color" },
	{ id: "color.green", label: "Green", family: "Color" },
	{ id: "color.blue", label: "Blue", family: "Color" },
	{ id: "pan", label: "Pan", family: "Position" },
	{ id: "tilt", label: "Tilt", family: "Position" },
	{ id: "zoom", label: "Zoom", family: "Focus" },
] as const;

function createStoryDynamic(): DynamicObject {
	const base = createDefaultDynamicDefinition(201, "intensity", {
		definition: "dynamic-story-201",
		lane: "dynamic-story-intensity",
	});
	const blue = createDefaultDynamicLane("color.blue", "dynamic-story-blue");
	const pan = createDefaultDynamicLane("pan", "dynamic-story-pan");
	return {
		kind: "dynamic",
		id: base.id,
		revision: 7,
		updated_at: "2026-07-27T12:00:00.000Z",
		body: {
			...base,
			revision: 7,
			name: "Ocean Sweep",
			target_binding: { type: "live_group", group_id: "group-front-wash" },
			lanes: [
				{
					...base.lanes[0],
					mode: "keyframes",
					width: 0.72,
					keyframes: {
						...base.lanes[0].keyframes,
						points: [
							{
								position: 0,
								source: { type: "value", value: 0.18 },
								interpolation: "ease_in_out",
							},
							{
								position: 0.38,
								source: { type: "value", value: 0.92 },
								interpolation: "ease_in_out",
							},
						],
					},
					max_min: {
						...base.lanes[0].max_min,
						minimum: { type: "value", value: 0.18 },
						maximum: { type: "value", value: 0.92 },
					},
				},
				{
					...blue,
					speed_multiplier: { numerator: 1, denominator: 2 },
					width: 0.58,
				},
				{
					...pan,
					mode: "middle_amplitude",
					speed_multiplier: { numerator: 1, denominator: 2 },
					middle_amplitude: {
						...pan.middle_amplitude,
						middle: { type: "current" },
						amplitude: 0.28,
						function: "cosinus",
					},
				},
			],
			phase: {
				...base.phase,
				ordering: { type: "grid_linear", angle_degrees: 32 },
				offset_degrees: 15,
				span_degrees: 360,
				block_size: 2,
				wings: true,
			},
			speed: {
				type: "speed_group",
				group: "A",
				beats_per_cycle: { numerator: 4, denominator: 1 },
			},
		},
	};
}

const runtime: DynamicRuntimeSnapshotProjection = {
	global_paused: false,
	definitions: [
		{
			dynamic_id: "dynamic-story-201",
			target_count: 12,
			compatible_target_count: 12,
			missing_target_count: 0,
			unpatched_target_count: 0,
			lane_count: 3,
			supported_address_count: 36,
			skipped_address_count: 0,
			warning: null,
		},
	],
	instances: [
		{
			instance_id: "dynamic-story-instance",
			dynamic_id: "dynamic-story-201",
			pool_number: 201,
			name: "Ocean Sweep",
			targets: Array.from({ length: 12 }, (_, index) => `fixture-${index + 1}`),
			pending: false,
			pending_until_millis: null,
			paused: false,
			speed_source: "Speed Group A",
			activation_boundary: "beat",
			effective_cycle_millis: 2000n,
			effective_bpm: 120,
			beat_phase: 0.38,
			phase_advancing: true,
			aliasing_warning: null,
			controllers: [
				{
					controller_id: "dynamic-story-controller",
					source: "Programmer",
					priority: 100,
					size: 1,
					speed_multiplier: 1,
					phase_offset_degrees: 0,
					paused: false,
					winning: true,
					releasing: false,
					activation_mix: 1,
				},
			],
		},
	],
};

function DynamicsProgrammerSurface({
	dynamic,
	onMutate,
	view,
	onView,
}: {
	dynamic: DynamicObject;
	onMutate(intent: DynamicUpdateIntent, mutationGroup?: string): Promise<void>;
	view: DynamicEditorView;
	onView(view: DynamicEditorView): void;
}) {
	const editor = useDynamicEditorSession();
	const [laneId, setLaneId] = useState(dynamic.body.lanes[0]?.id ?? "");
	const resolvedLaneId = editor.session?.primaryLaneId ?? laneId;
	const lane =
		dynamic.body.lanes.find((candidate) => candidate.id === resolvedLaneId) ??
		dynamic.body.lanes[0];
	return (
		<div className="parameter-controls">
			<DynamicEditorTaskTabs
				task={view}
				onTask={onView}
				page={editor.session?.encoderPage ?? 1}
				onPage={(encoderPage) => editor.update({ encoderPage })}
				dynamic={dynamic.body}
				laneId={lane?.id}
				onLane={(id) => {
					setLaneId(id);
					editor.update({ primaryLaneId: id, primaryKeyframeIndex: 0 });
				}}
			/>
			<div className="parameter-surfaces">
				<DynamicDefinitionEncoderSurface
					dynamic={dynamic.body}
					lane={lane ?? null}
					view={view}
					page={editor.session?.encoderPage ?? 1}
					keyframeIndex={editor.session?.primaryKeyframeIndex ?? 0}
					onKeyframeIndex={(primaryKeyframeIndex) =>
						editor.update({ primaryKeyframeIndex })
					}
					onLaneChange={async (change, mutationGroup) => {
						if (!lane) return;
						const next = change(lane);
						await onMutate(
							{ type: "replace_lane", lane_id: next.id, lane: next },
							mutationGroup,
						);
					}}
					onMutate={onMutate}
				/>
			</div>
		</div>
	);
}

function FullApplicationDynamicsMock({
	hardware,
	marketing = false,
}: {
	hardware: boolean;
	marketing?: boolean;
}) {
	const [dynamic, setDynamic] = useState(createStoryDynamic);
	const [message, setMessage] = useState(
		marketing
			? "12 fixtures · Speed Group A · 120 BPM"
			: "Offline discussion mock · changes are kept in Storybook memory only",
	);
	const [view, setView] = useState<DynamicEditorView>("curves");
	const [speedGroupBpms, setSpeedGroupBpms] = useState<
		Record<SpeedGroupId, number>
	>({ A: 120, B: 96, C: 128, D: 140, E: 72 });
	const speedGroupTapTimes = useRef<Partial<Record<SpeedGroupId, number[]>>>(
		{},
	);
	const tapSpeedGroup = (group: SpeedGroupId) => {
		const now = performance.now();
		const previous = speedGroupTapTimes.current[group]?.at(-1);
		const times =
			previous == null || now - previous > 2_000
				? [now]
				: [...(speedGroupTapTimes.current[group] ?? []).slice(-4), now];
		speedGroupTapTimes.current[group] = times;
		if (times.length < 2) {
			setMessage(`First tap captured for Speed Group ${group}`);
			return;
		}
		const intervals = times.slice(1).map((time, index) => time - times[index]);
		const average =
			intervals.reduce((sum, interval) => sum + interval, 0) / intervals.length;
		const bpm = Math.max(1, Math.min(999, Math.round(60_000 / average)));
		setSpeedGroupBpms((current) => ({ ...current, [group]: bpm }));
		setMessage(`Speed Group ${group} tapped to ${bpm} BPM`);
	};
	const storyRuntime = useMemo(
		() => ({
			...runtime,
			instances: runtime.instances.map((instance) => ({
				...instance,
				name: dynamic.body.name,
				pool_number: dynamic.body.pool_number,
				speed_source:
					dynamic.body.speed.type === "fixed"
						? "Fixed BPM"
						: `Speed Group ${dynamic.body.speed.group}`,
				effective_bpm:
					dynamic.body.speed.type === "fixed"
						? 60_000 / dynamic.body.speed.duration_millis
						: speedGroupBpms[dynamic.body.speed.group],
			})),
			definitions: runtime.definitions.map((status) => ({
				...status,
				lane_count: dynamic.body.lanes.length,
			})),
		}),
		[dynamic, speedGroupBpms],
	);
	const mutate = async (
		intent: DynamicUpdateIntent,
		_mutationGroup?: string,
	) => {
		setDynamic((current) => {
			const revision = current.revision + 1;
			const body = applyDynamicUpdateIntent(current.body, intent);
			return {
				...current,
				revision,
				updated_at: new Date().toISOString(),
				body: { ...body, revision },
			};
		});
		setMessage(`Applied ${intent.type.replaceAll("_", " ")} locally`);
	};
	return (
		<FullApplicationDynamicsView
			hardware={hardware}
			marketing={marketing}
			dynamic={dynamic}
			message={message}
			view={view}
			speedGroupBpms={speedGroupBpms}
			storyRuntime={storyRuntime}
			onView={setView}
			onMutate={mutate}
			onSpeedGroupTap={tapSpeedGroup}
			onMessage={setMessage}
			onDynamic={setDynamic}
		/>
	);
}

function FullApplicationDynamicsView({
	hardware,
	marketing,
	dynamic,
	message,
	view,
	speedGroupBpms,
	storyRuntime,
	onView,
	onMutate,
	onSpeedGroupTap,
	onMessage,
	onDynamic,
}: {
	hardware: boolean;
	marketing: boolean;
	dynamic: DynamicObject;
	message: string;
	view: DynamicEditorView;
	speedGroupBpms: Record<SpeedGroupId, number>;
	storyRuntime: DynamicRuntimeSnapshotProjection;
	onView(view: DynamicEditorView): void;
	onMutate(intent: DynamicUpdateIntent, mutationGroup?: string): Promise<void>;
	onSpeedGroupTap(group: SpeedGroupId): void;
	onMessage(message: string): void;
	onDynamic(update: (current: DynamicObject) => DynamicObject): void;
}) {
	return (
		<ApplicationStateHarness
			actions={[
				{ type: "SET_DOCK_MODE", mode: "builtins" },
				{ type: "OPEN_BUILTIN", kind: "dynamics" },
				...(hardware
					? ([{ type: "SET_MIDI_PROFILE", value: true }] as const)
					: []),
			]}
		>
			<AppShellView
				dock={
					<LeftDock
						presentation={{
							showIdentity: marketing ? "Demo Show" : "Dynamics UI Review",
							showIndicator: {
								label: marketing ? "Demo show" : "Offline mock",
								detail: marketing
									? "Deterministic marketing presentation."
									: "No headless server is connected",
								className: marketing
									? "show-status-connected"
									: "show-status-warning",
								connected: marketing,
							},
							clock: <span>12:00</span>,
						}}
					/>
				}
				workspace={
					<GridDesktop id="dynamics-review" name="Dynamics Review">
						<PaneView
							maximized
							showHeader={false}
							pane={{
								id: "dynamics-editor",
								title: "Dynamics",
								type: "dynamics",
								x: 1,
								y: 1,
								width: 24,
								height: 18,
							}}
							info={{
								primary: marketing
									? "Dynamic 201 · Ocean Sweep"
									: "Offline discussion surface",
								secondary: message,
							}}
						>
							<div className="dynamic-full-discussion-editor">
								<DynamicEditor
									dynamic={dynamic}
									compact={false}
									busy={false}
									error={null}
									attributes={attributes}
									presets={[]}
									runtime={storyRuntime}
									speedGroupBpms={speedGroupBpms}
									selection={runtime.instances[0].targets}
									selectedGroupId="group-front-wash"
									view={view}
									onViewChange={onView}
									onBack={() => onMessage("Back to Pool requested")}
									onMutate={async (_object, intent, mutationGroup) =>
										onMutate(intent, mutationGroup)
									}
									onSpeedGroupTap={onSpeedGroupTap}
									onDelete={() =>
										onMessage("Delete requested · ignored by offline mock")
									}
									onMove={(poolNumber) => {
										onDynamic((current) => ({
											...current,
											revision: current.revision + 1,
											body: {
												...current.body,
												pool_number: poolNumber,
												revision: current.revision + 1,
											},
										}));
										onMessage(`Moved locally to Dynamic ${poolNumber}`);
									}}
									onCopy={(poolNumber) =>
										onMessage(`Copy requested for Dynamic ${poolNumber}`)
									}
								/>
							</div>
						</PaneView>
					</GridDesktop>
				}
				control={
					<CommandSectionFixture
						inheritAppState
						initialMode="programmer"
						hardware={hardware}
						programmer={
							<DynamicsProgrammerSurface
								dynamic={dynamic}
								onMutate={onMutate}
								view={view}
								onView={onView}
							/>
						}
					/>
				}
			/>
		</ApplicationStateHarness>
	);
}

export function MarketingDynamicsApplication() {
	return <FullApplicationDynamicsMock hardware={false} marketing />;
}

export const FullApplicationDiscussion: Story = {
	render: (_args, context) => (
		<FullApplicationDynamicsMock
			hardware={context.globals.mode === "hardware"}
		/>
	),
};
