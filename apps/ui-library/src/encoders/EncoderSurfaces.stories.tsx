import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
	EncoderSection,
	type EncoderSectionModel,
	type EncoderSectionSurface,
	HardwareEncoderDisplayView,
	TouchEncoder,
} from "../encoders";

interface EncoderStoryProps {
	surface: EncoderSectionSurface;
	family: string;
	accentColor: string;
	disabled: boolean;
	indexed: boolean;
	showSecondary: boolean;
	slowStep: number;
	fastStep: number;
	repeatSeconds: number;
}

const meta: Meta<EncoderStoryProps> = {
	title: "Controls/Encoders",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		surface: { control: "inline-radio", options: ["touch", "hardware"] },
		family: { control: "text" },
		accentColor: { control: "color" },
		disabled: { control: "boolean" },
		indexed: { control: "boolean" },
		showSecondary: { control: "boolean" },
		slowStep: { control: { type: "number", min: 0.0001, step: 0.0001 } },
		fastStep: { control: { type: "number", min: 0.001, step: 0.001 } },
		repeatSeconds: { control: { type: "number", min: 0.02, step: 0.01 } },
	},
	args: {
		surface: "touch",
		family: "Position",
		accentColor: "#176777",
		disabled: false,
		indexed: false,
		showSecondary: true,
		slowStep: 0.001,
		fastStep: 0.01,
		repeatSeconds: 0.08,
	},
};

export default meta;
type Story = StoryObj<EncoderStoryProps>;

export const IndividualTouch: Story = {
	render: ({
		accentColor,
		disabled,
		fastStep,
		indexed,
		repeatSeconds,
		slowStep,
	}) => {
		const [state, setState] = useState({ value: 0.52, owned: true });
		return (
			<div style={{ width: 180, height: 420 }}>
				<TouchEncoder
					label="Enc 1 · Dimmer"
					slot={1}
					attributeLabel="Dimmer"
					display={
						state.owned
							? indexed
								? "Gobo 3"
								: `${(state.value * 100).toFixed(1)}%`
							: "Released"
					}
					value={state.value}
					mode={indexed ? "Indexed" : "Coarse · Fine"}
					accentColor={accentColor}
					disabled={disabled}
					indexed={indexed}
					slowStep={slowStep}
					fastStep={fastStep}
					repeatSeconds={repeatSeconds}
					canRelease={state.owned}
					onStep={(delta) =>
						setState((current) => ({
							value: Math.max(0, Math.min(1, current.value + delta)),
							owned: true,
						}))
					}
					onSet={(value) => setState({ value, owned: true })}
					onRelease={() =>
						setState((current) => ({ ...current, owned: false }))
					}
				/>
			</div>
		);
	},
};

export const IndividualTouchChoices: Story = {
	render: ({ accentColor, disabled }) => {
		const options = ["Loop", "One-shot"];
		const [index, setIndex] = useState(0);
		const selectIndex = (value: number) =>
			setIndex(Math.max(0, Math.min(options.length - 1, Math.round(value))));
		return (
			<div style={{ width: 180, height: 420 }}>
				<TouchEncoder
					label="Enc 1 · Run mode"
					slot={1}
					attributeLabel="Run mode"
					display={options[index]}
					value={index}
					minimum={0}
					maximum={options.length - 1}
					inputScale={1}
					slowStep={1}
					fastStep={1}
					touchInteraction="choices"
					presets={{
						selectedValue: String(index),
						groups: [
							{
								label: "Run mode",
								options: options.map((label, value) => ({
									value: String(value),
									label,
								})),
							},
						],
					}}
					accentColor={accentColor}
					disabled={disabled}
					onStep={(delta) => selectIndex(index + delta)}
					onSet={selectIndex}
				/>
			</div>
		);
	},
};

export const IndividualTouchReleased: Story = {
	render: ({ accentColor, disabled }) => (
		<div style={{ width: 180, height: 420 }}>
			<TouchEncoder
				label="Enc 1 · Dimmer"
				slot={1}
				attributeLabel="Dimmer"
				display="Released"
				value={0.52}
				mode="No programmer ownership"
				accentColor={accentColor}
				disabled={disabled}
				onStep={() => undefined}
				onSet={() => undefined}
			/>
		</div>
	),
};

export const IndividualTouchDisabled: Story = {
	...IndividualTouchReleased,
	args: { disabled: true },
};

export const IndividualTouchIndexed: Story = {
	...IndividualTouch,
	args: { indexed: true },
};

export const ScaledInternalValue: Story = {
	render: ({ accentColor, disabled }) => {
		const [value, setValue] = useState(520);
		return (
			<div style={{ width: 180, height: 420 }}>
				<TouchEncoder
					label="Enc 1 · Scaled dimmer"
					slot={1}
					attributeLabel="Scaled dimmer"
					value={value}
					formatValue={(internal) => `${(internal / 10).toFixed(1)}%`}
					minimum={0}
					maximum={1000}
					inputScale={0.1}
					slowStep={1}
					fastStep={10}
					accentColor={accentColor}
					disabled={disabled}
					onStep={(delta) =>
						setValue((current) => Math.max(0, Math.min(1000, current + delta)))
					}
					onSet={setValue}
				/>
			</div>
		);
	},
};

