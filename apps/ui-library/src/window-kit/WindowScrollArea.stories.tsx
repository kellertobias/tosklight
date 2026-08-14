import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "../controls";
import { WindowScrollArea } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Window Scroll Area",
	component: WindowScrollArea,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof WindowScrollArea>;

export default meta;
type Story = StoryObj;

export const PopulatedAndEmpty: Story = {
	render: () => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "1fr 1fr",
				gap: 16,
				height: 360,
			}}
		>
			<WindowScrollArea>
				<div style={{ display: "grid", gap: 8, padding: 8 }}>
					{Array.from({ length: 18 }, (_, index) => (
						<Button key={index}>Cue {index + 1}</Button>
					))}
				</div>
			</WindowScrollArea>
			<WindowScrollArea
				emptyState={{
					title: "No cues",
					description: "Record the first cue to begin.",
					icon: "◇",
					action: <Button variant="primary">Record cue</Button>,
				}}
			/>
		</div>
	),
};
