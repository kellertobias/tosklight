import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { TextAreaField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Multiline Text Input",
	component: TextAreaField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof TextAreaField>;

export default meta;
type Story = StoryObj;

function MultilineCatalog() {
	const [value, setValue] = useState(
		"House opens at 18:30.\nKeep the lectern special isolated.",
	);
	return (
		<div className="forms-story-canvas">
			<TextAreaField
				label="Operator notes"
				description="Visible in the show handover"
				value={value}
				onValueChange={setValue}
			/>
			<TextAreaField
				label="Required notes"
				required
				error="Add the safety note"
			/>
			<TextAreaField label="Archived note" value="Read only" disabled />
		</div>
	);
}

export const Primary: Story = {
	render: () => <MultilineCatalog />,
};
