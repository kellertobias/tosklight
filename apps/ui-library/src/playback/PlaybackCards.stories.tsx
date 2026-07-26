import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
	HardwareCueRowsView,
	PlaybackBankView,
	type PlaybackCardViewModel,
} from "../playback";

interface PlaybackStoryProps {
	mode: "touch" | "hardware";
	columns: number;
	selectedSlot: number;
	showEmpty: boolean;
	state:
		| "normal"
		| "running"
		| "loaded"
		| "pickup"
		| "bump"
		| "held-flash"
		| "held-swap"
		| "levels"
		| "long";
}

const meta: Meta<PlaybackStoryProps> = {
	title: "Playbacks/Playback bank",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		mode: { control: "inline-radio", options: ["touch", "hardware"] },
		columns: { control: { type: "range", min: 1, max: 4, step: 1 } },
		selectedSlot: { control: { type: "range", min: 1, max: 4, step: 1 } },
		showEmpty: { control: "boolean" },
	},
	args: {
		mode: "touch",
		columns: 4,
		selectedSlot: 1,
		showEmpty: true,
		state: "normal",
	},
};

export default meta;
type Story = StoryObj<PlaybackStoryProps>;

function model(
	slot: number,
	value: number,
	selectedSlot: number,
	assigned = true,
	state: PlaybackStoryProps["state"] = "normal",
): PlaybackCardViewModel {
	const isBump = state === "bump" && slot === 3;
	const hasFader = !isBump;
	const exceptional = slot === 1;
	const status = exceptional
		? state === "loaded"
			? { kind: "loaded" as const, label: "LOADED" }
			: state === "held-flash"
					? { kind: "flash" as const, label: "FLASH HELD" }
					: state === "held-swap"
						? { kind: "swap" as const, label: "SWAP HELD" }
						: undefined
		: undefined;
	return {
		page: 2,
		slot,
		row: 0,
		rowUnits: slot === 1 ? 4 : 2,
		name: assigned
			? state === "long" && slot === 1
				? "A Deliberately Long Production Playback Title"
				: ["Main Cuelist", "Front Wash", "Bump", "Page Chase"][slot - 1]
			: "Empty",
		assigned,
		selected: slot === selectedSlot,
		className: assigned
			? `playback-colored ${exceptional && state === "running" ? "running" : ""} ${exceptional && state === "loaded" ? "loaded" : ""} ${exceptional && state === "held-swap" ? "swap-active" : ""} ${slot === selectedSlot ? "selected" : ""}`
			: "empty",
		color: assigned
			? ["#176777", "#925ad1", "#d98236", "#2874bd"][slot - 1]
			: undefined,
		hasFader,
		faderValue: value,
		faderLabel: `Playback ${slot}`,
		faderDisplay: `${Math.round(value)}%`,
		faderMode:
			slot === 1
				? state === "long"
					? "Cue 104 · A deliberately long production cue name"
					: "Cue 4 · Solo"
				: undefined,
		hardwarePickup:
			exceptional && state === "pickup"
				? { physicalPosition: value / 100, pickupTarget: 0 }
				: undefined,
		status,
		actions: assigned
			? isBump
				? [{ id: "flash", label: "FLASH" }]
				: [
						{ id: "go", label: state === "long" ? "SELECT CONTENTS" : "GO +" },
						{ id: "off", label: "OFF" },
						{ id: "flash", label: state === "held-swap" ? "SWAP" : "FLASH" },
					]
			: [],
	};
}

function BankExample({
	mode,
	columns,
	selectedSlot,
	showEmpty,
	state,
}: PlaybackStoryProps) {
	const [values, setValues] = useState<Record<number, number>>({
		1: state === "levels" ? 0 : 62,
		2: state === "levels" ? 50 : 85,
		3: state === "levels" ? 100 : 0,
		4: 0,
	});
	const assignedCount = showEmpty ? Math.min(columns, 3) : columns;
	const items = Array.from({ length: columns }, (_, index) => {
		const slot = index + 1;
		const assigned = slot <= assignedCount;
		return {
			model: model(slot, values[slot] ?? 0, selectedSlot, assigned, state),
			callbacks: {
				onFaderChange: (value: number) =>
					setValues((current) => ({ ...current, [slot]: value })),
			},
			cueRows:
				slot === 1 ? (
					<HardwareCueRowsView
						previous={{ number: 3, name: "Look" }}
						current={{ number: 4, name: "Solo", fadeMillis: 2500 }}
						next={{
							number: 5,
							name:
								state === "long"
									? "Blackout with a deliberately long cue name"
									: "Blackout",
						}}
						nextLoaded={state === "loaded"}
						progress={0.42}
					/>
				) : undefined,
			group:
				slot === 2
					? { name: "Front Wash", master: `${values[slot]}%` }
					: undefined,
		};
	});
	return (
		<div
			style={{
				width: Math.max(420, columns * 180),
				height: mode === "touch" ? 560 : 290,
			}}
		>
			<PlaybackBankView mode={mode} items={items} />
		</div>
	);
}

