import type { Meta, StoryObj } from "@storybook/react-vite";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { DevelopmentWindow } from "./DevelopmentWindow";

const meta = {
	title: "Application/Windows/Development",
	component: DevelopmentWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	decorators: [
		(Story) => (
			<ApplicationStateHarness>
				<div style={{ height: 680, minWidth: 720 }}>
					<Story />
				</div>
			</ApplicationStateHarness>
		),
	],
} satisfies Meta<typeof DevelopmentWindow>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Forms: Story = {
	args: { developmentView: "forms" },
};

export const Faders: Story = {
	args: { developmentView: "faders" },
};

export const Buttons: Story = {
	args: { developmentView: "buttons" },
};

export const CompactForms: Story = {
	args: { compact: true, developmentView: "forms" },
};
