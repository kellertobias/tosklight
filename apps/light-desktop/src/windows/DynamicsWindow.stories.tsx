import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "@tosklight/ui";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useMemo, useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type {
	DynamicActivationPolicyProjection,
	DynamicDefinitionProjection,
	DynamicLaneProjection,
	DynamicPhaseDistributionProjection,
	DynamicRandomGroupProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicSpeedProjection,
	DynamicTargetBindingProjection,
	DynamicUpdateIntent,
} from "../api/generated/light-wire";
import type { ShowObject } from "../features/showObjects/contracts";
import { AppShellView } from "../components/shell/AppShell";
import { LeftDock } from "../components/shell/LeftDock";
import { DynamicEditorTaskTabs } from "../components/control/parameterControls/ParameterFamilyTabs";
import {
	createDefaultDynamicDefinition,
	createDefaultDynamicLane,
	DynamicEditor,
	DynamicEncoderDeck,
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
					width: 0.72,
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

function applyIntent(
	definition: DynamicDefinitionProjection,
	intent: DynamicUpdateIntent,
): DynamicDefinitionProjection {
	switch (intent.type) {
		case "set_name":
			return { ...definition, name: intent.name };
		case "set_color":
			return { ...definition, color: intent.color };
		case "set_icon":
			return { ...definition, icon: intent.icon };
		case "set_target_binding":
			return {
				...definition,
				target_binding: intent.target_binding as DynamicTargetBindingProjection,
			};
		case "add_lane": {
			const lanes = [...definition.lanes];
			const index = intent.index ?? lanes.length;
			lanes.splice(index, 0, intent.lane as DynamicLaneProjection);
			return { ...definition, lanes };
		}
		case "replace_lane":
			return {
				...definition,
				lanes: definition.lanes.map((lane) =>
					lane.id === intent.lane_id
						? (intent.lane as DynamicLaneProjection)
						: lane,
				),
			};
		case "delete_lane":
			return {
				...definition,
				lanes: definition.lanes.filter((lane) => lane.id !== intent.lane_id),
			};
		case "move_lane": {
			const lanes = [...definition.lanes];
			const current = lanes.findIndex((lane) => lane.id === intent.lane_id);
			if (current < 0) return definition;
			const [lane] = lanes.splice(current, 1);
			lanes.splice(Math.max(0, Math.min(intent.index, lanes.length)), 0, lane);
			return { ...definition, lanes };
		}
		case "set_phase":
			return {
				...definition,
				phase: intent.phase as DynamicPhaseDistributionProjection,
			};
		case "set_speed":
			return {
				...definition,
				speed: intent.speed as DynamicSpeedProjection,
			};
		case "set_overall_speed_multiplier":
			return { ...definition, overall_speed_multiplier: intent.multiplier };
		case "set_run_mode":
			return { ...definition, run_mode: intent.run_mode };
		case "set_activation":
			return {
				...definition,
				default_activation:
					intent.activation as DynamicActivationPolicyProjection,
			};
		case "set_activation_boundary":
			return { ...definition, activation_boundary: intent.boundary };
		case "add_random_group":
			return {
				...definition,
				random_groups: [
					...definition.random_groups,
					intent.group as DynamicRandomGroupProjection,
				],
			};
		case "replace_random_group":
			return {
				...definition,
				random_groups: definition.random_groups.map((group) =>
					group.id === intent.group_id
						? (intent.group as DynamicRandomGroupProjection)
						: group,
				),
			};
		case "delete_random_group":
			return {
				...definition,
				random_groups: definition.random_groups.filter(
					(group) => group.id !== intent.group_id,
				),
			};
	}
}

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
	const [laneId, setLaneId] = useState(dynamic.body.lanes[0]?.id ?? "");
	const lane =
		dynamic.body.lanes.find((candidate) => candidate.id === laneId) ??
		dynamic.body.lanes[0];
	return (
		<div className="parameter-controls">
			<DynamicEditorTaskTabs task={view} onTask={onView} />
			<div className="parameter-surfaces">
				<div className="programmer-dynamics-toolbar">
					<label>
						<span className="sr-only">Dynamic instance</span>
						<select aria-label="Dynamic instance" defaultValue={dynamic.id}>
							<option value={dynamic.id}>
								Dynamic {dynamic.body.pool_number} · {dynamic.body.name} ·
								Programmer
							</option>
						</select>
					</label>
					<label>
						<span className="sr-only">Dynamic lane</span>
						<select
							aria-label="Dynamic lane"
							value={lane?.id ?? ""}
							onChange={(event) => setLaneId(event.target.value)}
						>
							{dynamic.body.lanes.map((candidate) => (
								<option key={candidate.id} value={candidate.id}>
									{attributes.find((item) => item.id === candidate.attribute)
										?.label ?? candidate.attribute}
								</option>
							))}
						</select>
					</label>
				</div>
				<div className="programmer-dynamics-editor-deck">
					<DynamicEncoderDeck
						view={view}
						lane={lane}
						dynamic={dynamic.body}
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
		</div>
	);
}

