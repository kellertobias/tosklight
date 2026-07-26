import type { Meta, StoryObj } from "@storybook/react-vite";
import { FixtureSheetTableView as FixtureSheetTable } from "@tosklight/ui/tables";
import { useMemo, useState } from "react";
import { fixtureTypeIconAsset } from "../components/setup/fixtureTypeIconAssets";
import { SourceLegend } from "../components/shared/SourceLegend";
import { DEFAULT_FIXTURE_SHEET_COLUMNS } from "./FixtureSheetSettings";
import { FixtureSheetWindowView } from "./FixtureSheetWindow";
import { fixtureSheetColumns } from "./fixtureSheetColumns";
import type { FixtureSheetRow } from "./fixtureSheetProjection";
import type { FixtureStepPresenter } from "./fixtureSheetStep";

const profileIcon = fixtureTypeIconAsset("profile dimmer lamp");
const washIcon = fixtureTypeIconAsset("led wash moving light");

const common = {
	beam: "Open",
	childFixtureIds: [] as string[],
	color: "#ffffff",
	colorLabel: "Open White",
	dimmer: 0,
	focus: "Sharp",
	indented: false,
	limitingGroups: [],
	parentFixtureId: "",
	pan: 50,
	patch: "U1.1",
	positionLabel: "Center",
	preloadColor: null,
	preloadDimmer: null,
	preloadPan: null,
	preloadTilt: null,
	sources: {
		beam: "default" as const,
		color: "default" as const,
		dimmer: "default" as const,
		focus: "default" as const,
		position: "default" as const,
	},
	targetKind: "fixture" as const,
	tilt: 50,
	type: "Fixture",
};

const rows: FixtureSheetRow[] = [
	{
		...common,
		id: "101",
		fixtureId: "fixture-101",
		name: "Front Profile SL",
		fixtureType: "ETC · Source Four LED Series 3",
		icon: profileIcon,
		patch: "U1.1",
		dimmer: 72,
		color: "#f6c985",
		colorLabel: "Warm White",
		pan: 42,
		tilt: 38,
		positionLabel: "Lectern",
		beam: "Open",
		focus: "Sharp",
		sources: {
			...common.sources,
			dimmer: "programmer",
			color: "programmer",
			position: "programmer",
		},
	},
	{
		...common,
		id: "102.0",
		fixtureId: "fixture-102",
		name: "Stage Left Mover · Master",
		fixtureType: "Robe · Tetra2",
		icon: washIcon,
		patch: "U1.101",
		targetKind: "master",
		parentFixtureId: "fixture-102",
		childFixtureIds: ["fixture-102.1", "fixture-102.2"],
		dimmer: 48,
		color: "#1bd6ec",
		colorLabel: "Cyan",
		pan: 68,
		tilt: 27,
		positionLabel: "Downstage fan",
		beam: "Wash",
		focus: "Medium",
		sources: {
			...common.sources,
			dimmer: "default",
			color: "default",
			position: "default",
			beam: "playback",
			focus: "playback",
		},
	},
	{
		...common,
		id: "102.1",
		fixtureId: "fixture-102.1",
		name: "Stage Left Mover · Cell 1",
		fixtureType: "Robe · Tetra2 · Cell 1",
		icon: washIcon,
		patch: "U1.101",
		targetKind: "head",
		parentFixtureId: "fixture-102",
		indented: true,
		dimmer: 64,
		color: "#e24bdb",
		colorLabel: "Magenta",
		pan: 68,
		tilt: 27,
		positionLabel: "Downstage fan",
		beam: "Wash",
		focus: "Medium",
		preloadDimmer: 80,
		preloadColor: "#3f8cff",
		preloadPan: 72,
		preloadTilt: 30,
		sources: {
			...common.sources,
			dimmer: "programmer",
			color: "programmer",
			position: "default",
			beam: "playback",
			focus: "playback",
		},
	},
	{
		...common,
		id: "103",
		fixtureId: "fixture-103",
		name: "Back Wash",
		fixtureType: "Astera · AX9",
		icon: washIcon,
		patch: "U1.161",
		colorLabel: "White",
		positionLabel: "—",
		beam: "Wide",
		focus: "—",
	},
];

const presentStep: FixtureStepPresenter = (row) => ({
	base: row.fixtureId === "fixture-101",
	containedBase: false,
	current: row.fixtureId === "fixture-102.1",
	containedCurrent: row.fixtureId === "fixture-102",
});

function FixtureSheetStory({ compact = false }: { compact?: boolean }) {
	const [activeRow, setActiveRow] = useState(1);
	const columns = useMemo(
		() =>
			fixtureSheetColumns(true, presentStep).filter((column) =>
				DEFAULT_FIXTURE_SHEET_COLUMNS.includes(
					column.id as (typeof DEFAULT_FIXTURE_SHEET_COLUMNS)[number],
				),
			),
		[],
	);
	return (
		<div style={{ height: 680, minWidth: compact ? 420 : 760 }}>
			<FixtureSheetWindowView
				compact={compact}
				selectionCount={2}
				info={<SourceLegend />}
				table={
					<FixtureSheetTable
						activeRow={activeRow}
						columns={columns}
						onActivate={() => undefined}
						onActiveRowChange={setActiveRow}
						presentStep={presentStep}
						rows={rows}
						selectedFixtureIds={new Set(["fixture-101", "fixture-102"])}
					/>
				}
			/>
		</div>
	);
}

const meta = {
	title: "ToskLight/Windows/Fixture Sheet",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const SelectedAndActiveSteps: Story = {
	render: () => <FixtureSheetStory />,
};

export const Compact: Story = {
	render: () => <FixtureSheetStory compact />,
};
