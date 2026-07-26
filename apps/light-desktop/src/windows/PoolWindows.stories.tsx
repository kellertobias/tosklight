import type { Meta, StoryObj } from "@storybook/react-vite";
import { type CSSProperties, useMemo, useState } from "react";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type { CommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { WindowHeader } from "@tosklight/ui/window-kit";
import type { PresetCard } from "../features/presetRecording/presetCards";
import { ShowObjectsStateProvider } from "../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../features/showObjects/store";
import { GroupPoolGrid } from "./groupsWindow/GroupPoolGrid";
import type { Group } from "./groupsWindow/model";
import { PresetCardGrid } from "./presetsWindow/PresetsWindowView";

const SHOW_ID = "storybook-pools";
const noopAsync = async () => false;

function group(
	id: string,
	name: string,
	fixtures: string[],
	options: Partial<Group["body"]> = {},
): Group {
	return {
		kind: "group",
		id,
		revision: 3,
		updated_at: "2026-07-26T10:00:00Z",
		body: {
			name,
			fixtures,
			master: 1,
			playback_fader: null,
			programming: {},
			derived_from: null,
			frozen_from: null,
			...options,
		},
	};
}

const groups = [
	group("1", "All Fixtures", ["fixture-1", "fixture-2", "fixture-3"]),
	group("4", "Front Fresnels", ["fixture-1", "fixture-2"], {
		color: "#06b6d4",
		icon: "★",
	}),
	group("12", "Stored Empty", []),
] satisfies Group[];

const groupsWithStatus = [
	...groups,
	group("20", "Derived Odds", ["fixture-1", "fixture-3"], {
		derived_from: {
			source_group_id: "1",
			rule: { type: "odd" },
		},
	}),
	group("21", "Frozen Front", ["fixture-1", "fixture-2"], {
		frozen_from: {
			source_group_id: "4",
			source_revision: 2,
			captured_at: "2026-07-26T10:00:00Z",
		},
	}),
] satisfies Group[];

function cardsForGroups(objects: readonly Group[]) {
	return Array.from(
		{ length: 200 },
		(_, index) =>
			objects.find((candidate) => candidate.id === String(index + 1)) ?? null,
	);
}

const groupCards = cardsForGroups(groups);
const statusGroupCards = cardsForGroups(groupsWithStatus);

const command: CommandLineSurface = {
	ready: false,
	text: "",
	target: "FIXTURE",
	pristine: true,
	selected: ["fixture-1", "fixture-2"],
	selectedGroupId: "4",
	read: () => ({
		ready: false,
		text: "",
		target: "FIXTURE",
		pristine: true,
	}),
	replace: noopAsync,
	reset: noopAsync,
	execute: noopAsync,
	cancelChoice: noopAsync,
};

interface PoolViewport {
	width?: number;
	height?: number;
}

function poolViewportStyle({
	width,
	height = 680,
}: PoolViewport): CSSProperties {
	return { width, height, minWidth: 0 };
}

function GroupPoolStory({
	cards = groupCards,
	...viewport
}: PoolViewport & { cards?: (Group | null)[] } = {}) {
	const [interaction, setInteraction] = useState("Ready");
	const store = useMemo(() => {
		const next = new ShowObjectsStore();
		next.reset(SHOW_ID);
		next.setCollection(
			SHOW_ID,
			"group",
			cards.filter((group): group is Group => group !== null),
		);
		return next;
	}, [cards]);
	return (
		<ApplicationStateHarness>
			<ShowObjectsStateProvider store={store}>
				<div
					className="pool-window group-pool-window"
					style={poolViewportStyle(viewport)}
				>
					<WindowHeader
						title="Group Pool"
						info={{
							primary: "2 fixtures selected",
							secondary: "Ordered selection",
						}}
						actions={[]}
					/>
					<GroupPoolGrid
						active={false}
						cards={cards}
						capabilities={new Map()}
						knownFixtureIds={new Set(["fixture-1", "fixture-2", "fixture-3"])}
						command={command}
						onOpenContext={(id) => setInteraction(`Context Group ${id}`)}
						onOpenProperties={(id) => setInteraction(`Properties Group ${id}`)}
						onOpenRecord={(target) => setInteraction(`Record ${target.label}`)}
						recordGroup={async (target) =>
							setInteraction(`Recorded ${target.label}`)
						}
						runCommand={async (value) => setInteraction(value)}
					/>
					<output aria-label="Group pool interaction" hidden>
						{interaction}
					</output>
				</div>
			</ShowObjectsStateProvider>
		</ApplicationStateHarness>
	);
}

const presets = [
	{
		id: "2.1",
		body: {
			name: "Open White",
			number: 1,
			family: "Color",
			values: { "fixture-1": { color: "#ffffff" } },
			color: "#ffffff",
			icon: "◇",
		},
	},
	{
		id: "2.5",
		body: {
			name: "Lavender",
			number: 5,
			family: "Color",
			values: { "fixture-1": { color: "#b56cff" } },
			color: "#b56cff",
			icon: "◆",
		},
	},
] satisfies PresetCard[];

const presetCards = Array.from(
	{ length: 200 },
	(_, index) =>
		presets.find((candidate) => candidate.body.number === index + 1) ?? null,
);

function PresetPoolStory(viewport: PoolViewport = {}) {
	const [interaction, setInteraction] = useState("Ready");
	return (
		<ApplicationStateHarness>
			<div
				className="pool-window preset-pool-window pool-colors pool-family-color"
				style={poolViewportStyle(viewport)}
			>
				<WindowHeader
					title="Preset Pools"
					info={{ primary: "Color presets" }}
					actions={[]}
				/>
				<PresetCardGrid
					cards={presetCards}
					family="Color"
					customizations={{}}
					colorsEnabled
					selectionCount={2}
					storeArmed
					updateArmed={false}
					setArmed={false}
					onActivate={(index) => setInteraction(`Activated Color ${index + 1}`)}
				/>
				<output aria-label="Preset pool interaction" hidden>
					{interaction}
				</output>
			</div>
		</ApplicationStateHarness>
	);
}

const meta = {
	title: "Application/Windows/Pools",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Groups: Story = {
	render: () => <GroupPoolStory />,
};

export const Presets: Story = {
	render: () => <PresetPoolStory />,
};

export const GroupsNarrowShort: Story = {
	render: () => <GroupPoolStory width={420} height={380} />,
};

export const GroupsWideTall: Story = {
	render: () => <GroupPoolStory width={1280} height={760} />,
};

export const GroupsStatusMarkers: Story = {
	render: () => <GroupPoolStory cards={statusGroupCards} />,
};

export const PresetsNarrowShort: Story = {
	render: () => <PresetPoolStory width={420} height={380} />,
};

export const PresetsWideTall: Story = {
	render: () => <PresetPoolStory width={1280} height={760} />,
};
