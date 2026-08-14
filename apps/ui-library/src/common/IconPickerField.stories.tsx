import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { IconPickerField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Icon Picker Field",
	component: IconPickerField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof IconPickerField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("◇");
		return (
			<div className="forms-story-canvas">
				<IconPickerField
					label="Pool icon"
					value={value}
					defaultGroup="gobo"
					onChange={setValue}
				/>
			</div>
		);
	},
};
