import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "@tosklight/ui/controls";
import { useMemo, useState } from "react";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
} from "../../../api/types";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import {
	CUE_LIST_ID,
	cueProjection,
} from "../../../features/playbackRuntime/testFixtures";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import { VirtualPlaybackToolbar } from "../../../windows/VirtualPlaybacksWindow";
import { VirtualPlaybackGrid } from "./VirtualPlaybackGrid";

interface VirtualPlaybackStoryArgs {
	rows: number;
	columns: number;
	width: number;
}

const meta = {
	title: "ToskLight/Virtual Playbacks",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	args: { rows: 3, columns: 4, width: 920 },
	argTypes: {
		rows: { control: { type: "range", min: 1, max: 12, step: 1 } },
		columns: { control: { type: "range", min: 1, max: 12, step: 1 } },
		width: { control: { type: "range", min: 320, max: 1400, step: 20 } },
	},
} satisfies Meta<VirtualPlaybackStoryArgs>;

export default meta;
type Story = StoryObj<VirtualPlaybackStoryArgs>;

const playbackDefinitions: PlaybackDefinition[] = [
	{ ...playback(7, "Main", "go", "#176777"), presentation_icon: "☀" },
	{
		...playback(8, "Bump", "flash", "#d98236"),
		presentation_image:
			"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 160 100'%3E%3Crect width='160' height='100' fill='%23111820'/%3E%3Ccircle cx='80' cy='50' r='28' fill='%23d98236'/%3E%3C/svg%3E",
	},
	playback(9, "House", "swap", "#2874bd"),
	playback(10, "Light Color", "go", "#f6d365"),
	playback(11, "A deliberately long production playback name", "go", "#663399"),
	playback(12, "Colorless Playback", "go"),
];
const playbacks = new Map(
	playbackDefinitions.map((definition) => [definition.number, definition]),
);
const cueLists = new Map<string, CueList>([
	[
		CUE_LIST_ID,
		{
			id: CUE_LIST_ID,
			name: "Main Cuelist",
			mode: "sequence",
			priority: 0,
			looped: false,
			cues: [
				{
					id: "44444444-4444-4444-8444-444444444444",
					number: 4,
					name: "Solo",
					fade_millis: 2500,
					delay_millis: 0,
					trigger: { type: "manual" },
					changes: [],
				},
			],
		},
	],
]);
const pageOne: PlaybackPage = {
	number: 1,
	name: "Main",
	slots: { "1": 7, "4": 8 },
};
const pageTwo: PlaybackPage = {
	number: 2,
	name: "House",
	slots: { "1": 9, "3": 8 },
};

function playback(
	number: number,
	name: string,
	action: PlaybackDefinition["buttons"][number],
	color?: string,
): PlaybackDefinition {
	return {
		number,
		name,
		target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
		buttons: [action, "none", "none"],
		button_count: 1,
		fader: "master",
		has_fader: false,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color,
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

function StoryGrid({
	page = pageOne,
	rows,
	columns,
	width,
	configurationArmed = false,
	assignmentPending = false,
	updateArmed = false,
	shiftArmed = false,
	selectedSlots = [],
	zones = [],
	runningNumbers = [7],
}: VirtualPlaybackStoryArgs & {
	page?: PlaybackPage;
	configurationArmed?: boolean;
	assignmentPending?: boolean;
	updateArmed?: boolean;
	shiftArmed?: boolean;
	selectedSlots?: readonly number[];
	zones?: readonly VirtualPlaybackZone[];
	runningNumbers?: readonly number[];
}) {
	const [event, setEvent] = useState("Ready");
	const runtimeActions = useMemo(
		() =>
			({
				poolPlaybackAction: async (
					number: number,
					_action: string,
					input?: { pressed?: boolean },
				) => {
					setEvent(
						input?.pressed === false
							? `Released ${number}`
							: playbackDefinitions.find((item) => item.number === number)
										?.buttons[0] === "flash" ||
									playbackDefinitions.find((item) => item.number === number)
										?.buttons[0] === "swap"
								? `Pressed ${number}`
								: `Action ${number}`,
					);
					return null;
				},
			}) as PlaybackRuntimeActions,
		[],
	);
	return (
		<div
			style={{
				width,
				height: 680,
				display: "grid",
				gridTemplateRows: "32px minmax(0, 1fr)",
			}}
		>
			<output>{event}</output>
			<VirtualPlaybackGrid
				pageNumber={page.number}
				page={page}
				rows={rows}
				columns={columns}
				playbacks={playbacks}
				cueLists={cueLists}
				runtimes={
					new Map(
						runningNumbers.map((number) => [number, cueProjection(number, 3)]),
					)
				}
				runtimeActions={runtimeActions}
				zones={zones}
				selectedSlots={selectedSlots}
				configurationArmed={configurationArmed}
				assignmentPending={assignmentPending}
				assignmentTarget={7}
				updateArmed={updateArmed}
				shiftArmed={shiftArmed}
				onConfigure={(_playback, slot) => setEvent(`Configure ${slot}`)}
				onAssign={(slot) => setEvent(`Assign ${slot}`)}
				onToggleZone={(slot) => setEvent(`Zone ${slot}`)}
			/>
		</div>
	);
}

export const SparseGrid: Story = {
	render: (args) => <StoryGrid {...args} />,
};

export const DocumentationPane: Story = {
	args: { rows: 2, columns: 2, width: 1496 },
	render: (args) => (
		<section
			className="virtual-playback-pane"
			aria-label="Virtual Playbacks page 1"
			style={{ width: args.width, height: 713 }}
		>
			<VirtualPlaybackToolbar
				pageNumber={1}
				rows={args.rows}
				columns={args.columns}
				zonesReady
				zoneError={null}
				actionError={null}
				zoneCount={0}
				selectedSlots={[]}
				onSetSource={() => undefined}
				onAddTarget={() => undefined}
				onCreateZone={() => undefined}
				onCancelZone={() => undefined}
			/>
			<VirtualPlaybackGrid
				pageNumber={1}
				page={{
					number: 1,
					name: "Main",
					slots: { "1": 7, "2": 8, "3": 9, "4": 10 },
				}}
				rows={args.rows}
				columns={args.columns}
				playbacks={playbacks}
				cueLists={cueLists}
				runtimes={new Map([[7, cueProjection(7, 3)]])}
				runtimeActions={{} as PlaybackRuntimeActions}
				zones={[]}
				selectedSlots={[]}
				configurationArmed={false}
				assignmentPending={false}
				assignmentTarget={null}
				updateArmed={false}
				shiftArmed={false}
				onConfigure={() => undefined}
				onAssign={() => undefined}
				onToggleZone={() => undefined}
			/>
		</section>
	),
};

export const EveryState: Story = {
	args: { rows: 2, columns: 4 },
	render: (args) => (
		<StoryGrid
			{...args}
			configurationArmed
			assignmentPending
			updateArmed
			shiftArmed
			selectedSlots={[4]}
			zones={[{ id: "zone-1", name: "Front alternates", slots: [1, 4] }]}
		/>
	),
};

export const ConfigurationState: Story = {
	render: (args) => <StoryGrid {...args} configurationArmed />,
};

export const InactiveColored: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{ ...pageOne, slots: { "1": 9 } }}
			runningNumbers={[]}
		/>
	),
};