export const DeterministicBank: Story = {
	render: (args) => <BankExample {...args} />,
};

export const TouchBank: Story = {
	args: { mode: "touch", columns: 3 },
	render: (args) => <BankExample {...args} />,
};

export const HardwareBank: Story = {
	args: { mode: "hardware", columns: 3 },
	render: (args) => <BankExample {...args} />,
};

export const RunningWithGradient: Story = {
	args: { state: "running", columns: 1, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const LoadedNext: Story = {
	args: { state: "loaded", columns: 1, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const PickupRequired: Story = {
	args: {
		mode: "hardware",
		state: "pickup",
		columns: 1,
		showEmpty: false,
	},
	render: (args) => <BankExample {...args} />,
};

function PickupExample({
	initialPhysical,
	target,
	mode = "hardware",
	faderless = false,
	multiple = false,
}: {
	initialPhysical: number;
	target: number;
	mode?: "touch" | "hardware";
	faderless?: boolean;
	multiple?: boolean;
}) {
	const [physical, setPhysical] = useState(initialPhysical);
	const [authority, setAuthority] = useState<
		"hardware" | "replacement" | "disconnected"
	>(
		"hardware",
	);
	const direction = target - initialPhysical;
	const satisfied =
		authority !== "hardware" ||
		(direction >= 0 ? physical >= target : physical <= target);
	const slots = multiple ? [1, 2, 3] : [1];
	const items = slots.map((slot) => {
		const pickup = slot === 2 || !multiple;
		const itemModel: PlaybackCardViewModel = {
			...model(slot, slot === 1 ? physical * 100 : 50, 1),
			hasFader: !faderless,
			hardwarePickup:
				pickup && !satisfied
					? { physicalPosition: physical, pickupTarget: target }
					: undefined,
		};
		return {
			model: itemModel,
			callbacks: {
				onFaderChange: (value: number) => setPhysical(value / 100),
			},
		};
	});
	return (
		<div style={{ width: multiple ? 560 : 220, minHeight: 300 }}>
			<button type="button" onClick={() => setAuthority("replacement")}>
				Replace hardware authority
			</button>
			<button type="button" onClick={() => setAuthority("disconnected")}>
				Disconnect hardware
			</button>
			<button type="button" onClick={() => setAuthority("hardware")}>
				Reconnect hardware
			</button>
			<PlaybackBankView mode={mode} items={items} />
		</div>
	);
}

export const PickupRaise: Story = {
	render: () => <PickupExample initialPhysical={0.5} target={0.75} />,
};

export const PickupLower: Story = {
	render: () => <PickupExample initialPhysical={0.75} target={0.5} />,
};

export const PickupLowerToZero: Story = {
	render: () => <PickupExample initialPhysical={0.75} target={0} />,
};

export const PickupSatisfied: Story = {
	render: () => <PickupExample initialPhysical={0.5} target={0.5} />,
};

export const PickupBoundaries: Story = {
	render: () => <PickupExample initialPhysical={0} target={1} />,
};

export const PickupApproachAndReleaseFromBelow: Story = {
	render: () => <PickupExample initialPhysical={0.15} target={0.7} />,
};

export const PickupApproachAndReleaseFromAbove: Story = {
	render: () => <PickupExample initialPhysical={0.9} target={0.25} />,
};

export const PickupAuthorityReplacement: Story = {
	render: () => <PickupExample initialPhysical={0.8} target={0} />,
};

export const PickupHardwareDisconnectedAndReconnected: Story = {
	render: () => <PickupExample initialPhysical={0.5} target={0.75} />,
};

export const OnlyOneOfMultipleFadersRequiresPickup: Story = {
	render: () => (
		<PickupExample initialPhysical={0.8} target={0} multiple={true} />
	),
};

export const TouchNeverShowsPickup: Story = {
	render: () => (
		<PickupExample initialPhysical={0.8} target={0} mode="touch" />
	),
};

export const FaderlessNeverShowsPickup: Story = {
	render: () => (
		<PickupExample initialPhysical={0.8} target={0} faderless={true} />
	),
};

export const FaderlessBump: Story = {
	args: { state: "bump", columns: 3, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const HeldFlash: Story = {
	args: { state: "held-flash", columns: 1, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const HeldSwap: Story = {
	args: { mode: "hardware", state: "held-swap", columns: 1, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const ZeroMidFullLevels: Story = {
	args: { state: "levels", columns: 3, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const LongLabels: Story = {
	args: { mode: "hardware", state: "long", columns: 1, showEmpty: false },
	render: (args) => <BankExample {...args} />,
};

export const EmptyPlayback: Story = {
	args: { columns: 4, showEmpty: true },
	render: (args) => <BankExample {...args} />,
};
