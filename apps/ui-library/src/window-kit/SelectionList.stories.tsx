import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SelectionList } from "../window-kit";

const meta = {
	title: "ToskLight/Window System/Selection List",
	component: SelectionList,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof SelectionList>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("stage");
		return (
			<div
				style={{
					display: "grid",
					gridTemplateColumns: "1fr 1fr",
					gap: 16,
					height: 320,
				}}
			>
				<SelectionList
					ariaLabel="Available windows"
					value={value}
					options={[
						{
							value: "stage",
							label: "Stage",
							description: "2D and 3D fixture view",
						},
						{
							value: "fixtures",
							label: "Fixture Sheet",
							description: "Fixture values",
						},
						{ value: "delete", label: "Delete", tone: "danger" },
						{ value: "offline", label: "Unavailable", disabled: true },
					]}
					onChange={setValue}
				/>
				<SelectionList
					ariaLabel="Empty options"
					options={[]}
					emptyLabel="No options are available"
					onChange={() => undefined}
				/>
			</div>
		);
	},
};