export const RunningLightColor: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{ ...pageOne, slots: { "1": 10 } }}
			runningNumbers={[10]}
		/>
	),
};

export const RunningTransition: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => {
		const [running, setRunning] = useState(false);
		return (
			<div>
				<Button onClick={() => setRunning((current) => !current)}>
					Toggle running
				</Button>
				<StoryGrid
					{...args}
					page={{ ...pageOne, slots: { "1": 7 } }}
					runningNumbers={running ? [7] : []}
				/>
			</div>
		);
	},
};

export const IconAndImageArtwork: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{ ...pageOne, slots: { "1": 7, "2": 8 } }}
			runningNumbers={[]}
		/>
	),
};

export const LongLabelsAndColorless: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{ ...pageOne, slots: { "1": 11, "2": 12 } }}
			runningNumbers={[]}
		/>
	),
};

export const AssignmentState: Story = {
	render: (args) => <StoryGrid {...args} assignmentPending />,
};

export const UpdateState: Story = {
	render: (args) => <StoryGrid {...args} updateArmed />,
};

export const ExclusionZoneState: Story = {
	render: (args) => (
		<StoryGrid
			{...args}
			shiftArmed
			selectedSlots={[4]}
			zones={[{ id: "zone-1", name: "Front alternates", slots: [1, 4] }]}
		/>
	),
};

export const UnavailableSlots: Story = {
	args: { rows: 12, columns: 12, width: 1320 },
	render: (args) => <StoryGrid {...args} />,
};

export const HeldFlashAndSwap: Story = {
	args: { rows: 1, columns: 4 },
	render: (args) => (
		<StoryGrid {...args} page={{ ...pageOne, slots: { "1": 8, "2": 9 } }} />
	),
};

export const PageSwitching: Story = {
	render: (args) => {
		const [page, setPage] = useState(pageOne);
		return (
			<div
				style={{
					width: args.width,
					height: 720,
					display: "grid",
					gridTemplateRows: "44px minmax(0, 1fr)",
				}}
			>
				<div>
					<Button
						onClick={() =>
							setPage((current) => (current.number === 1 ? pageTwo : pageOne))
						}
					>
						Next page
					</Button>
					<span role="status" aria-label="Current virtual playback page">
						Page {page.number}
					</span>
				</div>
				<StoryGrid {...args} page={page} width={args.width} />
			</div>
		);
	},
};

export const NarrowTouch: Story = {
	args: { width: 360, rows: 3, columns: 4 },
	render: (args) => <StoryGrid {...args} />,
};

export const WideTouch: Story = {
	args: { width: 1320, rows: 3, columns: 6 },
	render: (args) => <StoryGrid {...args} />,
};
