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
}

const meta: Meta<EncoderStoryProps> = {
	title: "Encoders/Production encoder surfaces",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		surface: { control: "inline-radio", options: ["touch", "hardware"] },
		family: { control: "text" },
		accentColor: { control: "color" },
		disabled: { control: "boolean" },
		indexed: { control: "boolean" },
		showSecondary: { control: "boolean" },
	},
	args: {
		surface: "touch",
		family: "Position",
		accentColor: "#176777",
		disabled: false,
		indexed: false,
		showSecondary: true,
	},
};

export default meta;
type Story = StoryObj<EncoderStoryProps>;

export const IndividualTouch: Story = {
	render: ({ accentColor, disabled, indexed }) => {
		const [state, setState] = useState({ value: 0.52, owned: true });
		return (
			<div style={{ width: 180, height: 420 }}>
				<TouchEncoder
					label="Enc 1 · Dimmer"
					display={
						state.owned
							? indexed
								? "Gobo 3"
								: `${Math.round(state.value * 100)}%`
							: "Released"
					}
					value={state.value}
					mode={indexed ? "Indexed" : "Coarse · Fine"}
					accentColor={accentColor}
					disabled={disabled}
					indexed={indexed}
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

export const IndividualTouchReleased: Story = {
	render: ({ accentColor, disabled }) => (
		<div style={{ width: 180, height: 420 }}>
			<TouchEncoder
				label="Enc 1 · Dimmer"
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

export const IndividualHardware: Story = {
	render: ({ disabled, showSecondary }) => (
		<div className="hardware-connected" style={{ width: 140, height: 170 }}>
			<HardwareEncoderDisplayView
				slot={1}
				target={{ label: "Pan", value: "20°", role: "Turn" }}
				secondary={
					showSecondary
						? { label: "Tilt", value: "30°", role: "Press-turn" }
						: undefined
				}
				editValue={20}
				canRelease
				onEdit={disabled ? undefined : () => undefined}
				onRelease={() => undefined}
			/>
		</div>
	),
};

function FamilyExample(props: EncoderStoryProps) {
	const [values, setValues] = useState({ pan: 0.4, tilt: 0.6, zoom: 0.25 });
	const [owned, setOwned] = useState(() => new Set(["pan"]));
	const model: EncoderSectionModel = {
		id: props.family.toLowerCase().replace(/\s+/g, "-"),
		label: props.family,
		description: "Configured entirely by the supplied family model",
		encoders: [
			{
				id: "pan",
				slot: 1,
				target: {
					label: "Pan",
					display: owned.has("pan")
						? `${Math.round(values.pan * 100)}%`
						: "Released",
					role: "Turn",
				},
				secondary: props.showSecondary
					? {
							label: "Tilt",
							display: `${Math.round(values.tilt * 100)}%`,
							role: "Press-turn",
						}
					: undefined,
				value: values.pan,
				accentColor: props.accentColor,
				disabled: props.disabled,
				indexed: props.indexed,
				canRelease: owned.has("pan"),
			},
			{
				id: "tilt",
				slot: 2,
				target: {
					label: "Tilt",
					display: props.indexed
						? "Indexed"
						: `${Math.round(values.tilt * 100)}%`,
				},
				value: values.tilt,
				accentColor: props.accentColor,
				disabled: props.disabled,
				indexed: props.indexed,
			},
			{
				id: "zoom",
				slot: 3,
				target: { label: "Zoom", display: `${Math.round(values.zoom * 100)}%` },
				value: values.zoom,
				accentColor: props.accentColor,
				disabled: props.disabled,
			},
			{ id: "empty", slot: 4, value: 0 },
		],
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
};
