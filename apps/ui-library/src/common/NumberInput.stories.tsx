import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { NumberField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Number Input",
	component: NumberField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof NumberField>;

export default meta;
type Story = StoryObj;

function NumberInputCatalog() {
	const [level, setLevel] = useState("62.5");
	const [universe, setUniverse] = useState("1");
	return (
		<div className="forms-story-canvas">
			<NumberField
				label="Level"
				value={level}
				allowDecimal
				min={0}
				max={100}
				unit="%"
				modalFader={{ maximum: 100, step: 0.1, accentColor: "#1bd6ec" }}
				onValueChange={setLevel}
			/>
			<NumberField
				label="Universe"
				value={universe}
				min={1}
				max={63999}
				onValueChange={setUniverse}
			/>
			<NumberField label="Fixed address" value="512" disabled />
		</div>
	);
}

export const Primary: Story = {
	render: () => <NumberInputCatalog />,
};
