import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { MultiValueToggleField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Multi Value Toggle",
	component: MultiValueToggleField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof MultiValueToggleField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("2d");
		return (
			<div className="forms-story-canvas">
				<MultiValueToggleField
					label="Stage view"
					value={value}
					options={[
						{ value: "2d", label: "2D" },
						{ value: "3d", label: "3D" },
						{ value: "plan", label: "Plan", disabled: true },
					]}
					onChange={setValue}
				/>
			</div>
		);
	},
};
