import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { CyclingValueToggleField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Cycling Value Toggle",
	component: CyclingValueToggleField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof CyclingValueToggleField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("keyframes");
		return (
			<div className="forms-story-canvas">
				<CyclingValueToggleField
					label="Curve method"
					ariaLabel="Curve method"
					value={value}
					options={[
						{ value: "keyframes", label: "Keyframes" },
						{ value: "max-min", label: "Max / min" },
						{ value: "middle", label: "Middle / amplitude" },
					]}
					onChange={setValue}
				/>
			</div>
		);
	},
};
