import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { GroupedSelectionField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Grouped Selection Field",
	component: GroupedSelectionField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof GroupedSelectionField>;
export default meta;
type Story = StoryObj;

export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("go");
		return (
			<div className="forms-story-canvas">
				<GroupedSelectionField
					label="Playback button"
					value={value}
					groups={[
						{
							label: "Step Control",
							options: [
								{
									value: "go",
									label: "GO",
									description: "Advance to the next cue.",
								},
								{
									value: "back",
									label: "GO MINUS",
									description: "Return to the previous cue.",
								},
							],
						},
						{
							label: "Temporary State",
							options: [
								{
									value: "flash",
									label: "FLASH",
									description: "Output while held.",
								},
							],
						},
					]}
					onChange={setValue}
					clearAction={{ label: "Empty Button", value: "none" }}
				/>
			</div>
		);
	},
};