export const IndividualHardware: Story = {
	render: ({ disabled, showSecondary }) => {
		const [value, setValue] = useState(90);
		const [secondaryValue, setSecondaryValue] = useState(30);
		const [owned, setOwned] = useState(true);
		return (
			<div className="hardware-connected" style={{ width: 140, height: 170 }}>
				<HardwareEncoderDisplayView
					slot={1}
					target={{
						label: "Pan",
						value: owned ? "80° ... 100°" : "Released",
						role: "Turn",
					}}
					secondary={
						showSecondary
							? {
									label: "Tilt",
									value: `${secondaryValue}°`,
									role: "Push-turn",
								}
							: undefined
					}
					editValue={value}
					secondaryEditValue={secondaryValue}
					canRelease={owned}
					onEdit={
						disabled
							? undefined
							: (next) => {
									setValue(next);
									setOwned(true);
								}
					}
					onSecondaryEdit={showSecondary ? setSecondaryValue : undefined}
					onRelease={() => setOwned(false)}
				/>
			</div>
		);
	},
};

function FamilyExample(props: EncoderStoryProps) {
	const [values, setValues] = useState({
		red: 0.8,
		green: 0.45,
		blue: 0.25,
		white: 0.6,
		amber: 0.35,
		uv: 0.15,
	});
	const [owned, setOwned] = useState(() => new Set(["red", "green", "blue"]));
	const encoderTargets = [
		{ id: "red", label: "Red", slot: 1, color: "#ff4d57" },
		{ id: "green", label: "Green", slot: 2, color: "#43d66f" },
		{ id: "blue", label: "Blue", slot: 3, color: "#4f8cff" },
		{ id: "white", label: "White", slot: 4, color: "#edf4f6" },
		{ id: "amber", label: "Amber", slot: 5, color: "#ffb52e" },
		{ id: "uv", label: "UV", slot: 6, color: "#a56cff" },
	] as const;
	const valuePresets = {
		groups: [
			{
				label: "Intensity",
				options: [
					{ value: "0", label: "Off", description: "Release output to zero." },
					{
						value: "25",
						label: "Quarter",
						description: "Low working level.",
					},
					{
						value: "50",
						label: "Half",
						description: "Balanced working level.",
					},
					{
						value: "100",
						label: "Full",
						description: "Maximum output.",
					},
				],
			},
			{
				label: "Operator defaults",
				options: [
					{
						value: "42",
						label: "House preset",
						icon: "★",
						description: "Stored house intensity.",
					},
					{
						value: "68",
						label: "Show level",
						icon: "●",
						description: "Current show default.",
					},
				],
			},
		],
	} as const;
	const model: EncoderSectionModel = {
		id: props.family.toLowerCase().replace(/\s+/g, "-"),
		label: props.family,
		description: "RGBW · AUV",
		encoders: encoderTargets.map(({ id, label, slot, color }) => ({
			id,
			slot,
			target: {
				label,
				display:
					id === "white"
						? "0% ... 100%"
						: owned.has(id)
							? `${Math.round(values[id] * 100)}%`
							: "Released",
			},
			value: values[id],
			accentColor: color,
			disabled: props.disabled,
			indexed: props.indexed,
			canRelease: owned.has(id),
			presets: {
				...valuePresets,
				selectedValue: String(Math.round(values[id] * 100)),
			},
		})),
	};
	return (
		<div
			className={props.surface === "hardware" ? "hardware-connected" : ""}
			style={{ width: 760, minHeight: 220 }}
		>
			<EncoderSection
				model={model}
				surface={props.surface}
				callbacks={{
					onRelativeChange: (id, delta) => {
						setValues((current) => ({
							...current,
							[id]: Math.max(
								0,
								Math.min(1, current[id as keyof typeof current] + delta),
							),
						}));
						setOwned((current) => new Set(current).add(id));
					},
					onAbsoluteChange: (id, value) => {
						setValues((current) => ({ ...current, [id]: value }));
						setOwned((current) => new Set(current).add(id));
					},
					onRangeChange: (id, points) => {
						const first = points[0];
						if (first === undefined) return;
						setValues((current) => ({
							...current,
							[id]: Math.max(0, Math.min(1, first / 100)),
						}));
						setOwned((current) => new Set(current).add(id));
					},
					onRelease: (id) => {
						setOwned((current) => {
							const next = new Set(current);
							next.delete(id);
							return next;
						});
					},
				}}
			/>
		</div>
	);
}

export const ConfigurableFamily: Story = {
	render: (args) => <FamilyExample {...args} />,
	args: {
		family: "Color",
		showSecondary: false,
	},
};
