import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  stage3dFixtures,
  stageLayout,
  stageOptions,
  stagePresentations,
  stageSelection,
  stageVisualization,
} from "../../../ui-library/storybook/fixtures/application";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { DEFAULT_STAGE_CAMERA_3D } from "./Stage3dCanvas";
import { Stage2dView } from "./stageWindow/Stage2dView";
import { Stage3dView } from "./stageWindow/Stage3dView";

const meta = {
  title: "Application/Windows/Stage",
  tags: ["autodocs"],
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Stage2D: Story = {
  render: () => <ApplicationStateHarness>
    <div className="stage-window" style={{ width: 1496, height: 761 }}>
      <Stage2dView
        fixtures={stagePresentations}
        layout={stageLayout}
        options={stageOptions}
        selection={stageSelection}
      />
    </div>
  </ApplicationStateHarness>,
};

export const Stage2DCompact: Story = {
  render: () => <ApplicationStateHarness>
    <div className="stage-window compact" style={{ width: 720, height: 460 }}>
      <Stage2dView
        compact
        fixtures={stagePresentations}
        layout={stageLayout}
        options={stageOptions}
        selection={stageSelection}
      />
    </div>
  </ApplicationStateHarness>,
};

export const Stage3D: Story = {
  render: () => <ApplicationStateHarness>
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
  </ApplicationStateHarness>,
};
