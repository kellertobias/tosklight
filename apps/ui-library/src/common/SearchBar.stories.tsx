import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SearchBar } from "./SearchBar";

const meta = {
	title: "ToskLight/Controls/Search Bar",
	component: SearchBar,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof SearchBar>;
export default meta;
type Story = StoryObj;
export const Primary: Story = {
	render: () => {
		const [value, setValue] = useState("wash");
		const [type, setType] = useState("");
		return (
			<div className="forms-story-canvas">
				<SearchBar value={value} onChange={setValue} />
				<SearchBar
					value={value}
					onChange={setValue}
					settings={[
						{
							kind: "select",
							id: "type",
							label: "Fixture type",
							value: type,
							options: [
								{ value: "", label: "All" },
								{ value: "Dimmer", label: "Dimmer" },
							],
						},
					]}
					onSettingChange={(_, next) => setType(String(next))}
					onClearSettings={() => setType("")}
				/>
			</div>
		);
	},
};
