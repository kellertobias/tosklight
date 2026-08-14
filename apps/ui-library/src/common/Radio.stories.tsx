import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { RadioField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Radio",
	component: RadioField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof RadioField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("2d");
		return (
			<div
				className="forms-story-canvas"
				role="radiogroup"
				aria-label="Stage view"
			>
				<RadioField
					label="2D"
					name="stage-view"
					checked={value === "2d"}
					onChange={() => setValue("2d")}
				/>
				<RadioField
					label="3D"
					name="stage-view"
					checked={value === "3d"}
					onChange={() => setValue("3d")}
				/>
				<RadioField label="Unavailable" name="stage-view" disabled />
			</div>
		);
	},
};