function FullApplicationDynamicsMock() {
	const [dynamic, setDynamic] = useState(createStoryDynamic);
	const [message, setMessage] = useState(
		"Offline discussion mock · changes are kept in Storybook memory only",
	);
	const [view, setView] = useState<DynamicEditorView>("curves");
	const storyRuntime = useMemo(
		() => ({
			...runtime,
			instances: runtime.instances.map((instance) => ({
				...instance,
				name: dynamic.body.name,
				pool_number: dynamic.body.pool_number,
			})),
			definitions: runtime.definitions.map((status) => ({
				...status,
				lane_count: dynamic.body.lanes.length,
			})),
		}),
		[dynamic],
	);
	const mutate = async (
		intent: DynamicUpdateIntent,
		_mutationGroup?: string,
	) => {
		setDynamic((current) => {
			const revision = current.revision + 1;
			const body = applyIntent(current.body, intent);
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
		<ApplicationStateHarness
			actions={[
				{ type: "SET_DOCK_MODE", mode: "builtins" },
				{ type: "OPEN_BUILTIN", kind: "dynamics" },
			]}
		>
			<AppShellView
				dock={
					<LeftDock
						presentation={{
							showIdentity: "Dynamics UI Review",
							showIndicator: {
								label: "Offline mock",
								detail: "No headless server is connected",
								className: "show-status-warning",
								connected: false,
							},
							clock: <span>12:00</span>,
						}}
					/>
				}
				workspace={
					<GridDesktop id="dynamics-review" name="Dynamics Review">
						<PaneView
							maximized
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
								primary: "Offline discussion surface",
								secondary: message,
							}}
						>
							<DynamicEditor
								dynamic={dynamic}
								compact={false}
								busy={false}
								error={null}
								attributes={attributes}
								presets={[]}
								runtime={storyRuntime}
								selection={runtime.instances[0].targets}
								selectedGroupId="group-front-wash"
								view={view}
								onBack={() => setMessage("Back to Pool requested")}
								onMutate={async (_object, intent, mutationGroup) =>
									mutate(intent, mutationGroup)
								}
								onDelete={() =>
									setMessage("Delete requested · ignored by offline mock")
								}
								onMove={(poolNumber) => {
									setDynamic((current) => ({
										...current,
										revision: current.revision + 1,
										body: {
											...current.body,
											pool_number: poolNumber,
											revision: current.revision + 1,
										},
									}));
									setMessage(`Moved locally to Dynamic ${poolNumber}`);
								}}
								onCopy={(poolNumber) =>
									setMessage(`Copy requested for Dynamic ${poolNumber}`)
								}
							/>
						</PaneView>
					</GridDesktop>
				}
				control={
					<CommandSectionFixture
						initialMode="programmer"
						programmer={
							<DynamicsProgrammerSurface
								dynamic={dynamic}
								onMutate={mutate}
								view={view}
								onView={setView}
							/>
						}
					/>
				}
			/>
		</ApplicationStateHarness>
	);
}

export const FullApplicationDiscussion: Story = {
	render: () => <FullApplicationDynamicsMock />,
};
