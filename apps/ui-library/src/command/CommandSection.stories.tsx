import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ComponentProps } from "react";
import { CommandSectionFixture } from "../../storybook/fixtures/controlSection";
import { CommandSection } from "./CommandSection";
import type { ProgrammerClearState } from "./CommandSectionTools";

type CommandSectionStoryArgs = ComponentProps<typeof CommandSection> & {
	clearState: ProgrammerClearState;
	previousEnabled: boolean;
	nextEnabled: boolean;
	preloadArmed: boolean;
};

const meta = {
	title: "ToskLight/Command section",
	component: CommandSection,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	argTypes: {
		mode: {
			control: "inline-radio",
			options: ["programmer", "playbacks"],
		},
		hardware: { table: { disable: true } },
		clearState: {
			control: "inline-radio",
			options: ["idle", "selection", "active-values"],
		},
		previousEnabled: { control: "boolean" },
		nextEnabled: { control: "boolean" },
		preloadArmed: { control: "boolean" },
	},
	args: {
		mode: "programmer",
		hardware: false,
		clearState: "idle",
		previousEnabled: true,
		nextEnabled: true,
		preloadArmed: false,
		commandLine: null,
		programmer: null,
		playbacks: null,
		programmerTools: null,
		playbackTools: null,
		hardwareTools: null,
	},
} satisfies Meta<CommandSectionStoryArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

function StorySurface({
	mode,
	hardware,
	settings,
}: {
	mode: "programmer" | "playbacks";
	hardware: boolean;
	settings: Pick<
		CommandSectionStoryArgs,
		"clearState" | "previousEnabled" | "nextEnabled" | "preloadArmed"
	>;
}) {
	return (
		<div
			className="app-shell"
			style={{
				width: "calc(100vw - 48px)",
				height: "calc(100vh - 48px)",
			}}
		>
			<CommandSectionFixture
				key={`${mode}-${hardware}`}
				initialMode={mode}
				hardware={hardware}
				clearState={settings.clearState}
				previousEnabled={settings.previousEnabled}
				nextEnabled={settings.nextEnabled}
				preloadArmed={settings.preloadArmed}
			/>
		</div>
	);
}

export const Configurable: Story = {
	render: (args, context) => (
		<StorySurface
			mode={args.mode}
			hardware={context.globals.mode === "hardware"}
			settings={args as CommandSectionStoryArgs}
		/>
	),
};

export const ProgrammerSoftware: Story = {
	args: { mode: "programmer", hardware: false },
	globals: { mode: "software" },
	render: (args, context) => (
		<StorySurface
			mode={args.mode}
			hardware={context.globals.mode === "hardware"}
			settings={args as CommandSectionStoryArgs}
		/>
	),
};

export const ProgrammerHardwareConnected: Story = {
	args: { mode: "programmer", hardware: true },
	globals: { mode: "hardware" },
	render: (args, context) => (
		<StorySurface
			mode={args.mode}
			hardware={context.globals.mode === "hardware"}
			settings={args as CommandSectionStoryArgs}
		/>
	),
};

export const PlaybacksSoftware: Story = {
	args: { mode: "playbacks", hardware: false },
	globals: { mode: "software" },
	render: (args, context) => (
		<StorySurface
			mode={args.mode}
			hardware={context.globals.mode === "hardware"}
			settings={args as CommandSectionStoryArgs}
		/>
	),
};

export const PlaybacksHardwareConnected: Story = {
	args: { mode: "playbacks", hardware: true },
	globals: { mode: "hardware" },
	render: (args, context) => (
		<StorySurface
			mode={args.mode}
			hardware={context.globals.mode === "hardware"}
			settings={args as CommandSectionStoryArgs}
		/>
	),
};
