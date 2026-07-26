import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type { DeskConfiguration } from "../api/types";
import { defaultUpdateSettings } from "../components/control/updateWorkflow";
import { defaultRecordSettings } from "../components/setup/ProgrammerDefaults";
import { ScreensProvider } from "../features/screens/ScreensContext";
import type { ScreensContextValue } from "../features/screens/types";
import { SetupWindowView } from "./SetupWindow";
import type { SetupWindowController } from "./setupWindow/controller";

const screens: ScreensContextValue = {
	screens: null,
	bootstrap: null,
	session: null,
	saveScreen: async () => undefined,
	deleteScreen: async () => undefined,
	setScreenPage: async () => undefined,
	updateControlDesk: async () => undefined,
	selectControlDesk: () => undefined,
	removeClient: async () => false,
};

const configuration: DeskConfiguration = {
	frame_rate_hz: 44,
	output_bind_ip: "0.0.0.0",
	osc_bind: "0.0.0.0:8000",
	art_timecode_bind: null,
	midi_inputs: ["ToskLight MIDI"],
	rtp_midi_bind: null,
	timecode_sources: [
		{ source_prefix: "ltc:", priority: 10, fallback: true, loss_timeout_millis: 1000 },
		{ source_prefix: "midi:", priority: 20, fallback: false, loss_timeout_millis: 1000 },
	],
	osc_timecode: null,
	backup_retention: 20,
	autosave_interval_seconds: 30,
	speed_groups_bpm: [120, 120, 120, 120, 120],
	programmer_fade_millis: 500,
	command_line_at_uses_programmer_fade: true,
	sequence_master_fade_millis: 500,
	preload_programmer_changes: true,
	preload_physical_playback_actions: true,
	preload_virtual_playback_actions: true,
	patch_preview_highlight_dmx: false,
	matter_enabled: false,
	file_manager_system_picker_fallback: false,
	file_manager_roots: [],
};

function SetupStory({ initialSection }: { initialSection: number }) {
	const [section, setSection] = useState(initialSection);
	const [draft, setDraft] = useState(configuration);
	const controller = {
		section,
		setSection,
		restartRequired: false,
		draft,
		editDraft: setDraft,
		recordSettings: defaultRecordSettings,
		setRecordSettings: () => undefined,
		updateSettings: defaultUpdateSettings,
		setUpdateSettings: () => undefined,
		programmerSettingsError: null,
		programmerSettingsLoaded: true,
		serverUrl: "http://127.0.0.1:5000",
		setServerUrl: () => undefined,
		applyServerUrl: () => undefined,
		screenCanUndo: false,
		screenUndo: useRef<(() => void) | null>(null),
		save: async () => undefined,
		deskLockSettingsOpen: false,
		fixtureLibraryOpen: false,
		setDeskLockSettingsOpen: () => undefined,
		setFixtureLibraryOpen: () => undefined,
		updateScreenUndoAvailability: () => undefined,
	} as unknown as SetupWindowController;
	return (
		<ApplicationStateHarness>
			<div style={{ height: "100vh", minWidth: 820 }}>
				<ScreensProvider source={screens}>
					<SetupWindowView controller={controller} />
				</ScreensProvider>
			</div>
		</ApplicationStateHarness>
	);
}

const meta = {
	title: "Application/Windows/Setup",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Timecode: Story = {
	render: () => <SetupStory initialSection={4} />,
};

export const ShowsAndRecovery: Story = {
	render: () => <SetupStory initialSection={0} />,
};

export const UsersAndSessions: Story = {
	render: () => <SetupStory initialSection={1} />,
};

export const Programmer: Story = {
	render: () => <SetupStory initialSection={2} />,
};

export const Outputs: Story = {
	render: () => <SetupStory initialSection={3} />,
};

export const NetworkAndInputs: Story = {
	render: () => <SetupStory initialSection={5} />,
};

export const ScreensAndPlayback: Story = {
	render: () => <SetupStory initialSection={6} />,
};
