import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import {
	CommandLine,
	type CommandLineMode,
	type CommandStatus,
} from "./CommandLine";

const meta = {
	title: "Application/Command line",
	component: CommandLine,
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
	},
	argTypes: {
		mode: { control: "inline-radio", options: ["programmer", "playbacks"] },
		hardware: { table: { disable: true } },
		ready: { control: "boolean" },
		completed: { control: "boolean" },
		commandError: { control: "text" },
		persistentError: { control: "text" },
		preloadArmed: { control: "boolean" },
		preloadActive: { control: "boolean" },
		preloadReady: { control: "boolean" },
	},
	args: {
		mode: "programmer",
		hardware: false,
		ready: true,
		completed: false,
		commandError: null,
		persistentError: null,
		persistentErrorOpen: false,
		commandLine: "FIXTURE 1 AT 68",
		commandTarget: "FIXTURE",
		preloadArmed: false,
		preloadActive: false,
		preloadReady: true,
		preloadLabel: "PRELOAD",
		pendingSummary: "",
		recordState: "ready",
		recordShiftArmed: false,
		history: [],
		historyOpen: false,
		status: {
			connection: "connected",
			frequency: 44,
			timecode: null,
			blackout: false,
		},
		onReplace: () => undefined,
		onExecute: () => undefined,
		onToggleMode: () => undefined,
		onHistoryOpenChange: () => undefined,
		onReuseHistory: () => undefined,
		onOpenStatus: () => undefined,
		onAcknowledgeCommandError: () => undefined,
		onPersistentErrorOpenChange: () => undefined,
		onAcknowledgePersistentError: () => undefined,
		onRecordStart: () => undefined,
		onRecordEnd: () => undefined,
		onRecordCancel: () => undefined,
		onRecordComplete: () => undefined,
		onAdvancePreload: () => undefined,
		onReleasePreload: () => undefined,
	},
} satisfies Meta<typeof CommandLine>;

export default meta;
type Story = StoryObj<typeof meta>;

const history = [
	{
		id: "accepted",
		command: "FIXTURE 1 AT 68",
		status: "accepted" as const,
		feedback: "Applied to Fixture 1.",
		source: "software" as const,
		at: "2026-07-26T12:34:56.000Z",
	},
	{
		id: "rejected",
		command: "GROUP 99 AT FULL",
		status: "rejected" as const,
		feedback: "Group 99 is not stored.",
		source: "osc" as const,
		at: "2026-07-26T12:33:12.000Z",
	},
] as const;

function CommandLineHarness({
	initialMode,
	initialHardware,
	initialCommand,
	initialStatus,
	initialPreloadArmed = false,
	initialPreloadActive = false,
	preloadReady = true,
}: {
	initialMode: CommandLineMode;
	initialHardware: boolean;
	initialCommand: string;
	initialStatus: CommandStatus;
	initialPreloadArmed?: boolean;
	initialPreloadActive?: boolean;
	preloadReady?: boolean;
}) {
	const [mode, setMode] = useState(initialMode);
	const [command, setCommand] = useState(initialCommand);
	const [completed, setCompleted] = useState(false);
	const [historyOpen, setHistoryOpen] = useState(false);
	const [recordArmed, setRecordArmed] = useState(false);
	const [preloadArmed, setPreloadArmed] = useState(initialPreloadArmed);
	const [preloadActive, setPreloadActive] = useState(initialPreloadActive);
	const [notice, setNotice] = useState("Ready");
	return (
		<div
			className={initialHardware ? "hardware-connected" : "touch-connected"}
			style={{ width: "100vw", minWidth: 960, minHeight: 180, padding: 24 }}
		>
			<CommandLine
				mode={mode}
				hardware={initialHardware}
				ready
				completed={completed}
				commandError={null}
				persistentError={null}
				persistentErrorOpen={false}
				commandLine={command}
				commandTarget="FIXTURE"
				preloadArmed={preloadArmed}
				preloadActive={preloadActive || preloadArmed}
				preloadReady={preloadReady}
				preloadLabel={preloadArmed ? "PRELOAD GO" : "PRELOAD"}
				pendingSummary={preloadArmed ? "PROG 4 · GO MINUS 2" : ""}
				recordState={recordArmed ? "record-armed" : "ready"}
				recordShiftArmed={false}
				history={history}
				historyOpen={historyOpen}
				status={initialStatus}
				onReplace={(value) => {
					setCommand(value);
					setCompleted(false);
				}}
				onExecute={() => {
					setCompleted(true);
					setNotice(`Executed ${command}`);
				}}
				onToggleMode={() =>
					setMode((current) =>
						current === "programmer" ? "playbacks" : "programmer",
					)
				}
				onHistoryOpenChange={setHistoryOpen}
				onReuseHistory={(value) => {
					setCommand(value);
					setHistoryOpen(false);
				}}
				onOpenStatus={() => setNotice("Opened running and output controls")}
				onAcknowledgeCommandError={() => undefined}
				onPersistentErrorOpenChange={() => undefined}
				onAcknowledgePersistentError={() => undefined}
				onRecordStart={() => undefined}
				onRecordEnd={() => undefined}
				onRecordCancel={() => undefined}
				onRecordComplete={() => setRecordArmed((armed) => !armed)}
				onAdvancePreload={() => {
					setPreloadArmed((armed) => !armed);
					setPreloadActive(true);
				}}
				onReleasePreload={() => {
					setPreloadArmed(false);
					setPreloadActive(false);
				}}
			/>
			<output
				aria-label="Command line event"
				style={{ display: "block", marginTop: 18, color: "var(--muted)" }}
			>
				{notice}
			</output>
		</div>
	);
}

export const Interactive: Story = {
	args: {},
	render: (args, context) => (
		<CommandLineHarness
			key={`${args.mode}-${context.globals.mode}-${args.preloadArmed}-${args.preloadActive}-${args.preloadReady}`}
			initialMode={args.mode}
			initialHardware={context.globals.mode === "hardware"}
			initialCommand={args.commandLine}
			initialStatus={args.status}
			initialPreloadArmed={args.preloadArmed}
			initialPreloadActive={args.preloadActive}
			preloadReady={args.preloadReady}
		/>
	),
};

export const HardwarePlaybackWithPickupQueue: Story = {
	args: {},
	globals: { mode: "hardware" },
	render: (_args, context) => (
		<CommandLineHarness
			initialMode="playbacks"
			initialHardware={context.globals.mode === "hardware"}
			initialCommand="GO 1"
			initialStatus={{
				connection: "connected",
				frequency: 44,
				timecode: "01:02:03:12",
				blackout: false,
			}}
		/>
	),
};
