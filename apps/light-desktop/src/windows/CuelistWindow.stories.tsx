import type { Meta, StoryObj } from "@storybook/react-vite";
import { type PropsWithChildren, useMemo } from "react";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
	VersionedObject,
} from "../api/types";
import { ShowObjectsStateProvider } from "../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../features/showObjects/store";
import { CuelistWindow } from "./CuelistWindow";

const SHOW_ID = "storybook-cuelists";

const cueList: CueList = {
	id: "main",
	name: "Main Sequence",
	priority: 10,
	mode: "sequence",
	looped: false,
	cues: [
		{
			id: "opening",
			number: 1,
			name: "Opening Look",
			fade_millis: 2_500,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		},
		{
			id: "dialogue",
			number: 2,
			name: "Dialogue",
			fade_millis: 1_200,
			delay_millis: 0,
			trigger: { type: "follow", millis: 8_000 },
			changes: [],
		},
		{
			id: "solo",
			number: 3,
			name: "Solo",
			fade_millis: 1_800,
			delay_millis: 200,
			trigger: { type: "manual" },
			changes: [],
		},
		{
			id: "finale",
			number: 4,
			name: "Finale",
			fade_millis: 3_500,
			delay_millis: 0,
			trigger: { type: "follow", millis: 12_000 },
			changes: [],
		},
		{
			id: "blackout",
			number: 5,
			name: "Blackout",
			fade_millis: 1_000,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		},
		{
			id: "interval",
			number: 6,
			name: "Interval",
			fade_millis: 2_000,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		},
		{
			id: "encore",
			number: 7,
			name: "Encore",
			fade_millis: 1_500,
			delay_millis: 0,
			trigger: { type: "follow", millis: 5_000 },
			changes: [],
		},
		{
			id: "house",
			number: 8,
			name: "House Open",
			fade_millis: 4_000,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		},
	],
};

const playback: PlaybackDefinition = {
	number: 1,
	name: "Main Sequence",
	target: { type: "cue_list", cue_list_id: cueList.id },
	buttons: ["go", "go_minus", "flash"],
	fader: "master",
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
};

const secondaryPlayback: PlaybackDefinition = {
	...playback,
	number: 4,
	name: "Side Sequence",
};

const page: PlaybackPage = {
	number: 1,
	name: "Main",
	slots: { "1": 1 },
};

function versioned<T>(kind: string, id: string, body: T): VersionedObject<T> {
	return {
		kind,
		id,
		body,
		revision: 4,
		updated_at: "2026-07-26T10:00:00Z",
	};
}

function CuelistStoryState({ children }: PropsWithChildren) {
	const store = useMemo(() => {
		const next = new ShowObjectsStore();
		next.reset(SHOW_ID);
		next.setCollection(SHOW_ID, "cue_list", [
			versioned("cue_list", "main-object", cueList),
		]);
		next.setCollection(SHOW_ID, "playback", [
			versioned("playback", "playback-1", playback),
			versioned("playback", "playback-4", secondaryPlayback),
		]);
		next.setCollection(SHOW_ID, "playback_page", [
			versioned("playback_page", "page-1", page),
		]);
		next.setCollection(SHOW_ID, "group", []);
		return next;
	}, []);
	return (
		<ShowObjectsStateProvider store={store}>
			{children}
		</ShowObjectsStateProvider>
	);
}

function cueThumbnail(background: string, left: string, right: string): string {
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="90" viewBox="0 0 160 90"><rect width="160" height="90" fill="${background}"/><path d="M32 8L10 82h58z" fill="${left}" fill-opacity=".52"/><path d="M128 8L92 82h58z" fill="${right}" fill-opacity=".52"/><path d="M0 82h160" stroke="#52616d"/><path d="M12 82h136M22 70h116M33 58h94" stroke="#263039"/></svg>`;
	return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const marketingCueThumbnails = {
	0: cueThumbnail("#101820", "#ffd59a", "#ffc76b"),
	1: cueThumbnail("#0b1520", "#89b8ff", "#ffd2a3"),
	2: cueThumbnail("#170d1d", "#e15ac8", "#68d8ff"),
	3: cueThumbnail("#101622", "#35d6ef", "#df4bc2"),
	4: cueThumbnail("#050608", "#1b242b", "#192127"),
	5: cueThumbnail("#12151a", "#7a8b96", "#6e7f8a"),
	6: cueThumbnail("#15101e", "#ec58d2", "#44d9ef"),
	7: cueThumbnail("#17120c", "#ffd49a", "#ffb76a"),
};

export function MarketingCuesWindow() {
	return (
		<ApplicationStateHarness>
			<CuelistStoryState>
				<div className="ui-window" style={{ height: "100%", minWidth: 0 }}>
					<CuelistWindow
						active
						cueListTab="cues"
						cueListSource="fixed"
						fixedCueListNumber={1}
						showCueSidebar
						thumbnails={marketingCueThumbnails}
					/>
				</div>
			</CuelistStoryState>
		</ApplicationStateHarness>
	);
}

const meta = {
	title: "ToskLight/Windows/Cuelists and Cues",
	component: CuelistWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	excludeStories: /^(Marketing|marketing)/,
	decorators: [
		(Story) => (
			<ApplicationStateHarness>
				<CuelistStoryState>
					<div style={{ height: 680, minWidth: 820 }}>
						<Story />
					</div>
				</CuelistStoryState>
			</ApplicationStateHarness>
		),
	],
} satisfies Meta<typeof CuelistWindow>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Pool: Story = {
	args: { active: true, cueListTab: "pool" },
};

export const PoolCompact: Story = {
	args: { active: true, compact: true, cueListTab: "pool" },
};

export const PoolNarrowShort: Story = {
	args: { active: true, cueListTab: "pool" },
	render: (args) => (
		<div style={{ width: 420, height: 380, minWidth: 0 }}>
			<CuelistWindow {...args} />
		</div>
	),
};

export const PoolWideTall: Story = {
	args: { active: true, cueListTab: "pool" },
	render: (args) => (
		<div style={{ width: 1280, height: 760, minWidth: 0 }}>
			<CuelistWindow {...args} />
		</div>
	),
};

export const CuesWithProperties: Story = {
	args: {
		active: true,
		cueListTab: "cues",
		cueListSource: "fixed",
		fixedCueListNumber: 1,
		showCueSidebar: true,
	},
};

export const CuesMarketing: Story = {
	render: () => <MarketingCuesWindow />,
};

export const FixedCuesUnavailable: Story = {
	args: {
		active: true,
		cueListTab: "cues",
		cueListSource: "fixed",
		fixedCueListNumber: 99,
		showCueSidebar: true,
	},
};

export const FollowSelectionEmpty: Story = {
	args: {
		active: true,
		cueListTab: "cues",
		cueListSource: "follow-selection",
		showCueSidebar: true,
	},
};
