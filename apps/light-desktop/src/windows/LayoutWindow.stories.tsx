import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "@tosklight/ui";
import { useState } from "react";
import { LayoutWindow } from "./LayoutWindow";

const meta = {
	title: "ToskLight/Windows/Layout",
	component: LayoutWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta<typeof LayoutWindow>;

export default meta;
type Story = StoryObj<typeof meta>;

const frontCells = [
	{ id: "101", level: 82, color: "#ff6b52", column: 1, row: 1 },
	{ id: "102", level: 68, color: "#ffb347", column: 2, row: 1 },
	{ id: "103", level: 74, color: "#ffe066", column: 3, row: 1 },
	{ id: "111", level: 45, color: "#66d9ff", column: 1, row: 2 },
	{ id: "112", level: 52, color: "#8fa8ff", column: 2, row: 2 },
	{ id: "113", level: 39, color: "#c18cff", column: 3, row: 2 },
] as const;

const backCells = [
	{ id: "201", level: 31, color: "#4fd1a5", column: 1, row: 1 },
	{ id: "202", level: 57, color: "#52b7ff", column: 2, row: 1 },
	{ id: "203", level: 63, color: "#b794f4", column: 3, row: 1 },
] as const;

export const RepresentativeGroup: Story = {
	render: () => (
		<div style={{ width: 980, height: 620, padding: 16 }}>
			<StoryLayout
				name="Front Truss"
				groupId="1"
				cells={frontCells}
				initialSelection={["102", "103"]}
			/>
		</div>
	),
};

export const TwoIndependentGroups: Story = {
	render: () => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "1fr 1fr",
				gap: 16,
				width: 1280,
				height: 520,
				padding: 16,
			}}
		>
			<StoryLayout
				name="Front Truss"
				groupId="1"
				cells={frontCells}
				initialSelection={["102"]}
			/>
			<StoryLayout
				name="Back Truss"
				groupId="2"
				cells={backCells}
				initialSelection={[]}
			/>
		</div>
	),
};

function StoryLayout({
	name,
	groupId,
	cells,
	initialSelection,
}: {
	name: string;
	groupId: string;
	cells: readonly {
		id: string;
		level: number;
		color: string;
		column: number;
		row: number;
	}[];
	initialSelection: readonly string[];
}) {
	const [selected, setSelected] = useState(() => new Set(initialSelection));
	return (
		<section
			className="layout-window"
			aria-label={`${name} Layout pane, Group ${groupId}`}
			data-layout-group-id={groupId}
		>
			<header className="layout-window-header">
				<strong>Layout · {name}</strong>
				<span>Group {groupId}</span>
			</header>
			<section
				className="layout-grid"
				aria-label={`${name} fixture layout`}
				style={{
					gridTemplateColumns: "repeat(3, minmax(5rem, 1fr))",
					gridTemplateRows: "repeat(2, minmax(4rem, 1fr))",
				}}
			>
				{cells.map((cell) => {
					const active = selected.has(cell.id);
					return (
						<Button
							key={cell.id}
							className={`layout-cell ${active ? "selected" : ""}`}
							aria-pressed={active}
							aria-label={`Fixture ${cell.id}, ${cell.level}%`}
							data-layout-fixture-id={cell.id}
							style={
								{
									gridColumn: cell.column,
									gridRow: cell.row,
									"--layout-fixture-color": cell.color,
								} as React.CSSProperties
							}
							onClick={() =>
								setSelected((current) => {
									const next = new Set(current);
									if (next.has(cell.id)) next.delete(cell.id);
									else next.add(cell.id);
									return next;
								})
							}
						>
							<strong>{cell.id}</strong>
							<span className="layout-cell-level">{cell.level}%</span>
							<span className="layout-cell-color" aria-hidden="true" />
						</Button>
					);
				})}
			</section>
		</section>
	);
}
