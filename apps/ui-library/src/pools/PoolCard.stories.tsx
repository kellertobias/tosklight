import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ButtonGrid } from "../grids";
import { PoolCard, type PoolCardViewModel } from "../pools";

interface PoolStoryProps {
	width: number;
	minimum: number;
	holdDelay: number;
}

const meta: Meta<PoolStoryProps> = {
	title: "Pools/Production pool cards",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		width: { control: { type: "range", min: 280, max: 1000, step: 20 } },
		minimum: { control: { type: "range", min: 88, max: 180, step: 2 } },
		holdDelay: { control: { type: "range", min: 300, max: 1000, step: 50 } },
	},
	args: { width: 720, minimum: 132, holdDelay: 650 },
};

export default meta;
type Story = StoryObj<PoolStoryProps>;

const models: PoolCardViewModel[] = [
	{ number: 1, primary: "All", secondary: "12 fixtures · ordered", kind: "group", states: ["selected"], icon: "◇", color: "#1bd6ec" },
	{ number: 2, primary: "Front Wash", secondary: "4 fixtures · ordered", details: ["1 portable attribute"], kind: "group", states: ["active"], derived: true },
	{ number: 3, primary: "Frozen", secondary: "Revision 8", kind: "group", frozen: true },
	{ number: 4, primary: "Blue", secondary: "Color preset", kind: "preset", color: "#264fd4" },
	{ number: 5, primary: "Main", secondary: "Cuelist · 62%", details: ["Playbacks on pages 1, 2"], kind: "cuelist", states: ["active"] },
	{ number: 6, primary: "Empty", secondary: "Press Record to use this slot", kind: "group", states: ["empty"] },
	{ number: 7, primary: "Store here", kind: "preset", states: ["empty", "store-target"] },
	{ number: 8, primary: "Update", kind: "cuelist", states: ["update-target"] },
	{ number: 9, primary: "Set target", kind: "cuelist", states: ["set-target"] },
	{ number: 10, primary: "Disabled", secondary: "Other family", kind: "preset", states: ["disabled"] },
];

function PoolGridExample({ width, minimum, holdDelay }: PoolStoryProps) {
	const [event, setEvent] = useState("Tap a card or hold one for its context action");
	return (
		<div style={{ width }}>
			<output aria-live="polite" style={{ display: "block", minHeight: 28 }}>
				{event}
			</output>
			<ButtonGrid className="card-pool" minimum={minimum}>
				{models.map((model) => (
					<PoolCard
						key={String(model.number)}
						model={model}
						holdDelay={holdDelay}
						onClick={() => setEvent(`Clicked ${model.primary}`)}
						onPressHold={() => setEvent(`Held ${model.primary}`)}
					/>
				))}
			</ButtonGrid>
		</div>
	);
}

export const ScalingAndEveryState: Story = {
	render: (args) => <PoolGridExample {...args} />,
};

export const NarrowScaling: Story = {
	args: { width: 340, minimum: 88 },
	render: (args) => <PoolGridExample {...args} />,
};
