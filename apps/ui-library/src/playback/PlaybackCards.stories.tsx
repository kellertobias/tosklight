import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import {
	HardwareCueRowsView,
	PlaybackBankView,
	type PlaybackCardKind,
	type PlaybackCardSummary,
	type PlaybackCardViewModel,
} from "../playback";

interface ConfigurablePlaybackStoryProps {
	mode: "touch" | "hardware";
	playbacksWide: number;
	playbacksHigh: number;
	availableWidth: number;
	kind: PlaybackCardKind;
	name: string;
	page: number;
	slot: number;
	summaryLabel: string;
	summaryDetail: string;
	progress: number;
	buttonCount: 1 | 2 | 3;
	topButton: string;
	middleButton: string;
	bottomButton: string;
	activeButton: "none" | "top" | "middle" | "bottom";
	hasFader: boolean;
	faderValue: number;
	empty: boolean;
	selected: boolean;
	pickupRequired: boolean;
	physicalPosition: number;
	pickupTarget: number;
}

const meta: Meta<ConfigurablePlaybackStoryProps> = {
	title: "Playbacks/Playback bank",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		mode: { control: "inline-radio", options: ["touch", "hardware"] },
		playbacksWide: {
			control: { type: "range", min: 1, max: 12, step: 1 },
		},
		playbacksHigh: {
			control: { type: "range", min: 1, max: 6, step: 1 },
		},
		availableWidth: {
			control: { type: "range", min: 640, max: 1600, step: 10 },
		},
		kind: {
			control: "select",
			options: [
				"cue-list",
				"group-master",
				"speed-group",
				"special-master",
				"empty",
			],
		},
		name: { control: "text" },
		page: { control: { type: "number", min: 1, step: 1 } },
		slot: { control: { type: "number", min: 1, step: 1 } },
		summaryLabel: { control: "text" },
		summaryDetail: { control: "text" },
		progress: { control: { type: "range", min: 0, max: 1, step: 0.01 } },
		buttonCount: { control: "inline-radio", options: [1, 2, 3] },
		topButton: { control: "text" },
		middleButton: { control: "text" },
		bottomButton: { control: "text" },
		activeButton: {
			control: "inline-radio",
			options: ["none", "top", "middle", "bottom"],
		},
		hasFader: { control: "boolean" },
		faderValue: { control: { type: "range", min: 0, max: 100, step: 1 } },
		empty: { control: "boolean" },
		selected: { control: "boolean" },
		pickupRequired: { control: "boolean" },
		physicalPosition: {
			control: { type: "range", min: 0, max: 1, step: 0.01 },
			if: { arg: "pickupRequired", truthy: true },
		},
		pickupTarget: {
			control: { type: "range", min: 0, max: 1, step: 0.01 },
			if: { arg: "pickupRequired", truthy: true },
		},
	},
	args: {
		mode: "touch",
		playbacksWide: 8,
		playbacksHigh: 2,
		availableWidth: 960,
		kind: "cue-list",
		name: "Main Cuelist",
		page: 2,
		slot: 1,
		summaryLabel: "4 · Mephisto Stage Center",
		summaryDetail: "3.2s",
		progress: 0.62,
		buttonCount: 3,
		topButton: "GO −",
		middleButton: "GO +",
		bottomButton: "FLASH",
		activeButton: "none",
		hasFader: true,
		faderValue: 62,
		empty: false,
		selected: false,
		pickupRequired: false,
		physicalPosition: 0.25,
		pickupTarget: 0.75,
	},
};

export default meta;
type Story = StoryObj<ConfigurablePlaybackStoryProps>;

const bankExamples: Array<{
	kind: PlaybackCardKind;
	name: string;
	summary: PlaybackCardSummary | undefined;
}> = [
	{
		kind: "cue-list",
		name: "Main Cuelist",
		summary: {
			label: "4 · Mephisto Stage Center",
			detail: "3.2s",
			progress: 0.62,
		},
	},
	{
		kind: "group-master",
		name: "Profile Group",
		summary: { label: "12 Fixtures", detail: "62%" },
	},
	{
		kind: "cue-list",
		name: "House Sequence",
		summary: {
			label: "120 BPM",
			detail: "running",
			beat: { count: 4, active: 2 },
		},
	},
	{
		kind: "speed-group",
		name: "Speed Master",
		summary: {
			label: "128 BPM",
			detail: "manual",
			beat: { count: 4, active: 0 },
		},
	},
	{
		kind: "special-master",
		name: "Playback Fade Time",
		summary: { label: "Playback fade", detail: "2.5s" },
	},
	{
		kind: "special-master",
		name: "Grand Master",
		summary: { label: "Grand master", detail: "100%" },
	},
	{
		kind: "special-master",
		name: "Programmer Fade",
		summary: { label: "Programmer fade", detail: "1.0s" },
	},
	{ kind: "empty", name: "Empty", summary: undefined },
];

