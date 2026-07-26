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

const meta = {
	title: "Application/Windows/Cuelists and Cues",
	component: CuelistWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
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
