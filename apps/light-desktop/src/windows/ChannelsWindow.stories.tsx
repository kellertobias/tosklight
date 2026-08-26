import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AppProvider } from "../state/AppContext";
import { type Channel, ChannelsWindowView } from "./ChannelsWindow";

const CHANNEL_FIXTURE_NAMES = [
	"Front Fresnel 1",
	"Front Fresnel 2",
	"Front Fresnel 3",
	"Front Fresnel 4",
	"Wash Left",
	"Wash Right",
	"Spot Left",
	"Spot Right",
	"Backlight 1",
	"Backlight 2",
	"Cyc Blue",
	"Cyc Amber",
	"Haze",
	"House",
];

const channels = Array.from({ length: 14 }, (_, index) => ({
	number: index + 1,
	fixture: {
		fixture_id: `fixture-${index + 1}`,
		fixture_number: index + 1,
	} as Channel["fixture"],
	fixtureLabel: CHANNEL_FIXTURE_NAMES[index],
	fixtureId: String(index + 1),
	attribute: "intensity",
	attributeLabel: "Intensity",
	level: [70, 70, 45, 45, 30, 30, 85, 65, 50, 40, 25, 15, 0, 100][index],
})) satisfies Channel[];

function ChannelsStory({ compact = false }: { compact?: boolean }) {
	const [page, setPage] = useState(0);
	const [picker, setPicker] = useState(false);
	const [selected, setSelected] = useState<ReadonlySet<string>>(
		new Set(["fixture-2"]),
	);
	return (
		<AppProvider>
			<div style={{ height: "100vh" }}>
				<ChannelsWindowView
					channels={channels}
					compact={compact}
					page={page}
					pages={8}
					pagePickerOpen={picker}
					selectedFixtureIds={selected}
					valuesReady
					onPage={setPage}
					onPagePickerOpen={setPicker}
					onSelect={(fixtureId) => setSelected(new Set([fixtureId]))}
					onSetValue={() => undefined}
				/>
			</div>
		</AppProvider>
	);
}

const meta = {
	title: "ToskLight/Windows/Channels",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj;

export const TwoRowBank: Story = {
	render: () => <ChannelsStory />,
};

export const Compact: Story = {
	render: () => <ChannelsStory compact />,
};
