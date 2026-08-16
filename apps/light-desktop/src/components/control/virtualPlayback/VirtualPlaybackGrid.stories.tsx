import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "@tosklight/ui/controls";
import { useMemo, useState } from "react";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
} from "../../../api/types";
import { virtualPlaybackNumber } from "../../../api/virtualPlaybackAddress";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import {
	identityKey,
	virtualPlaybackIdentity,
} from "../../../features/playbackRuntime/contracts";
import {
	CUE_LIST_ID,
	cueProjection,
} from "../../../features/playbackRuntime/testFixtures";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
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
		rows: { control: { type: "number", min: 1, max: 300, step: 1 } },
		columns: { control: { type: "number", min: 1, max: 300, step: 1 } },
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
					number: "4",
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
	virtual_playbacks: {
		"1001": virtualPlayback(1, 1, 7),
		"1004": virtualPlayback(1, 4, 8),
	},
};
const pageTwo: PlaybackPage = {
	number: 2,
	name: "House",
	slots: { "1": 9, "3": 8 },
	virtual_playbacks: {
		"1301": virtualPlayback(2, 1, 9),
		"1303": virtualPlayback(2, 3, 8),
	},
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

function virtualPlayback(page: number, cell: number, sourceNumber: number) {
	const source = playbacks.get(sourceNumber);
	if (!source) throw new Error(`Missing Storybook Playback ${sourceNumber}`);
	return { ...source, number: virtualPlaybackNumber(page, cell) };
}

function StoryGrid({
	page = pageOne,
	rows,
	columns,
	width,
	configurationArmed = false,
	updateArmed = false,
	shiftArmed = false,
	selectedSlots = [],
	zones = [],
	runningNumbers = [1001],
}: VirtualPlaybackStoryArgs & {
	page?: PlaybackPage;
	configurationArmed?: boolean;
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
				virtualPlaybackAction: async (
					_page: number,
					number: number,
					_action: string,
					input?: { pressed?: boolean },
				) => {
					const playback = page.virtual_playbacks[String(number)];
					const heldAction =
						playback?.buttons[0] === "flash" ||
						playback?.buttons[0] === "swap";
					setEvent(
						input?.pressed === false
							? `Released ${number}`
							: input?.pressed === true && heldAction
								? `Pressed ${number}`
								: `Action ${number}`,
					);
					return null;
				},
			}) as PlaybackRuntimeActions,
		[page],
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
				pageObjectId="story-page-one"
				pageObjectRevision={1}
				rows={rows}
				columns={columns}
				playbacks={playbacks}
				cueLists={cueLists}
				runtimes={
					new Map(
						runningNumbers.map((number) => [
							identityKey(virtualPlaybackIdentity(page.number, number)),
							{
								...cueProjection(number, 3),
								requested: virtualPlaybackIdentity(page.number, number),
							},
						]),
					)
				}
				runtimeActions={runtimeActions}
				zones={zones}
				selectedSlots={selectedSlots}
				configurationArmed={configurationArmed}
				updateArmed={updateArmed}
				shiftArmed={shiftArmed}
				onConfigure={(_playback, slot) => setEvent(`Configure ${slot}`)}
				onToggleZone={(slot) => setEvent(`Zone ${slot}`)}
			/>
		</div>
	);
}

export const SparseGrid: Story = {
	render: (args) => <StoryGrid {...args} />,
};

