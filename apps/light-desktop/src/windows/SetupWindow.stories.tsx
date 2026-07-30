import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type {
	DeskConfiguration,
	FixtureProfile,
	OutputRoute,
	VersionedObject,
} from "../api/types";
import { defaultUpdateSettings } from "../components/control/updateWorkflow";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../components/setup/fixtureProfileModel";
import { fixtureTypeIconAsset } from "../components/setup/fixtureTypeIconAssets";
import { defaultRecordSettings } from "../components/setup/ProgrammerDefaults";
import {
	type DmxDiagnostics,
	DmxDiagnosticsProvider,
} from "../features/dmxDiagnostics/DmxDiagnosticsContext";
import {
	FixtureLibraryProvider,
	type FixtureLibraryState,
} from "../features/fixtureLibrary/FixtureLibraryContext";
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
		{
			source_prefix: "ltc:",
			priority: 10,
			fallback: true,
			loss_timeout_millis: 1000,
		},
		{
			source_prefix: "midi:",
			priority: 20,
			fallback: false,
			loss_timeout_millis: 1000,
		},
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
	highlight_look: {
		intensity: 1,
		color: null,
		iris: null,
		zoom: null,
		focus: null,
		frost: null,
		compatibility: "semantic",
	},
	matter_enabled: false,
	file_manager_system_picker_fallback: false,
	file_manager_roots: [],
};

function versionedRoute(
	id: string,
	body: OutputRoute,
	revision = 3,
): VersionedObject<OutputRoute> {
	return {
		kind: "output_route",
		id,
		body,
		revision,
		updated_at: "2026-07-26T12:00:00Z",
	};
}

export const marketingOutputRoutes = [
	versionedRoute("front-artnet", {
		protocol: "art_net",
		logical_universe: 1,
		destination_universe: 1,
		delivery_mode: "broadcast",
		destination: null,
		enabled: true,
		minimum_slots: 512,
	}),
	versionedRoute("stage-artnet", {
		protocol: "art_net",
		logical_universe: 2,
		destination_universe: 2,
		delivery_mode: "unicast",
		destination: "10.10.20.41:6454",
		enabled: true,
		minimum_slots: 256,
	}),
	versionedRoute("house-sacn", {
		protocol: "sacn",
		logical_universe: 3,
		destination_universe: 101,
		delivery_mode: "multicast",
		destination: null,
		enabled: true,
		minimum_slots: 128,
	}),
	versionedRoute("backup-sacn", {
		protocol: "sacn",
		logical_universe: 4,
		destination_universe: 102,
		delivery_mode: "unicast",
		destination: "10.10.20.52:5568",
		enabled: false,
		minimum_slots: 128,
	}),
] satisfies VersionedObject<OutputRoute>[];

const marketingDmxDiagnostics: DmxDiagnostics = {
	readDmx: async () => ({ revision: 1, universes: [], overrides: [] }),
	setDmxOverride: async () => undefined,
	outputRoutes: marketingOutputRoutes,
	saveOutputRoute: async () => true,
	deleteOutputRoute: async () => true,
};

function marketingProfile(
	id: string,
	manufacturer: string,
	name: string,
	fixtureType: string,
	modes: string[],
	footprint: number,
	physical: [number, number, number],
): FixtureProfile {
	const profile = blankFixtureProfile();
	profile.id = id;
	profile.revision = 4;
	profile.manufacturer = manufacturer;
	profile.name = name;
	profile.short_name = name;
	profile.fixture_type = fixtureType;
	profile.stage_icon_asset = fixtureTypeIconAsset(fixtureType);
	profile.physical.width_millimetres = physical[0];
	profile.physical.height_millimetres = physical[1];
	profile.physical.depth_millimetres = physical[2];
	profile.modes = modes.map((mode, index) => ({
		...structuredClone(profile.modes[0]),
		id: `${id}-mode-${index + 1}`,
		name: mode,
		splits: [{ number: 1, footprint: footprint + index * 4 }],
	}));
	return profile;
}

