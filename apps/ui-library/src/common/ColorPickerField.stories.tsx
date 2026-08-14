import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ColorPickerField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Color Picker Field",
	component: ColorPickerField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof ColorPickerField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("#1bd6ec");
		return (
			<div className="forms-story-canvas">
				<ColorPickerField
					label="Pool color"
					value={value}
					onChange={setValue}
				/>
			</div>
		);
	},
};