function actionModels(
	count: 1 | 2 | 3,
	labels: readonly string[],
	activeButton: ConfigurablePlaybackStoryProps["activeButton"] = "none",
) {
	const activeIndex = {
		none: -1,
		top: 0,
		middle: 1,
		bottom: 2,
	}[activeButton];
	return Array.from({ length: count }, (_, index) => ({
		id: `action-${index}`,
		label: labels[index] || `BUTTON ${index + 1}`,
		className: index === activeIndex ? "playback-button-active" : undefined,
	}));
}

function ConfigurablePlaybackExample(args: ConfigurablePlaybackStoryProps) {
	const [value, setValue] = useState(args.faderValue);
	useEffect(() => setValue(args.faderValue), [args.faderValue]);
	const assigned = !args.empty && args.kind !== "empty";
	const model: PlaybackCardViewModel = {
		page: args.page,
		slot: args.slot,
		row: 0,
		rowUnits: args.hasFader ? 4 : 2,
		name: assigned ? args.name : "Empty",
		assigned,
		kind: assigned ? args.kind : "empty",
		selected: args.selected,
		hasFader: assigned && args.hasFader,
		faderValue: value,
		faderLabel: "Master",
		faderDisplay: `${Math.round(value)}%`,
		summary: assigned
			? {
					label: args.summaryLabel,
					detail: args.summaryDetail,
					progress: args.progress,
				}
			: undefined,
		hardwarePickup:
			assigned &&
			args.mode === "hardware" &&
			args.hasFader &&
			args.pickupRequired
				? {
						physicalPosition: args.physicalPosition,
						pickupTarget: args.pickupTarget,
					}
				: undefined,
		actions: assigned
			? actionModels(
					args.buttonCount,
					[args.topButton, args.middleButton, args.bottomButton],
					args.activeButton,
				)
			: [],
	};
	return (
		<div
			style={{
				width: args.mode === "touch" ? 330 : 300,
				height: args.mode === "touch" ? 560 : 300,
			}}
		>
			<PlaybackBankView
				mode={args.mode}
				columns={1}
				items={[
					{
						model,
						callbacks: { onFaderChange: setValue },
						cueRows:
							args.mode === "hardware" && args.kind === "cue-list" ? (
								<HardwareCueRowsView
									previous={{ number: 3, name: "Previous" }}
									current={{
										number: 4,
										name: args.summaryLabel,
										fadeMillis: 3200,
									}}
									next={{ number: 5, name: "Next" }}
									progress={args.progress}
								/>
							) : undefined,
					},
				]}
			/>
		</div>
	);
}

function bankModel({
	exampleIndex,
	itemIndex,
	mode,
	row,
	rows,
	value,
}: {
	exampleIndex: number;
	itemIndex: number;
	mode: "touch" | "hardware";
	row: number;
	rows: number;
	value: number;
}): PlaybackCardViewModel {
	const example = bankExamples[exampleIndex] ?? bankExamples[0];
	const assigned = example.kind !== "empty";
	const faderRow = rows <= 2 && row === rows - 1;
	const labelOnly = mode === "hardware" && rows >= 5;
	const buttonCount: 1 | 2 | 3 =
		rows === 1 || faderRow
			? 3
			: mode === "hardware" && (rows === 3 || rows === 4)
				? 1
				: rows === 2
					? 1
					: itemIndex % 2 === 0
						? 1
						: 2;
	const loaded = exampleIndex === 0 && faderRow;
	const activeButton =
		assigned && exampleIndex === 2 && faderRow
			? buttonCount === 1
				? "top"
				: buttonCount === 2
					? "middle"
					: "bottom"
			: "none";
	const faderValue =
		exampleIndex === 5
			? 100
			: exampleIndex === 4
				? 35
				: exampleIndex === 6
					? 48
					: value;
	return {
		page: 2,
		slot: itemIndex + 1,
		row,
		rowUnits: faderRow ? (mode === "touch" ? 4 : 2) : 1,
		name: example.name,
		assigned,
		kind: example.kind,
		selected: exampleIndex === 1 && row === rows - 1,
		className: faderRow ? undefined : "playback-row-compact",
		hasFader: assigned && faderRow,
		faderValue,
		faderLabel: "Master",
		faderDisplay: `${Math.round(faderValue)}%`,
		status: loaded ? { kind: "loaded", label: "LOADED" } : undefined,
		hardwareButtonLabel: labelOnly
			? assigned
				? exampleIndex === 2
					? "PAUSE"
					: "GO −"
				: ""
			: undefined,
		summary:
			assigned && example.summary
				? {
						...example.summary,
					}
				: undefined,
		hardwarePickup:
			mode === "hardware" && faderRow && exampleIndex === 1
				? { physicalPosition: 0.75, pickupTarget: 0.5 }
				: mode === "hardware" && faderRow && exampleIndex === 2
					? { physicalPosition: 0.5, pickupTarget: 0.75 }
					: undefined,
		actions: assigned && !labelOnly
			? actionModels(
					buttonCount,
					[
						exampleIndex === 2 ? "PAUSE" : "GO −",
						exampleIndex === 2 ? "TAP" : "GO +",
						"FLASH",
					],
					activeButton,
				)
			: [],
	};
}

