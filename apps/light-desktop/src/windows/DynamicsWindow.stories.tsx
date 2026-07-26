import type { Meta, StoryObj } from "@storybook/react-vite";
import { DynamicsWindow } from "./DynamicsWindow";

const meta = {
	title: "Application/Windows/Dynamics",
	component: DynamicsWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	args: { active: true },
} satisfies Meta<typeof DynamicsWindow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const FutureFeatureEmptyState: Story = {};
