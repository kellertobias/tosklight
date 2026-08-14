import type { Meta, StoryObj } from "@storybook/react-vite";
import { WindowSettings } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Window Settings",
	component: WindowSettings,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof WindowSettings>;

export default meta;
type Story = StoryObj;

const tabs = [
	{
		id: "pane",
		label: "Pane",
		content: <p>Size, placement, and visibility.</p>,
	},
	{
		id: "content",
		label: "Content",
		content: <p>Window-specific presentation.</p>,
	},
];

export const ModalAndAnchored: Story = {
	render: () => (
		<div
			style={{
				minHeight: 520,
				display: "grid",
				gridTemplateColumns: "1fr 1fr",
				gap: 24,
			}}
		>
			<div>
				<WindowSettings
					title="Pane Settings"
					tabs={tabs}
					onClose={() => undefined}
				/>
			</div>
			<div>
				<WindowSettings
					modal={false}
					anchor={new DOMRect(720, 80, 120, 38)}
					title="Stage Settings"
					tabs={tabs}
					onClose={() => undefined}
				/>
			</div>
		</div>
	),
};