export const FullTwentyByFifteenPage: Story = {
	args: { rows: 20, columns: 15, width: 920 },
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
			<VirtualPlaybackGrid
				pageNumber={1}
				page={{
					number: 1,
					name: "Main",
					slots: { "1": 7, "2": 8, "3": 9, "4": 10 },
					virtual_playbacks: {
						"1001": virtualPlayback(1, 1, 7),
						"1002": virtualPlayback(1, 2, 8),
						"1003": virtualPlayback(1, 3, 9),
						"1004": virtualPlayback(1, 4, 10),
					},
				}}
				pageObjectId="story-page-one"
				pageObjectRevision={1}
				rows={args.rows}
				columns={args.columns}
				playbacks={playbacks}
				cueLists={cueLists}
				runtimes={
					new Map([
						[
							identityKey(virtualPlaybackIdentity(1, 1001)),
							{
								...cueProjection(1001, 3),
								requested: virtualPlaybackIdentity(1, 1001),
							},
						],
					])
				}
				runtimeActions={{} as PlaybackRuntimeActions}
				zones={[]}
				selectedSlots={[]}
				configurationArmed={false}
				updateArmed={false}
				shiftArmed={false}
				onConfigure={() => undefined}
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
			updateArmed
			shiftArmed
			selectedSlots={[4]}
			zones={[
				{
					id: "zone-1",
					name: "Front alternates",
					playbackNumbers: [1001, 1004],
				},
			]}
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
			page={{
				...pageOne,
				slots: { "1": 9 },
				virtual_playbacks: { "1001": virtualPlayback(1, 1, 9) },
			}}
			runningNumbers={[]}
		/>
	),
};

export const RunningLightColor: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{
				...pageOne,
				slots: { "1": 10 },
				virtual_playbacks: { "1001": virtualPlayback(1, 1, 10) },
			}}
			runningNumbers={[1001]}
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
					page={{
						...pageOne,
						slots: { "1": 7 },
						virtual_playbacks: { "1001": virtualPlayback(1, 1, 7) },
					}}
					runningNumbers={running ? [1001] : []}
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
			page={{
				...pageOne,
				virtual_playbacks: {
					"1001": virtualPlayback(1, 1, 7),
					"1002": virtualPlayback(1, 2, 8),
				},
			}}
			runningNumbers={[]}
		/>
	),
};

export const LongLabelsAndColorless: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{
				...pageOne,
				virtual_playbacks: {
					"1001": virtualPlayback(1, 1, 11),
					"1002": virtualPlayback(1, 2, 12),
				},
			}}
			runningNumbers={[]}
		/>
	),
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
			zones={[
				{
					id: "zone-1",
					name: "Front alternates",
					playbackNumbers: [1001, 1004],
				},
			]}
		/>
	),
};

export const SparseLargeGrid: Story = {
	args: { rows: 15, columns: 20, width: 1320 },
	render: (args) => <StoryGrid {...args} />,
};

export const PinnedPage: Story = {
	args: { rows: 2, columns: 2 },
	render: (args) => <StoryGrid {...args} page={pageTwo} />,
};

export const OverlappingZones: Story = {
	args: { rows: 2, columns: 2 },
	render: (args) => (
		<StoryGrid
			{...args}
			shiftArmed
			selectedSlots={[4]}
			zones={[
				{
					id: "zone-front",
					name: "Front alternates",
					playbackNumbers: [1001, 1004],
				},
				{
					id: "zone-bump",
					name: "Bump alternates",
					playbackNumbers: [1003, 1004],
				},
			]}
		/>
	),
};

export const HiddenMembership: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<div>
			<p role="status">
				Virtual Playback 1301 remains a saved member on page 2.
			</p>
			<StoryGrid
				{...args}
				zones={[
					{
						id: "zone-hidden",
						name: "Touring alternates",
						playbackNumbers: [1001, 1301],
					},
				]}
			/>
		</div>
	),
};

export const ZoneErrorState: Story = {
	args: { rows: 2, columns: 2 },
	render: (args) => (
		<section
			className="virtual-playback-pane"
			aria-label="Virtual Playbacks page 1"
			style={{ width: args.width, height: 713 }}
		>
			<StoryGrid {...args} />
			<p className="virtual-playback-pane-error" role="alert">
				Zone revision changed on another screen
			</p>
		</section>
	),
};

export const HeldFlashAndSwap: Story = {
	args: { rows: 1, columns: 4 },
	render: (args) => (
		<StoryGrid
			{...args}
			page={{
				...pageOne,
				virtual_playbacks: {
					"1001": virtualPlayback(1, 1, 8),
					"1002": virtualPlayback(1, 2, 9),
				},
			}}
		/>
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