export const marketingFixtureProfiles = [
	marketingProfile(
		"storybook-fixture-robe-esprite",
		"Robe",
		"ESPRITE",
		"profile moving light",
		["Mode 1", "Mode 2", "Extended"],
		48,
		[408, 856, 494],
	),
	marketingProfile(
		"storybook-fixture-robe-ledbeam",
		"Robe",
		"LEDBeam 350",
		"led wash moving light",
		["Standard", "Extended", "Pixel"],
		24,
		[340, 584, 246],
	),
	marketingProfile(
		"storybook-fixture-etc-lustr",
		"ETC",
		"Source Four LED Series 3 Lustr X8",
		"profile",
		["Direct", "HSI", "RGB", "Studio"],
		16,
		[338, 610, 680],
	),
	marketingProfile(
		"storybook-fixture-astera-titan",
		"Astera",
		"Titan Tube",
		"strip light",
		["RGBW", "RGBW 8 Pixel", "RGBW 16 Pixel"],
		64,
		[42, 1035, 42],
	),
	marketingProfile(
		"storybook-fixture-generic-hazer",
		"Generic",
		"Hazer",
		"hazer",
		["Fog + Fan", "Fog only"],
		2,
		[330, 280, 520],
	),
	marketingProfile(
		"storybook-fixture-claypaky-sharpy",
		"Claypaky",
		"Sharpy",
		"beam moving light",
		["Standard", "Vector"],
		20,
		[330, 485, 410],
	),
	marketingProfile(
		"storybook-fixture-glp-jdc1",
		"GLP",
		"JDC1",
		"strobe",
		["Basic", "Standard", "Segment", "Extended"],
		23,
		[390, 180, 235],
	),
	marketingProfile(
		"storybook-fixture-martin-mac250",
		"Martin",
		"MAC 250 Entour",
		"profile moving light",
		["16 Bit", "16 Bit Extended"],
		18,
		[385, 580, 440],
	),
	marketingProfile(
		"storybook-fixture-chauvet-colorado",
		"CHAUVET Professional",
		"COLORado 1 Solo",
		"led wash",
		["RGBW", "HSIC", "Tour", "Extended"],
		12,
		[222, 236, 252],
	),
	marketingProfile(
		"storybook-fixture-showtec-sunstrip",
		"Showtec",
		"Sunstrip LED RGB",
		"strip light",
		["RGB Pixel"],
		30,
		[1000, 112, 104],
	),
	marketingProfile(
		"storybook-fixture-jb-a7",
		"JB-Lighting",
		"JBLED A7",
		"led wash moving light",
		["Compact", "Standard", "Extended", "Pixel"],
		20,
		[360, 540, 280],
	),
	marketingProfile(
		"storybook-fixture-hes-trackspot",
		"High End Systems",
		"Trackspot",
		"profile moving light",
		["Standard", "Extended"],
		16,
		[305, 610, 395],
	),
] satisfies FixtureProfile[];

const marketingFixtureLibrary: FixtureLibraryState = {
	fixtureLibrary: fixtureDefinitionsFromProfiles(marketingFixtureProfiles),
	fixtureProfiles: marketingFixtureProfiles,
	fixtureProfileWarnings: [],
	patchLayers: [],
	unresolvedMvrFixtures: [],
	savePatchLayer: async () => true,
	saveFixtureProfile: async (profile) => profile,
	deleteFixtureProfile: async () => undefined,
	fixtureProfileRevisions: async () => [],
	saveFixtureProfileSourceGdtf: async () => true,
	importFixturePackage: async () => marketingFixtureProfiles[0],
	exportFixturePackage: async () => new Blob(),
};

export function MarketingSetupWindow({
	initialSection,
	initialFixtureLibraryOpen = false,
}: {
	initialSection: number;
	initialFixtureLibraryOpen?: boolean;
}) {
	const [section, setSection] = useState(initialSection);
	const [draft, setDraft] = useState(configuration);
	const [fixtureLibraryOpen, setFixtureLibraryOpen] = useState(
		initialFixtureLibraryOpen,
	);
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
		fixtureLibraryOpen,
		setDeskLockSettingsOpen: () => undefined,
		setFixtureLibraryOpen,
		updateScreenUndoAvailability: () => undefined,
	} as unknown as SetupWindowController;
	return (
		<ApplicationStateHarness>
			<DmxDiagnosticsProvider diagnostics={marketingDmxDiagnostics}>
				<FixtureLibraryProvider library={marketingFixtureLibrary}>
					<div style={{ height: "100vh", minWidth: 820 }}>
						<ScreensProvider source={screens}>
							<SetupWindowView controller={controller} />
						</ScreensProvider>
					</div>
				</FixtureLibraryProvider>
			</DmxDiagnosticsProvider>
		</ApplicationStateHarness>
	);
}

export function MarketingSetupOutputsWindow() {
	return <MarketingSetupWindow initialSection={3} />;
}

export function MarketingFixtureLibraryWindow() {
	return <MarketingSetupWindow initialSection={0} initialFixtureLibraryOpen />;
}

const meta = {
	title: "ToskLight/Windows/Setup",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	excludeStories: /^(Marketing|marketing)/,
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const Timecode: Story = {
	render: () => <MarketingSetupWindow initialSection={4} />,
};

export const ShowsAndRecovery: Story = {
	render: () => <MarketingSetupWindow initialSection={0} />,
};

export const UsersAndSessions: Story = {
	render: () => <MarketingSetupWindow initialSection={1} />,
};

export const Programmer: Story = {
	render: () => <MarketingSetupWindow initialSection={2} />,
};

export const Outputs: Story = {
	render: () => <MarketingSetupOutputsWindow />,
};

export const OutputsMarketing: Story = {
	render: () => <MarketingSetupOutputsWindow />,
};

export const FixtureLibraryMarketing: Story = {
	render: () => <MarketingFixtureLibraryWindow />,
};

export const NetworkAndInputs: Story = {
	render: () => <MarketingSetupWindow initialSection={5} />,
};

export const ScreensAndPlayback: Story = {
	render: () => <MarketingSetupWindow initialSection={6} />,
};
