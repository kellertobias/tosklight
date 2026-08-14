import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import { WindowFrame, WindowScrollArea } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Window Frame",
	component: WindowFrame,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof WindowFrame>;

export default meta;
type Story = StoryObj;

function WindowFrameCatalog() {
	const [query, setQuery] = useState("");
	return (
		<div style={{ height: 620, containerType: "inline-size" }}>
			<WindowFrame
				title="Fixture Sheet"
				info={{ primary: "12 fixtures", secondary: "4 selected" }}
				search={{ value: query, onSearch: setQuery }}
				groups={[
					{
						id: "highlight",
						actions: [
							{
								id: "highlight",
								label: "Highlight",
								active: true,
								onPress: () => undefined,
							},
						],
					},
					{
						id: "selection",
						actions: [
							{
								id: "select",
								label: "Select fixtures",
								onPress: () => undefined,
							},
						],
					},
				]}
				settingsTabs={[
					{ id: "columns", label: "Columns", content: "Column settings" },
					{ id: "display", label: "Display", content: "Display settings" },
				]}
				navigation={<Button fullWidth>All fixtures</Button>}
				infoSection={<p>Fixture and logical-head information</p>}
				bottom={
					<div style={{ padding: 10 }}>
						Programmer values use LTP semantics.
					</div>
				}
			>
				<WindowScrollArea>
					<div style={{ padding: 12 }}>Production window composition</div>
				</WindowScrollArea>
			</WindowFrame>
		</div>
	);
}

export const Primary: Story = { render: () => <WindowFrameCatalog /> };
