import type { Meta, StoryObj } from "@storybook/react-vite";
import type { FixtureSheetRowView } from "@tosklight/ui/tables";
import type { DataTableColumn } from "@tosklight/ui/window-kit";
import { useState } from "react";
import { FixtureSheetTableView as FixtureSheetTable } from "@tosklight/ui/tables";
import { FixtureSheetWindowView } from "./FixtureSheetWindow";

interface FixtureStoryRow extends FixtureSheetRowView {
	number: number;
	name: string;
	patch: string;
	intensity: string;
	source: string;
}

const rows: FixtureStoryRow[] = [
	{
		id: "fixture-1",
		fixtureId: "fixture-1",
		targetKind: "fixture",
		parentFixtureId: "",
		childFixtureIds: [],
		indented: false,
		number: 1,
		name: "Front Fresnel SL",
		patch: "1.001",
		intensity: "72%",
		source: "Programmer",
	},
	{
		id: "fixture-2",
		fixtureId: "fixture-2",
		targetKind: "fixture",
		parentFixtureId: "",
		childFixtureIds: ["fixture-2-head-1"],
		indented: false,
		number: 2,
		name: "Stage Left Mover",
		patch: "1.101",
		intensity: "48%",
		source: "Cue 4",
	},
	{
		id: "fixture-2-head-1",
		fixtureId: "fixture-2-head-1",
		targetKind: "head",
		parentFixtureId: "fixture-2",
		childFixtureIds: [],
		indented: true,
		number: 2,
		name: "Head 1",
		patch: "1.101",
		intensity: "48%",
		source: "Cue 4",
	},
	{
		id: "fixture-3",
		fixtureId: "fixture-3",
		targetKind: "fixture",
		parentFixtureId: "",
		childFixtureIds: [],
		indented: false,
		number: 3,
		name: "Back Wash",
		patch: "1.121",
		intensity: "0%",
		source: "Default",
	},
];

const columns: DataTableColumn<FixtureStoryRow>[] = [
	{ id: "number", header: "#", width: "64px", render: (row) => row.number },
	{
		id: "name",
		header: "Name",
		width: "minmax(180px, 1fr)",
		render: (row) => row.name,
	},
	{ id: "patch", header: "Patch", width: "100px", render: (row) => row.patch },
	{
		id: "intensity",
		header: "Intensity",
		width: "110px",
		align: "right",
		render: (row) => row.intensity,
	},
	{
		id: "source",
		header: "Source",
		width: "130px",
		render: (row) => row.source,
	},
];

function FixtureSheetStory({ compact = false }: { compact?: boolean }) {
	const [activeRow, setActiveRow] = useState(1);
	return (
		<div style={{ height: 680, minWidth: compact ? 420 : 760 }}>
			<FixtureSheetWindowView
				compact={compact}
				selectionCount={2}
				info="Programmer · Cue 4"
				table={
					<FixtureSheetTable
						activeRow={activeRow}
						columns={columns}
						onActivate={() => undefined}
						onActiveRowChange={setActiveRow}
						presentStep={(row) => ({
							base: row.fixtureId === "fixture-1",
							containedBase: false,
							current: row.fixtureId === "fixture-2-head-1",
							containedCurrent: row.fixtureId === "fixture-2",
						})}
						rows={rows}
						selectedFixtureIds={new Set(["fixture-1", "fixture-2"])}
					/>
				}
			/>
		</div>
	);
}

const meta = {
	title: "Application/Windows/Fixture Sheet",
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
