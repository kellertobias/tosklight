import type { Meta, StoryObj } from "@storybook/react-vite";
import type { CSSProperties } from "react";
import { useState } from "react";
import { type PoolSlotViewModel, PoolWindow } from "./PoolWindow";

interface GenericPoolArgs {
	width: number;
	height: number;
	slotCount: number;
	minimumCardWidth: number;
}

const meta: Meta<GenericPoolArgs> = {
	title: "Pools/Generic pool window",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	args: {
		width: 980,
		height: 680,
		slotCount: 200,
		minimumCardWidth: 96,
	},
	argTypes: {
		width: { control: { type: "range", min: 320, max: 1400, step: 20 } },
		height: { control: { type: "range", min: 280, max: 900, step: 20 } },
		slotCount: { control: { type: "range", min: 1, max: 260, step: 1 } },
		minimumCardWidth: {
			control: { type: "range", min: 88, max: 180, step: 2 },
		},
	},
};

export default meta;
type Story = StoryObj<GenericPoolArgs>;

const host = (width: number, height: number): CSSProperties => ({
	width,
	height,
	minWidth: 0,
	containerType: "inline-size",
});

function emptySlot(index: number): PoolSlotViewModel<string> {
	return {
		id: `empty-${index + 1}`,
		position: index,
		card: {
			number: index + 1,
			primary: "Empty",
			secondary: "Press Record to use this slot",
			kind: "generic",
			states: ["empty"],
		},
	};
}

const sparseSlots: PoolSlotViewModel<string>[] = [
	{
		id: "group-1",
		position: 0,
		card: {
			number: 1,
			primary: "All Fixtures",
			secondary: "12 fixtures · ordered",
			kind: "group",
		},
	},
	{
		id: "preset-color-2",
		position: 8,
		card: {
			number: "2.2",
			primary: "Blue",
			secondary: "Color",
			kind: "preset",
			color: "#285bd8",
		},
	},
	{
		id: "cuelist-42",
		position: 41,
		card: {
			number: 42,
			primary: "Encore",
			secondary: "2 cues",
			kind: "cuelist",
		},
	},
];

function PoolExample({
	width,
	height,
	slotCount,
	minimumCardWidth,
	slots = sparseSlots,
}: GenericPoolArgs & { slots?: PoolSlotViewModel<string>[] }) {
	return (
		<div style={host(width, height)}>
			<PoolWindow
				title="Generic Pool"
				info={{
					primary: `${slots.length} stored`,
					secondary: `${slotCount} slots`,
				}}
				actions={[
					[{ id: "record", label: "Record", onClick: () => undefined }],
				]}
				settingsTabs={[
					{
						id: "pool",
						label: "Pool",
						content: "Card size, labels, and pool behavior",
					},
				]}
				slots={slots}
				slotCount={slotCount}
				emptySlot={emptySlot}
				minimumCardWidth={minimumCardWidth}
			/>
		</div>
	);
}

export const Empty: Story = {
	render: (args) => <PoolExample {...args} slots={[]} />,
};

export const Sparse: Story = {
	render: (args) => <PoolExample {...args} />,
};

export const EveryCardState: Story = {
	render: (args) => (
		<PoolExample
			{...args}
			slots={[
				{ id: "filled", position: 0, card: { number: 1, primary: "Filled" } },
				{
					id: "selected",
					position: 1,
					card: { number: 2, primary: "Selected", states: ["selected"] },
				},
				{
					id: "active",
					position: 2,
					card: { number: 3, primary: "Active", states: ["active"] },
				},
				{
					id: "disabled",
					position: 3,
					card: { number: 4, primary: "Disabled", states: ["disabled"] },
				},
				{
					id: "store",
					position: 4,
					card: {
						number: 5,
						primary: "Store target",
						states: ["empty", "store-target"],
					},
				},
				{
					id: "update",
					position: 5,
					card: {
						number: 6,
						primary: "Update target",
						states: ["update-target"],
					},
				},
				{
					id: "set",
					position: 6,
					card: { number: 7, primary: "Set target", states: ["set-target"] },
				},
				{
					id: "color",
					position: 7,
					card: { number: 8, primary: "Configured color", color: "#c026d3" },
				},
				{
					id: "derived",
					position: 8,
					card: {
						number: 9,
						primary: "Derived group",
						kind: "group",
						derived: true,
					},
				},
				{
					id: "frozen",
					position: 9,
					card: {
						number: 10,
						primary: "Frozen group",
						kind: "group",
						frozen: true,
					},
				},
			]}
		/>
	),
};

export const Narrow: Story = {
	args: { width: 360, height: 680, minimumCardWidth: 88 },
	render: (args) => <PoolExample {...args} />,
};

export const Wide: Story = {
	args: { width: 1320, height: 680, minimumCardWidth: 118 },
	render: (args) => <PoolExample {...args} />,
};

export const Extended: Story = {
	args: { slotCount: 260 },
	render: (args) => <PoolExample {...args} />,
};

export const NarrowTall: Story = {
	args: { width: 360, height: 820, minimumCardWidth: 88 },
	render: (args) => <PoolExample {...args} />,
};

export const NarrowShort: Story = {
	args: { width: 360, height: 360, minimumCardWidth: 88 },
	render: (args) => <PoolExample {...args} />,
};

export const WideTall: Story = {
	args: { width: 1320, height: 820, minimumCardWidth: 118 },
	render: (args) => <PoolExample {...args} />,
};

export const WideShort: Story = {
	args: { width: 1320, height: 360, minimumCardWidth: 118 },
	render: (args) => <PoolExample {...args} />,
};

function LiveResizeExample(args: GenericPoolArgs) {
	const [size, setSize] = useState({ width: 360, height: 360 });
	return (
		<>
			<div>
				<button
					type="button"
					onClick={() =>
						setSize((current) =>
							current.width === 360
								? { width: 1120, height: 720 }
								: { width: 360, height: 360 },
						)
					}
				>
					Resize pool viewport
				</button>
			</div>
			<PoolExample {...args} {...size} />
		</>
	);
}

export const LiveResize: Story = {
	render: (args) => <LiveResizeExample {...args} />,
};
