import { StageRendererView } from "./stageWindow/StageRendererView";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
	stage3dFixtures,
	stageLayout,
	stageOptions,
	stagePresentations,
	stageSelection,
	stageVisualization,
} from "../../../ui-library/storybook/fixtures/application";
import {
	marketingStage3dFixtures,
	marketingStageVisualization,
} from "../../../ui-library/storybook/fixtures/marketingApplication";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { StageHeader } from "./stageWindow/StageHeader";

const meta = {
	title: "ToskLight/Windows/Stage",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	excludeStories: /^(Marketing|marketing)/,
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Stage2D: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div className="stage-window" style={{ width: 1496, height: 761 }}>
				<StageRendererView options={stageOptions} selection={stageSelection} />
			</div>
		</ApplicationStateHarness>
	),
};

export const Stage2DCompact: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div className="stage-window compact" style={{ width: 720, height: 460 }}>
				<StageRendererView options={stageOptions} selection={stageSelection} />
			</div>
		</ApplicationStateHarness>
	),
};

export const Stage3D: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div className="stage-window" style={{ width: 1496, height: 761 }}>
				<StageRendererView options={{ ...stageOptions, view: "3d" }} selection={stageSelection} />
			</div>
		</ApplicationStateHarness>
	),
};

export function MarketingStage3DWindow() {
	const [followPreload, setFollowPreload] = useState(true);
	const options = {
		...stageOptions,
		view: "3d" as const,
		groupsVisible: false,
		followPreload,
		toggleFollowPreload: () => setFollowPreload((current) => !current),
	};
	return (
		<ApplicationStateHarness>
			<div
				className="ui-window stage-window"
				style={{ width: "100%", height: "100%" }}
			>
				<StageHeader
					options={options}
					selectedCount={stageSelection.fixtureIds.length}
				/>
				<StageRendererView options={{ ...stageOptions, view: "3d" }} selection={stageSelection} />
			</div>
		</ApplicationStateHarness>
	);
}

export const Stage3DMarketingWindow: Story = {
	render: () => <MarketingStage3DWindow />,
};
