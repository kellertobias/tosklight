import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { FixtureSheetTableView as FixtureSheetTable } from "@tosklight/ui/tables";
import { useState } from "react";
import {
	stageOptions,
	stageSelection,
} from "../../ui-library/storybook/fixtures/application";
import { CommandSectionFixture } from "../../ui-library/storybook/fixtures/controlSection";
import {
	dmxOutputHealth,
	dmxPatchedFixtures,
	dmxSnapshot,
} from "../../ui-library/storybook/fixtures/dmx";
import {
	marketingFixtureSheetPresentStep,
	marketingFixtureSheetRows,
	marketingFixtureSheetSelectedFixtureIds,
	marketingStage3dFixtures,
	marketingStageVisualization,
} from "../../ui-library/storybook/fixtures/marketingApplication";
import { ApplicationStateHarness } from "../../ui-library/storybook/providers/ApplicationStateHarness";
import { AppShellView } from "./components/shell/AppShell";
import { Clock } from "./components/shell/Clock";
import { LeftDock } from "./components/shell/LeftDock";
import { NativeDragStrip } from "./components/shell/NativeDragStrip";
import { SourceLegend } from "./components/shared/SourceLegend";
import { type Channel, ChannelsWindowView } from "./windows/ChannelsWindow";
import { MarketingCuesWindow } from "./windows/CuelistWindow.stories";
import { DmxWindowView } from "./windows/DmxWindow";
import { DEFAULT_FIXTURE_SHEET_COLUMNS } from "./windows/FixtureSheetSettings";
import { FixtureSheetWindowView } from "./windows/FixtureSheetWindow";
import { fixtureSheetColumns } from "./windows/fixtureSheetColumns";
import {
	MarketingColorPresetsWindow,
	MarketingGroupsWindow,
	MarketingPositionPresetsWindow,
} from "./windows/PoolWindows.stories";
import {
	MarketingFixtureLibraryWindow,
	MarketingSetupOutputsWindow,
	marketingOutputRoutes,
} from "./windows/SetupWindow.stories";
import { DEFAULT_STAGE_CAMERA_3D } from "./windows/Stage3dCanvas";
import { MarketingStage3DWindow } from "./windows/StageWindow.stories";
import { Stage3dView } from "./windows/stageWindow/Stage3dView";

