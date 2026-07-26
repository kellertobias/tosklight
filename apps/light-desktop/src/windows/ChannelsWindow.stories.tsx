import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { AppProvider } from "../state/AppContext";
import { type Channel, ChannelsWindowView } from "./ChannelsWindow";

const channels = Array.from({ length: 14 }, (_, index) => ({
	number: index + 1,
	fixture: {
		fixture_id: `fixture-${index + 1}`,
		fixture_number: index + 1,
	} as Channel["fixture"],
	name: index < 6 ? "Fresnel" : "Moving Wash",
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
					onSetIntensity={() => undefined}
				/>
			</div>
		</AppProvider>
	);
}

const meta = {
	title: "Application/Windows/Channels",
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
