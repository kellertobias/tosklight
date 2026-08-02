import type { Meta, StoryObj } from "@storybook/react-vite";
import { type CSSProperties, useMemo, useState } from "react";
import {
	marketingColorPresets,
	marketingGroupCards,
	marketingKnownFixtureIds,
	marketingPositionPresets,
} from "../../../ui-library/storybook/fixtures/marketingApplication";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type { CommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { defaultPoolPresentation } from "../features/poolPresentation/poolPresentation";
import type { PresetCard } from "../features/presetRecording/presetCards";
import { ShowObjectsStateProvider } from "../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../features/showObjects/store";
import type { PresetFamily } from "../presetFamilies";
import { GroupPoolHeader } from "./GroupsWindow";
import { GroupPoolGrid } from "./groupsWindow/GroupPoolGrid";
import type { Group } from "./groupsWindow/model";
import {
	PresetCardGrid,
	PresetWindowHeader,
} from "./presetsWindow/PresetsWindowView";

export {
	marketingColorPresets,
	marketingPositionPresets,
} from "../../../ui-library/storybook/fixtures/marketingApplication";

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

export interface PoolViewport {
	width?: number;
	height?: number | string;
	showHeader?: boolean;
}

function poolViewportStyle({
	width,
	height = "100%",
}: PoolViewport): CSSProperties {
	return { width, height, minWidth: 0 };
}

export function MarketingGroupsWindow({
	cards = marketingGroupCards,
	knownFixtureIds = marketingKnownFixtureIds,
	showHeader = true,
	...viewport
}: PoolViewport & {
	cards?: (Group | null)[];
	knownFixtureIds?: Set<string>;
} = {}) {
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
					className="ui-window pool-window group-pool-window"
					style={poolViewportStyle(viewport)}
				>
					{showHeader && (
						<GroupPoolHeader command={command} onSettings={() => undefined} />
					)}
					<GroupPoolGrid
						active={false}
						cards={cards}
						capabilities={new Map()}
						knownFixtureIds={knownFixtureIds}
						command={command}
						onOpenSettings={(id) => setInteraction(`Settings Group ${id}`)}
						onOpenRecord={(target) => setInteraction(`Record ${target.label}`)}
						recordGroup={async (target) =>
							setInteraction(`Recorded ${target.label}`)
						}
					/>
					<output aria-label="Group pool interaction" hidden>
						{interaction}
					</output>
				</div>
			</ShowObjectsStateProvider>
		</ApplicationStateHarness>
	);
}

function cardsForPresets(presets: readonly PresetCard[]) {
	return Array.from(
		{ length: 200 },
		(_, index) =>
			presets.find((candidate) => candidate.body.number === index + 1) ?? null,
	);
}

export function MarketingPresetWindow({
	family,
	presets,
	showHeader = true,
	...viewport
}: PoolViewport & {
	family: PresetFamily;
	presets: readonly PresetCard[];
}) {
	const [interaction, setInteraction] = useState("Ready");
	const cards = cardsForPresets(presets);
	return (
		<ApplicationStateHarness>
			<div
				className={`ui-window pool-window preset-pool-window pool-colors pool-family-${family.toLowerCase()}`}
				style={poolViewportStyle(viewport)}
			>
				{showHeader && (
					<PresetWindowHeader
						family={family}
						compact
						showFamilyActions={false}
						onFamily={() => undefined}
						onOpenGroups={() => undefined}
						onSettings={() => undefined}
					/>
				)}
				<PresetCardGrid
					cards={cards}
					family={family}
					customizations={{}}
					poolPresentation={defaultPoolPresentation()}
					showId={SHOW_ID}
					surfaceKey={`show:${SHOW_ID}:builtin:preset`}
					fallbackMode="type"
					selectionCount={2}
					recallReady
					storeArmed={false}
					updateArmed={false}
					setArmed={false}
					onActivate={(index) =>
						setInteraction(`Activated ${family} ${index + 1}`)
					}
				/>
				<output aria-label="Preset pool interaction" hidden>
					{interaction}
				</output>
			</div>
		</ApplicationStateHarness>
	);
}

export function MarketingColorPresetsWindow(viewport: PoolViewport = {}) {
	return (
		<MarketingPresetWindow
			{...viewport}
			family="Color"
			presets={marketingColorPresets}
		/>
	);
}

export function MarketingPositionPresetsWindow(viewport: PoolViewport = {}) {
	return (
		<MarketingPresetWindow
			{...viewport}
			family="Position"
			presets={marketingPositionPresets}
		/>
	);
}

const meta = {
	title: "ToskLight/Windows/Pools",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	excludeStories: /^(Marketing|marketing)/,
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Groups: Story = {
	render: () => <MarketingGroupsWindow />,
};

export const Presets: Story = {
	render: () => <MarketingColorPresetsWindow />,
};

export const ColorPresetsMarketing: Story = {
	render: () => <MarketingColorPresetsWindow />,
};

export const PositionPresetsMarketing: Story = {
	render: () => <MarketingPositionPresetsWindow />,
};

export const GroupsNarrowShort: Story = {
	render: () => <MarketingGroupsWindow width={420} height={380} />,
};

export const GroupsWideTall: Story = {
	render: () => <MarketingGroupsWindow width={1280} height={760} />,
};

export const GroupsStatusMarkers: Story = {
	render: () => (
		<MarketingGroupsWindow
			cards={statusGroupCards}
			knownFixtureIds={new Set(["fixture-1", "fixture-2", "fixture-3"])}
		/>
	),
};

export const PresetsNarrowShort: Story = {
	render: () => <MarketingColorPresetsWindow width={420} height={380} />,
};

export const PresetsWideTall: Story = {
	render: () => <MarketingColorPresetsWindow width={1280} height={760} />,
};
