import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { CheckboxField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Checkbox",
	component: CheckboxField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof CheckboxField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [checked, setChecked] = useState(true);
		return (
			<div className="forms-story-canvas">
				<CheckboxField
					label="Desktop lock"
					stateLabel="Prevent layout changes"
					checked={checked}
					onChange={(event) => setChecked(event.target.checked)}
				/>
				<CheckboxField
					label="Unavailable option"
					stateLabel="Unavailable"
					disabled
				/>
			</div>
		);
	},
};
