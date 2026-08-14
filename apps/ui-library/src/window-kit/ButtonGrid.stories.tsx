import type { Meta, StoryObj } from "@storybook/react-vite";
import { ButtonGrid, GridButton } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Button Grid",
	component: ButtonGrid,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof ButtonGrid>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => (
		<div style={{ width: 600 }}>
			<ButtonGrid>
				<GridButton
					number="1"
					primary="All"
					secondary="12 fixtures"
					state="active"
				/>
				<GridButton number="2" primary="Front Wash" secondary="4 fixtures" />
				<GridButton
					number="2.1"
					primary="Selected"
					secondary="Logical heads"
					state="selected"
				/>
				<GridButton number="3" primary="Empty" state="empty" />
				<GridButton number="4" primary="Disabled" state="disabled" />
				<GridButton number="5" primary="Store here" state="store-target" />
			</ButtonGrid>
		</div>
	),
};
