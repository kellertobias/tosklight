import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import {
	VirtualPlaybackGridView,
	type VirtualPlaybackBoxViewModel,
} from "./VirtualPlaybackGrid";

interface VirtualGridArgs {
	rows: number;
	columns: number;
	width: number;
}

const meta: Meta<VirtualGridArgs> = {
	title: "Tables and Grids/Virtual playback grid",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	args: { rows: 3, columns: 4, width: 920 },
	argTypes: {
		rows: { control: { type: "number", min: 1, max: 300, step: 1 } },
		columns: { control: { type: "number", min: 1, max: 300, step: 1 } },
		width: { control: { type: "range", min: 320, max: 1400, step: 20 } },
	},
};

export default meta;
type Story = StoryObj<VirtualGridArgs>;

const pageOne: VirtualPlaybackBoxViewModel[] = [
	{ number: 1001, slot: 1, position: 0, availability: "assigned", label: "Main", actionLabel: "GO", currentCue: "Cue 4 · Solo", color: "#176777", running: true },
	{ number: 1002, slot: 2, position: 1, availability: "assigned", label: "Front Wash", actionLabel: "TOGGLE", color: "#925ad1", icon: "☀" },
	{ number: 1004, slot: 4, position: 3, availability: "assigned", label: "Bump", actionLabel: "FLASH", color: "#d98236", heldAction: true },
	{ number: 1009, slot: 9, position: 8, availability: "unavailable" },
];

const pageTwo: VirtualPlaybackBoxViewModel[] = [
	{ number: 1301, slot: 1, position: 0, availability: "assigned", label: "House", actionLabel: "GO", currentCue: "Cue 1", color: "#2874bd" },
	{ number: 1303, slot: 3, position: 2, availability: "assigned", label: "Encore", actionLabel: "SWAP", heldAction: true, color: "#b33f75" },
	{ number: 1309, slot: 9, position: 8, availability: "unavailable" },
];

function GridHost({
	boxes,
	page = 1,
	rows,
	columns,
	width,
}: VirtualGridArgs & {
	boxes: VirtualPlaybackBoxViewModel[];
	page?: number;
}) {
	const [event, setEvent] = useState("Ready");
	return (
		<div style={{ width, height: 680, display: "grid", gridTemplateRows: "32px minmax(0, 1fr)" }}>
			<output aria-live="polite">{event}</output>
			<VirtualPlaybackGridView
				page={page}
				rows={rows}
				columns={columns}
				boxes={boxes.map((box) => ({
					...box,
					number: box.number ?? 1_001 + (page - 1) * 300 + box.slot - 1,
				}))}
				callbacks={{
					onAction: (slot) => setEvent(`Action ${slot}`),
					onActionPress: (slot) => setEvent(`Pressed ${slot}`),
					onActionRelease: (slot) => setEvent(`Released ${slot}`),
					onConfigure: (slot) => setEvent(`Configure ${slot}`),
					onAssign: (slot) => setEvent(`Assign ${slot}`),
					onUpdate: (slot) => setEvent(`Update ${slot}`),
					onZoneSelection: (slot) => setEvent(`Zone ${slot}`),
				}}
			/>
		</div>
	);
}

export const SparseGrid: Story = {
	render: (args) => <GridHost {...args} boxes={pageOne} />,
};

export const FullTwentyByFifteenPage: Story = {
	args: { rows: 20, columns: 15, width: 920 },
	render: (args) => <GridHost {...args} boxes={pageOne} />,
};

export const EveryState: Story = {
	args: { rows: 2, columns: 4 },
	render: (args) => (
		<GridHost
			{...args}
			boxes={[
				{ slot: 1, position: 0, availability: "assigned", label: "Running", actionLabel: "GO", running: true },
				{ slot: 2, position: 1, availability: "empty" },
				{ slot: 3, position: 2, availability: "unavailable" },
				{ slot: 4, position: 3, availability: "assigned", label: "Configure", configurationTarget: true },
				{ slot: 5, position: 4, availability: "empty", assignmentTarget: true },
				{ slot: 6, position: 5, availability: "assigned", label: "Update", updateTarget: true },
				{ slot: 7, position: 6, availability: "assigned", label: "Zone member", exclusionMember: true },
				{ slot: 8, position: 7, availability: "assigned", label: "Zone selected", exclusionMember: true, exclusionSelected: true, selectingExclusionZone: true },
			]}
		/>
	),
};

export const HeldFlashAndSwap: Story = {
	args: { rows: 1, columns: 2 },
	render: (args) => (
		<GridHost
			{...args}
			boxes={[
				{ slot: 1, position: 0, availability: "assigned", label: "Bump", actionLabel: "FLASH", heldAction: true },
				{ slot: 2, position: 1, availability: "assigned", label: "Solo", actionLabel: "SWAP", heldAction: true },
			]}
		/>
	),
};

export const PageSwitching: Story = {
	render: (args) => {
		const [page, setPage] = useState(1);
		return (
			<div style={{ width: args.width, height: 680, display: "grid", gridTemplateRows: "44px minmax(0, 1fr)" }}>
				<div>
					<Button onClick={() => setPage((current) => current === 1 ? 2 : 1)}>
						Next page
					</Button>
					<span aria-label="Current virtual playback page">Page {page}</span>
				</div>
				<VirtualPlaybackGridView
					page={page}
					rows={args.rows}
					columns={args.columns}
					boxes={page === 1 ? pageOne : pageTwo}
				/>
			</div>
		);
	},
};

export const NarrowTouch: Story = {
	args: { width: 360, rows: 3, columns: 4 },
	render: (args) => <GridHost {...args} boxes={pageOne} />,
};

export const WideTouch: Story = {
	args: { width: 1320, rows: 3, columns: 6 },
	render: (args) => <GridHost {...args} boxes={pageOne} />,
};
