import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import { WindowHeader } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Window Header",
	component: WindowHeader,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof WindowHeader>;

export default meta;
type Story = StoryObj;

function HeaderCatalog() {
	const [query, setQuery] = useState("");
	const [follow, setFollow] = useState(true);
	return (
		<div style={{ display: "grid", gap: 14 }}>
			<WindowHeader title="Stage" settings onSettings={() => undefined} />
			<WindowHeader
				title="Fixture Sheet"
				info={{ primary: "4 selected", secondary: "Shift for range" }}
				search={{ value: query, onSearch: setQuery }}
				groups={[
					{
						id: "follow",
						actions: [
							{
								id: "follow",
								label: "Follow Preload",
								active: follow,
								onPress: () => setFollow((value) => !value),
							},
						],
					},
				]}
				toolbar={<Button size="compact">Select fixtures</Button>}
				settings
				onSettings={() => undefined}
			/>
			<WindowHeader
				title="Cuelist"
				groups={[
					{
						id: "store",
						actions: [
							{ id: "store", label: "Store", onPress: () => undefined },
						],
					},
				]}
			/>
		</div>
	);
}

export const Primary: Story = { render: () => <HeaderCatalog /> };
