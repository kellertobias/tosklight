import type { Meta, StoryObj } from "@storybook/react-vite";
import { FormLayout, NumberField, TextField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Form Layout",
	component: FormLayout,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof FormLayout>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => (
		<div className="forms-story-canvas">
			<FormLayout columns={2} labelPlacement="top">
				<TextField label="Route name" defaultValue="Front truss" />
				<NumberField label="Universe" defaultValue="1" />
				<TextField label="Description" defaultValue="Primary Art-Net route" />
				<NumberField label="Priority" defaultValue="100" />
			</FormLayout>
			<FormLayout columns={1} labelPlacement="side" labelWidth={140}>
				<TextField label="Side label" defaultValue="Aligned control" />
			</FormLayout>
		</div>
	),
};
