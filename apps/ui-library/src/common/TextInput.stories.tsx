import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { TextField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Text Input",
	component: TextField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof TextField>;

export default meta;
type Story = StoryObj;

function TextInputCatalog() {
	const [value, setValue] = useState("Front wash");
	return (
		<div className="forms-story-canvas">
			<TextField
				label="Fixture name"
				description="The name shown to operators"
				value={value}
				clearable
				onValueChange={setValue}
			/>
			<TextField label="Required name" required error="Enter a name" />
			<TextField label="Read-only identity" value="Fixture 101" readOnly />
			<TextField label="Unavailable input" value="Offline" disabled />
		</div>
	);
}

export const Primary: Story = {
	render: () => <TextInputCatalog />,
};
