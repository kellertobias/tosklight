import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FileDropField, FormLayout } from "../controls";

const meta = {
	title: "ToskLight/Controls/File Drop Field",
	component: FileDropField,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof FileDropField>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [selected, setSelected] = useState("No file selected");
		const common = {
			label: "Fixture profile",
			constraints: { extensions: [".gdtf"] },
			onFiles: () => undefined,
			onOpenPicker: () => undefined,
		};
		return (
			<div className="forms-story-canvas">
				<FormLayout columns={2}>
					<FileDropField
						{...common}
						selectedLabel={selected}
						onFiles={(files) =>
							setSelected(files[0]?.name ?? "No file selected")
						}
					/>
					<FileDropField
						{...common}
						status="loading"
						statusMessage="Loading selected file…"
					/>
					<FileDropField
						{...common}
						status="success"
						statusMessage="touring-profile.gdtf"
					/>
					<FileDropField
						{...common}
						status="error"
						statusMessage="The archive could not be read."
					/>
				</FormLayout>
			</div>
		);
	},
};
