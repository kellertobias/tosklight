import type { Meta, StoryObj } from "@storybook/react-vite";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { PatchViewProvider } from "../features/patch/PatchContext";
import { PatchWindow } from "./PatchWindow";

const meta = {
	title: "Application/Windows/Patch",
	component: PatchWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	decorators: [
		(Story) => (
			<ApplicationStateHarness>
				<PatchViewProvider
					showId={null}
					initialFixtures={[]}
					definitions={[]}
					transport={null}
				>
					<div style={{ height: 680, minWidth: 920 }}>
						<Story />
					</div>
				</PatchViewProvider>
			</ApplicationStateHarness>
		),
	],
} satisfies Meta<typeof PatchWindow>;
export default meta;
type Story = StoryObj<typeof meta>;

export const EmptyPatch: Story = {
	args: { active: true },
};
