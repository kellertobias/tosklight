import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { ButtonGrid } from "../grids";
import {
	PoolCard,
	type PoolCardViewModel,
	type PoolObjectType,
	resolvePoolPresentation,
} from "../pools";

interface PoolStoryProps {
	width: number;
	minimum: number;
	holdDelay: number;
}

const meta: Meta<PoolStoryProps> = {
	title: "Tables and Grids/Pools/Production pool cards",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		width: { control: { type: "range", min: 280, max: 1000, step: 20 } },
		minimum: { control: { type: "range", min: 88, max: 180, step: 2 } },
		holdDelay: { control: { type: "range", min: 300, max: 1000, step: 50 } },
	},
	args: { width: 720, minimum: 132, holdDelay: 650 },
};

export default meta;
type Story = StoryObj<PoolStoryProps>;

const models: PoolCardViewModel[] = [
	{
		number: 1,
		primary: "All",
		secondary: "12 fixtures · ordered",
		kind: "group",
		states: ["selected"],
		icon: "◇",
		iconColor: "#5ff2ff",
		iconBackgroundColor: "#16383d",
		color: "#1bd6ec",
	},
	{
		number: 2,
		primary: "Front Wash With A Deliberately Long Operator Name",
		secondary: "4 fixtures · ordered",
		details: ["1 portable attribute"],
		kind: "group",
		states: ["active"],
		derived: true,
	},
	{
		number: 3,
		primary: "Frozen",
		secondary: "Revision 8",
		kind: "group",
		frozen: true,
		frozenLabel: "Frozen · rev 8",
	},
	{
		number: 4,
		primary: "Blue",
		secondary: "Color preset",
		kind: "preset",
		color: "#264fd4",
	},
	{
		number: 5,
		primary: "Main",
		secondary: "Cuelist · 62%",
		details: ["Playbacks on pages 1, 2"],
		kind: "cuelist",
		states: ["active"],
		image: {
			src: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 160 100'%3E%3Crect width='160' height='100' fill='%23182229'/%3E%3Ccircle cx='46' cy='50' r='24' fill='%231bd6ec' fill-opacity='.65'/%3E%3Ccircle cx='114' cy='50' r='24' fill='%23f4b942' fill-opacity='.65'/%3E%3C/svg%3E",
			alt: "Stage look thumbnail",
		},
	},
	{
		number: 6,
		primary: "Empty",
		secondary: "Press Record to use this slot",
		kind: "group",
		states: ["empty"],
	},
	{
		number: 7,
		primary: "Record here",
		kind: "preset",
		states: ["empty", "record-target"],
	},
	{ number: 8, primary: "Update", kind: "cuelist", states: ["update-target"] },
	{ number: 9, primary: "Set target", kind: "cuelist", states: ["set-target"] },
];

function PoolGridExample({ width, minimum, holdDelay }: PoolStoryProps) {
	const [event, setEvent] = useState(
		"Tap a card or hold one for its context action",
	);
	return (
		<div style={{ width }}>
			<output aria-live="polite" style={{ display: "block", minHeight: 28 }}>
				{event}
			</output>
			<ButtonGrid className="card-pool pool-filled-tinted" minimum={minimum}>
				{models.map((model) => (
					<PoolCard
						key={String(model.number)}
						model={model}
						holdDelay={holdDelay}
						onClick={() => setEvent(`Clicked ${model.primary}`)}
						onPressHold={() => setEvent(`Held ${model.primary}`)}
					/>
				))}
			</ButtonGrid>
		</div>
	);
}

export const ScalingAndEveryState: Story = {
	render: (args) => <PoolGridExample {...args} />,
};

export const NarrowScaling: Story = {
	args: { width: 340, minimum: 88 },
	render: (args) => <PoolGridExample {...args} />,
};

export const OutlineOnlyFilledCards: Story = {
	args: { width: 720, minimum: 132 },
	render: ({ width, minimum, holdDelay }) => (
		<div style={{ width }}>
			<ButtonGrid className="card-pool pool-filled-outline" minimum={minimum}>
				{models.slice(0, 5).map((model) => (
					<PoolCard
						key={String(model.number)}
						model={model}
						holdDelay={holdDelay}
					/>
				))}
			</ButtonGrid>
		</div>
	),
};

export const FilteredNumbersStayStable: Story = {
	args: { width: 360, minimum: 132 },
	render: ({ width, minimum }) => (
		<div style={{ width }}>
			<ButtonGrid className="card-pool pool-filled-tinted" minimum={minimum}>
				{[models[0], models[3]].map((model) => (
					<PoolCard key={String(model.number)} model={model} />
				))}
			</ButtonGrid>
		</div>
	),
};

const defaultColorCards: Array<{
	type: PoolObjectType;
	label: string;
	family?: "mixed" | "intensity" | "color" | "position" | "beam";
}> = [
	{ type: "group", label: "Groups" },
	{ type: "macro", label: "Macros" },
	{ type: "dynamic", label: "Dynamics" },
	{ type: "cuelist", label: "Cuelists" },
	{ type: "sequence", label: "Sequences" },
	{ type: "preset", label: "Mixed", family: "mixed" },
	{ type: "preset", label: "Intensity", family: "intensity" },
	{ type: "preset", label: "Color", family: "color" },
	{ type: "preset", label: "Position", family: "position" },
	{ type: "preset", label: "Beam", family: "beam" },
];

export const ConsistentObjectTypeColors: Story = {
	args: { width: 920, minimum: 132 },
	render: ({ width, minimum }) => (
		<div style={{ width }}>
			<h2>Type colors</h2>
			<ButtonGrid className="card-pool" minimum={minimum}>
				{defaultColorCards.map(({ type, label, family }, index) => {
					const presentation = resolvePoolPresentation({
						objectType: type,
						presetFamily: family,
						mode: "type",
						states: index === 0 ? ["selected"] : [],
					});
					return (
						<PoolCard
							key={`${type}-${label}`}
							className={presentation.className}
							style={presentation.style}
							aria-pressed={index === 0}
							model={{
								number: index + 1,
								primary: label,
								secondary: type === "preset" ? "Preset family" : type,
								kind:
									type === "group" || type === "preset" || type === "cuelist"
										? type
										: "generic",
							}}
						/>
					);
				})}
			</ButtonGrid>
			<h2>Individual colors and non-color states</h2>
			<ButtonGrid className="card-pool" minimum={minimum}>
				{[
					{ label: "Explicit", color: "#6857d8", states: ["focused"] as const },
					{ label: "Uncolored", states: [] as const },
					{
						label: "Record target",
						states: ["record-target"] as const,
					},
					{
						label: "Update target",
						states: ["update-target"] as const,
					},
					{
						label: "Empty",
						states: ["empty"] as const,
					},
				].map(({ label, color, states }, index) => {
					const presentation = resolvePoolPresentation({
						objectType: "group",
						mode: "individual",
						itemColor: color,
						states,
					});
					return (
						<PoolCard
							key={label}
							className={presentation.className}
							style={presentation.style}
							model={{
								number: index + 1,
								primary: label,
								secondary: "Group",
								kind: "group",
							}}
						/>
					);
				})}
			</ButtonGrid>
		</div>
	),
};