const meta = {
	title: "Application/Marketing",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const fixtureSheetColumnsForMarketing = fixtureSheetColumns(
	true,
	marketingFixtureSheetPresentStep,
).filter((column) =>
	DEFAULT_FIXTURE_SHEET_COLUMNS.includes(
		column.id as (typeof DEFAULT_FIXTURE_SHEET_COLUMNS)[number],
	),
);

function MarketingViewport({ children }: { children: React.ReactNode }) {
	return (
		<div
			className="marketing-screenshot-viewport"
			style={{
				width: "100%",
				height: "100%",
				minWidth: 0,
				overflow: "hidden",
				border: "1px solid var(--line2)",
				background: "var(--panel)",
			}}
		>
			{children}
		</div>
	);
}

function MarketingFixtureSheet({ compact = false }: { compact?: boolean }) {
	const [activeRow, setActiveRow] = useState(2);
	return (
		<FixtureSheetWindowView
			compact={compact}
			selectionCount={marketingFixtureSheetSelectedFixtureIds.size}
			table={
				<FixtureSheetTable
					activeRow={activeRow}
					columns={fixtureSheetColumnsForMarketing}
					onActivate={() => undefined}
					onActiveRowChange={setActiveRow}
					presentStep={marketingFixtureSheetPresentStep}
					rows={marketingFixtureSheetRows}
					selectedFixtureIds={marketingFixtureSheetSelectedFixtureIds}
				/>
			}
		/>
	);
}

function FullApplicationComposition() {
	return (
		<ApplicationStateHarness>
			<MarketingViewport>
				<div style={{ width: "100%", height: "100%" }}>
					<AppShellView
						nativeDragStrip={<NativeDragStrip />}
						dock={
							<LeftDock
								presentation={{
									showIdentity: "Demo Show",
									showIndicator: {
										className: "show-status-warning",
										label: "Demo show",
										detail: "Deterministic marketing presentation.",
										connected: true,
									},
									clock: <Clock now={new Date(2026, 6, 26, 23, 59, 0)} />,
								}}
							/>
						}
						workspace={
							<div className="workspace-view">
								<GridDesktop id="marketing" name="Show">
									<PaneView
										pane={{
											id: "fixture-sheet",
											title: "Fixture Sheet",
											type: "fixtures",
											x: 1,
											y: 1,
											width: 24,
											height: 9,
										}}
										info={{
											primary: `${marketingFixtureSheetSelectedFixtureIds.size} selected`,
											secondary: <SourceLegend />,
										}}
										settings
										onSettings={() => undefined}
										onRectChange={() => undefined}
									>
										<MarketingFixtureSheet compact />
									</PaneView>
									<PaneView
										pane={{
											id: "stage",
											title: "Stage 3D",
											type: "stage",
											x: 1,
											y: 10,
											width: 12,
											height: 9,
										}}
										info={{
											primary: "0 selected",
											secondary: "Tap to select · Shift for range",
										}}
										actions={[
											[
												{
													id: "follow",
													label: "Follow Preload",
													active: true,
													onClick: () => undefined,
												},
											],
											[
												{
													id: "groups",
													label: "Groups",
													onClick: () => undefined,
												},
											],
										]}
										settings
										onSettings={() => undefined}
										onRectChange={() => undefined}
									>
										<Stage3dView
											camera3d={DEFAULT_STAGE_CAMERA_3D}
											fixtures={marketingStage3dFixtures}
											options={{
												...stageOptions,
												view: "3d",
												showSelection: false,
											}}
											patchPreviewFixtures={[]}
											patchSelectionPreview={false}
											selection={{ ...stageSelection, fixtureIds: [] }}
											visualization={marketingStageVisualization}
										/>
									</PaneView>
									<PaneView
										pane={{
											id: "groups",
											title: "Groups",
											type: "groups",
											x: 13,
											y: 10,
											width: 6,
											height: 9,
										}}
										settings
										onSettings={() => undefined}
										onRectChange={() => undefined}
									>
										<MarketingGroupsWindow showHeader={false} />
									</PaneView>
									<PaneView
										pane={{
											id: "color-presets",
											title: "Color Presets",
											type: "presets",
											x: 19,
											y: 10,
											width: 6,
											height: 9,
										}}
										settings
										onSettings={() => undefined}
										onRectChange={() => undefined}
									>
										<MarketingColorPresetsWindow showHeader={false} />
									</PaneView>
								</GridDesktop>
							</div>
						}
						control={
							<CommandSectionFixture initialMode="playbacks" hardware={false} />
						}
					/>
				</div>
			</MarketingViewport>
		</ApplicationStateHarness>
	);
}

const channels = Array.from({ length: 14 }, (_, index) => ({
	number: index + 1,
	fixture: {
		fixture_id: `fixture-${index + 1}`,
		fixture_number: index + 1,
	} as Channel["fixture"],
	name:
		index < 4
			? "Front Profile"
			: index < 10
				? "Moving Wash"
				: "Audience Blinder",
	level: [72, 68, 54, 54, 86, 78, 56, 56, 42, 38, 24, 24, 0, 100][index],
})) satisfies Channel[];

function MarketingChannelsWindow() {
	const [page, setPage] = useState(0);
	const [picker, setPicker] = useState(false);
	const [columns, setColumns] = useState(6);
	const [selected, setSelected] = useState<ReadonlySet<string>>(
		new Set(["fixture-6"]),
	);
	return (
		<MarketingViewport>
			<ChannelsWindowView
				channels={channels}
				columns={columns}
				page={page}
				pages={8}
				pagePickerOpen={picker}
				selectedFixtureIds={selected}
				valuesReady
				onPage={setPage}
				onPagePickerOpen={setPicker}
				onColumnsChange={setColumns}
				onSelect={(fixtureId) => setSelected(new Set([fixtureId]))}
				onSetIntensity={() => undefined}
			/>
		</MarketingViewport>
	);
}

function MarketingDmxWindow() {
	return (
		<MarketingViewport>
			<DmxWindowView
				defaultSelection={{ universe: 1, address: 13 }}
				dotSize="small"
				onDotSizeChange={() => undefined}
				onSetDmxOverride={() => undefined}
				outputHealth={dmxOutputHealth}
				outputRoutes={marketingOutputRoutes}
				patchedFixtures={dmxPatchedFixtures}
				snapshot={dmxSnapshot}
			/>
		</MarketingViewport>
	);
}

export const FullApplication: Story = {
	render: () => <FullApplicationComposition />,
};

export const GroupsWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingGroupsWindow />
		</MarketingViewport>
	),
};

export const ColorPresetsWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingColorPresetsWindow />
		</MarketingViewport>
	),
};

export const PositionPresetsWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingPositionPresetsWindow />
		</MarketingViewport>
	),
};

export const CuesWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingCuesWindow />
		</MarketingViewport>
	),
};

export const Stage3DWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingStage3DWindow />
		</MarketingViewport>
	),
};

export const FixtureLibraryWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingFixtureLibraryWindow />
		</MarketingViewport>
	),
};

export const DeskSetupOutputsWindow: Story = {
	render: () => (
		<MarketingViewport>
			<MarketingSetupOutputsWindow />
		</MarketingViewport>
	),
};

export const ChannelsWindow: Story = {
	render: () => (
		<ApplicationStateHarness>
			<MarketingChannelsWindow />
		</ApplicationStateHarness>
	),
};

export const DmxWindow: Story = {
	render: () => (
		<ApplicationStateHarness>
			<MarketingDmxWindow />
		</ApplicationStateHarness>
	),
};
