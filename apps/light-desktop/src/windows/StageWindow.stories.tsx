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
import { DEFAULT_STAGE_CAMERA_3D } from "./Stage3dCanvas";
import { Stage2dView } from "./stageWindow/Stage2dView";
import { Stage3dView } from "./stageWindow/Stage3dView";
import { StageHeader } from "./stageWindow/StageHeader";

const meta = {
	title: "Application/Windows/Stage",
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
				<Stage2dView
					fixtures={stagePresentations}
					layout={stageLayout}
					options={stageOptions}
					selection={stageSelection}
				/>
			</div>
		</ApplicationStateHarness>
	),
};

export const Stage2DCompact: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div className="stage-window compact" style={{ width: 720, height: 460 }}>
				<Stage2dView
					compact
					fixtures={stagePresentations}
					layout={stageLayout}
					options={stageOptions}
					selection={stageSelection}
				/>
			</div>
		</ApplicationStateHarness>
	),
};

export const Stage3D: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div className="stage-window" style={{ width: 1496, height: 761 }}>
				<Stage3dView
					camera3d={DEFAULT_STAGE_CAMERA_3D}
					fixtures={stage3dFixtures}
					options={{ ...stageOptions, view: "3d" }}
					patchPreviewFixtures={[]}
					patchSelectionPreview={false}
					selection={stageSelection}
					visualization={stageVisualization}
				/>
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
				<Stage3dView
					camera3d={DEFAULT_STAGE_CAMERA_3D}
					fixtures={marketingStage3dFixtures}
					options={options}
					patchPreviewFixtures={[]}
					patchSelectionPreview={false}
					selection={stageSelection}
					visualization={marketingStageVisualization}
				/>
			</div>
		</ApplicationStateHarness>
	);
}

export const Stage3DMarketingWindow: Story = {
	render: () => <MarketingStage3DWindow />,
};
