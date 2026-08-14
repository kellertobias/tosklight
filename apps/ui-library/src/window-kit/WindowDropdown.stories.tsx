import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { WindowDropdown } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Window Dropdown",
	component: WindowDropdown,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof WindowDropdown>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [selected, setSelected] = useState("2D");
		return (
			<div className="forms-story-canvas">
				<WindowDropdown
					label={`View: ${selected}`}
					ariaLabel="Stage view"
					items={[
						{ id: "2d", label: "2D", onSelect: () => setSelected("2D") },
						{ id: "3d", label: "3D", onSelect: () => setSelected("3D") },
						{
							id: "plan",
							label: "Plan unavailable",
							disabled: true,
							onSelect: () => undefined,
						},
					]}
				/>
				<output aria-label="Selected stage view">{selected}</output>
			</div>
		);
	},
};
