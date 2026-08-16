import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SwitchField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Switch",
	component: SwitchField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof SwitchField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [enabled, setEnabled] = useState(true);
		return (
			<div className="forms-story-canvas">
				<SwitchField
					label="Output route"
					offLabel="Disabled"
					onLabel="Enabled"
					checked={enabled}
					onChange={(event) => setEnabled(event.target.checked)}
				/>
				<SwitchField
					label="Unavailable route"
					offLabel="Disabled"
					onLabel="Enabled"
					disabled
				/>
			</div>
		);
	},
};
