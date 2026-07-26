import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import {
	stage3dFixtures,
	stageLayout,
	stageOptions,
	stagePresentations,
	stageSelection,
	stageVisualization,
} from "../../ui-library/storybook/fixtures/application";
import {
	dmxOutputHealth,
	dmxPatchedFixtures,
	dmxSnapshot,
} from "../../ui-library/storybook/fixtures/dmx";
import { ApplicationStateHarness } from "../../ui-library/storybook/providers/ApplicationStateHarness";
import { ControlSectionView } from "./components/control/ControlSection";
import { CommandInputView } from "./components/control/commandLine/CommandInput";
import { NumericPad } from "./components/control/NumericPad";
import { AppShellView } from "./components/shell/AppShell";
import { LeftDock } from "./components/shell/LeftDock";
import { DemoPlaybackControlsView } from "./features/productDemo/DemoPlaybackControls";
import type { DemoPlaybackControlsValue } from "./features/productDemo/useDemoPlaybackControls";
import {
	DemoApplicationScreenView,
	DemoDmxGridView,
	ProductDemoSurfaceView,
} from "./ProductDemoApp";
import { DmxWindowView } from "./windows/DmxWindow";
import { DEFAULT_STAGE_CAMERA_3D } from "./windows/Stage3dCanvas";
import { Stage2dView } from "./windows/stageWindow/Stage2dView";
import { Stage3dView } from "./windows/stageWindow/Stage3dView";

const meta = {
	title: "Application/Marketing",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const playbackLevels = new Map([
	[1, 0.72],
	[2, 0.48],
	[3, 1],
	[4, 0.24],
	[21, 1],
	[22, 1],
	[23, 1],
	[24, 1],
]);

const playbackControls: DemoPlaybackControlsValue = {
	ready: true,
	status: { kind: "ready" },
	playbackNumber: (slot) => (playbackLevels.has(slot) ? slot : null),
	faderLevel: (slot) => playbackLevels.get(slot) ?? null,
	press: () => undefined,
	release: () => undefined,
	setMaster: () => undefined,
};

function MarketingShell() {
	return (
		<AppShellView
			dock={<LeftDock />}
			workspace={
				<GridDesktop id="marketing-story" name="Product Demo">
					<PaneView
						pane={{
							id: "stage",
							title: "Stage",
							type: "stage",
							x: 1,
							y: 1,
							width: 14,
							height: 18,
						}}
					>
						<Stage2dView
							fixtures={stagePresentations}
							layout={stageLayout}
							options={stageOptions}
							selection={stageSelection}
						/>
					</PaneView>
					<PaneView
						pane={{
							id: "dmx",
							title: "DMX",
							type: "dmx",
							x: 15,
							y: 1,
							width: 10,
							height: 18,
						}}
					>
						<DmxWindowView
							compact
							dotSize="small"
							onDotSizeChange={() => undefined}
							onSetDmxOverride={() => undefined}
							outputHealth={dmxOutputHealth}
							outputRoutes={[]}
							patchedFixtures={dmxPatchedFixtures}
							snapshot={dmxSnapshot}
						/>
					</PaneView>
				</GridDesktop>
			}
			control={
				<ControlSectionView
					mode="programmer"
					hardware
					commandLine={
						<CommandInputView
							commandError={null}
							commandLine="FIXTURE 1 THRU 5 AT 72"
							commandTarget="FIXTURE"
							completed={false}
							hardware
							onExecute={async () => undefined}
							onOpenHistory={() => undefined}
							onReplace={() => undefined}
							onToggleMode={() => undefined}
							playback={false}
							preloadArmed={false}
							status={null}
						/>
					}
					left={<div className="control-left-pane" />}
					right={
						<aside className="control-right-pane hardware-right-pane">
							<div className="control-right-main">
								<NumericPad demo />
							</div>
						</aside>
					}
				/>
			}
		/>
	);
}

const demoDmx = [1, 2, 3, 4].map((universeNumber) => {
	const slots = Array.from({ length: 512 }, (_, index) =>
		index < 32 ? (index * 29 + universeNumber * 47) % 256 : 0,
	);
	return (
		<DemoDmxGridView
			key={universeNumber}
			universeNumber={universeNumber}
			slots={slots}
		/>
	);
});

export const CompleteProductDemo: Story = {
	render: () => (
		<ApplicationStateHarness
			actions={[{ type: "SET_MIDI_PROFILE", value: true }]}
		>
			<ProductDemoSurfaceView
				application={
					<DemoApplicationScreenView>
						<MarketingShell />
					</DemoApplicationScreenView>
				}
				stage={
					<Stage3dView
						camera3d={DEFAULT_STAGE_CAMERA_3D}
						fixtures={stage3dFixtures}
						options={{ ...stageOptions, view: "3d", showSelection: false }}
						patchPreviewFixtures={[]}
						patchSelectionPreview={false}
						selection={{ ...stageSelection, fixtureIds: [] }}
						visualization={stageVisualization}
					/>
				}
				dmx={demoDmx}
				playbackControls={
					<DemoPlaybackControlsView controls={playbackControls} />
				}
				programmer={<NumericPad demo />}
			/>
		</ApplicationStateHarness>
	),
};
