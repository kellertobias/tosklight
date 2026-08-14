import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { TouchSelect } from "./TouchSelect";

const meta = {
	title: "ToskLight/Controls/Touch Select",
	component: TouchSelect,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof TouchSelect>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState(1);
		return (
			<div className="forms-story-canvas">
				<TouchSelect
					label="Universe"
					value={value}
					options={[1, 2, 3, 4]}
					onChange={setValue}
				/>
			</div>
		);
	},
};