function PlaybackGroupExample({
	mode,
	playbacksWide,
	playbacksHigh,
	availableWidth,
}: {
	mode: "touch" | "hardware";
	playbacksWide: number;
	playbacksHigh: number;
	availableWidth: number;
}) {
	const columns = Math.max(1, Math.min(12, Math.round(playbacksWide)));
	const rows = Math.max(1, Math.min(6, Math.round(playbacksHigh)));
	const requestedWidth = Math.max(
		640,
		Math.min(1600, Math.round(availableWidth)),
	);
	const [width, setWidth] = useState(requestedWidth);
	const [values, setValues] = useState<Record<number, number>>({});
	useEffect(() => setWidth(requestedWidth), [requestedWidth]);
	const items = Array.from({ length: columns * rows }, (_, itemIndex) => {
		const row = Math.floor(itemIndex / columns);
		const exampleIndex = itemIndex % bankExamples.length;
		const model = bankModel({
			exampleIndex,
			itemIndex,
			mode,
			row,
			rows,
			value: values[itemIndex] ?? 62,
		});
		const loaded = exampleIndex === 0 && row === rows - 1;
		return {
			model,
			callbacks: {
				onFaderChange: (next: number) =>
					setValues((current) => ({
						...current,
						[itemIndex]: Math.round(next),
					})),
			},
			cueRows:
				mode === "hardware" && model.kind === "cue-list" ? (
					<HardwareCueRowsView
						previous={{ number: 3, name: "House Open" }}
						current={{
							number: 4,
							name: "Mephisto Stage Center",
							fadeMillis: 2500,
						}}
						next={{ number: 5, name: "Stage Blackout" }}
						nextLoaded={loaded}
						progress={0.46}
					/>
				) : undefined,
		};
	});
	return (
		<section style={{ width: "fit-content" }}>
			<label
				style={{
					width,
					height: 24,
					display: "grid",
					gridTemplateColumns: "auto minmax(180px, 1fr) auto",
					alignItems: "center",
					gap: 8,
					color: "#a5afb6",
					fontSize: 11,
				}}
			>
				<span>Available width</span>
				<input
					aria-label={`${mode} playback group available width`}
					type="range"
					min="640"
					max="1600"
					step="10"
					value={width}
					onChange={(event) => setWidth(Number(event.currentTarget.value))}
				/>
				<output>{width}px</output>
			</label>
			<div
				data-playback-group-frame={mode}
				style={{
					width,
					height: mode === "touch" ? 280 : 140,
				}}
			>
				<PlaybackBankView
					mode={mode}
					columns={columns}
					items={items}
					rowWeights={
						rows === 2
							? mode === "touch"
								? [1, 4]
								: [1, 2]
							: Array.from({ length: rows }, () => 1)
					}
				/>
			</div>
		</section>
	);
}

export const ConfigurablePlayback: Story = {
	parameters: {
		controls: {
			exclude: ["playbacksWide", "playbacksHigh", "availableWidth"],
		},
	},
	render: (args) => <ConfigurablePlaybackExample {...args} />,
};

export const EightByTwoTouchBank: Story = {
	name: "Configurable touch group",
	parameters: {
		controls: {
			include: ["playbacksWide", "playbacksHigh", "availableWidth"],
		},
	},
	render: (args) => (
		<PlaybackGroupExample
			mode="touch"
			playbacksWide={args.playbacksWide}
			playbacksHigh={args.playbacksHigh}
			availableWidth={args.availableWidth}
		/>
	),
};

export const EightByTwoHardwareBank: Story = {
	name: "Configurable hardware group",
	parameters: {
		controls: {
			include: ["playbacksWide", "playbacksHigh", "availableWidth"],
		},
	},
	render: (args) => (
		<PlaybackGroupExample
			mode="hardware"
			playbacksWide={args.playbacksWide}
			playbacksHigh={args.playbacksHigh}
			availableWidth={args.availableWidth}
		/>
	),
};
