import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import { SelectionTree } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Selection Tree",
	component: SelectionTree,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof SelectionTree>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [kind, setKind] = useState("cue");
		const [target, setTarget] = useState("main");
		return (
			<div style={{ height: 420 }}>
				<SelectionTree
					columns={[
						{
							id: "kind",
							title: "Function",
							ariaLabel: "Functions",
							value: kind,
							options: [
								{ value: "cue", label: "Cue List" },
								{ value: "speed", label: "Speed Group" },
							],
							onChange: setKind,
						},
						{
							id: "target",
							title: "Target",
							ariaLabel: "Targets",
							value: target,
							options: [
								{ value: "main", label: "Main", description: "6 cues" },
								{ value: "encore", label: "Encore", description: "3 cues" },
							],
							onChange: setTarget,
							footer: <Button fullWidth>Clear assignment</Button>,
						},
					]}
				/>
			</div>
		);
	},
};
