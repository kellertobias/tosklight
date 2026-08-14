import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { FormField, Select, SelectField } from "../controls";

const meta = {
	title: "ToskLight/Controls/Select",
	component: Select,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen", docs: { source: { type: "dynamic" } } },
} satisfies Meta<typeof Select>;

export default meta;
type Story = StoryObj;

function SelectCatalog() {
	const [value, setValue] = useState("artnet");
	return (
		<div className="forms-story-canvas">
			<SelectField
				label="Output mode"
				value={value}
				options={[
					{ value: "artnet", label: "Art-Net" },
					{ value: "sacn", label: "sACN" },
					{ value: "usb", label: "USB unavailable", disabled: true },
				]}
				onChange={setValue}
			/>
			<FormField label="Output protocol">
				<Select
					aria-label="Output protocol"
					value={value}
					onChange={(event) => setValue(event.target.value)}
				>
					<option value="artnet">Art-Net</option>
					<option value="sacn">sACN</option>
					<option value="usb" disabled>
						USB unavailable
					</option>
				</Select>
			</FormField>
			<FormField label="Disabled protocol">
				<Select aria-label="Disabled protocol" value="artnet" disabled>
					<option value="artnet">Art-Net</option>
				</Select>
			</FormField>
		</div>
	);
}

export const Primary: Story = { render: () => <SelectCatalog /> };
