import { StageRendererView } from "../windows/stageWindow/StageRendererView";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import {
	HardwareCueRowsView,
	PlaybackBankView,
	type PlaybackCardViewModel,
} from "@tosklight/ui/playback";
import { useState } from "react";
import {
	stageLayout,
	stageOptions,
	stagePresentations,
	stageSelection,
} from "../../../ui-library/storybook/fixtures/application";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import {
	dmxOutputHealth,
	dmxPatchedFixtures,
	dmxSnapshot,
} from "../../../ui-library/storybook/fixtures/dmx";
import {
	helpCatalog,
	helpQuickStartId,
	helpQuickStartTopic,
} from "../../../ui-library/storybook/fixtures/help";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { useApp } from "../state/AppContext";
import { DmxWindowView } from "../windows/DmxWindow";
import { HelpWindowView } from "../windows/HelpWindow";
import { NumericPad } from "./control/NumericPad";
import { ProgrammerFadeFader } from "./control/ProgrammerFadeFader";
import type { ParameterFamily } from "./control/parameterControls/model";
import { ParameterControlView } from "./control/parameterControls/ParameterControlView";
import type { ParameterController } from "./control/parameterControls/useParameterController";
import { LeftDock } from "./shell/LeftDock";

const meta = {
	title: "ToskLight/Shell and control",
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const desktops = [{ type: "SET_DOCK_MODE", mode: "desks" }] as const;
const builtIns = [
	{ type: "SET_DOCK_MODE", mode: "builtins" },
	{ type: "OPEN_BUILTIN", kind: "stage" },
] as const;
const hardwareMode = [{ type: "SET_MIDI_PROFILE", value: true }] as const;

function ShellFrame({ children }: { children: React.ReactNode }) {
	return (
		<div className="app-shell" style={{ width: 1496, height: 761 }}>
			{children}
		</div>
	);
}

export const DockDesktops: Story = {
	render: () => (
		<ApplicationStateHarness actions={desktops}>
			<ShellFrame>
				<LeftDock />
			</ShellFrame>
		</ApplicationStateHarness>
	),
};

export const DockBuiltIns: Story = {
	render: () => (
		<ApplicationStateHarness actions={builtIns}>
			<ShellFrame>
				<LeftDock />
			</ShellFrame>
		</ApplicationStateHarness>
	),
};

function ControlStory({
	mode,
	hardware = false,
}: {
	mode: "programmer" | "playbacks";
	hardware?: boolean;
}) {
	return (
		<ApplicationStateHarness actions={hardware ? hardwareMode : []}>
			<ShellFrame>
				<CommandSectionFixture initialMode={mode} hardware={hardware} />
			</ShellFrame>
		</ApplicationStateHarness>
	);
}

export const ProgrammerSoftware: Story = {
	render: () => <ControlStory mode="programmer" />,
};

export const ProgrammerHardwareConnected: Story = {
	render: () => <ControlStory hardware mode="programmer" />,
};

export const PlaybacksSoftware: Story = {
	render: () => <ControlStory mode="playbacks" />,
};

export const PlaybacksHardwareConnected: Story = {
	render: () => <ControlStory hardware mode="playbacks" />,
};

function ParameterFamiliesExample({
	hardware = false,
}: {
	hardware?: boolean;
}) {
	const { state, dispatch } = useApp();
	const [family, setFamily] = useState<ParameterFamily>("Intensity");
	const [encoderPage, setEncoderPage] = useState(1);
	const attributes: Record<ParameterFamily, Array<string | null>> = {
		Intensity: ["intensity", "shutter", "strobe", "master", null, null],
		Color: [
			"color.red",
			"color.green",
			"color.blue",
			"color.white",
			"color.amber",
			"color.uv",
		],
		Position: ["pan", "tilt", null, null, null, null],
		Beam: ["gobo", "gobo.2", "gobo.rotation", "prism", "prism.2", "iris"],
		Shapers: [
			"shaper.blade.1",
			"shaper.blade.2",
			"shaper.blade.3",
			"shaper.blade.4",
			"shaper.rotation",
			null,
		],
		Focus: ["focus", "zoom", "frost", "edge", null, null],
		Control: [
			"control",
			"media.play_mode",
			"media.playback_speed",
			"media.playback_bpm",
			"media.scaling_mode",
			null,
		],
		Media: [
			"media.folder",
			"media.file",
			"media.mask.folder",
			"media.mask.file",
			"media.mask.invert",
			null,
		],
	};
	const normalized = new Map([
		["intensity", 0.68],
		["color.red", 0.92],
		["color.green", 0.35],
		["color.blue", 0.64],
		["pan", 0.42],
		["tilt", 0.58],
		["zoom", 0.7],
	]);
	const controller = {
		state,
		dispatch,
		family,
		setFamily,
		encoderGroups: Object.keys(attributes).map((name) => ({
			id: name.toLowerCase(),
			label: name,
			pages: [{ number: 1, slots: [] }],
		})),
		encoderPage,
		selectEncoderGroup: (next: ParameterFamily, page: number) => {
			setFamily(next);
			setEncoderPage(page);
		},
		alignMode: null,
		setAlignMode: () => undefined,
		dynamicsMode: false,
		setDynamicsMode: () => undefined,
		hardwareConnected: hardware,
		selectedFixtureIds: ["front-left", "front-right"],
		selectedFixtures: [],
		selectionRevision: 1,
		selectedGroupId: null,
		programmerValuesRoute: "normal",
		programmerValuesReady: true,
		programmerValues: [],
		groupProgrammerValues: [],
		encoderSlots: attributes[family],
		encoderPageCount: 1,
		attributeLabels: new Map<string, string>(),
		normalized,
		normalizedByFixture: new Map<string, Map<string, number>>(),
		discrete: new Map<string, string>(),
		discreteByFixture: new Map<string, Map<string, string>>(),
		programmerTarget: (attribute: string) => normalized.get(attribute),
		programmerDiscreteTarget: () => undefined,
		encoderNormalizedDisplay: (attribute: string) =>
			normalized.has(attribute)
				? `${Math.round((normalized.get(attribute) ?? 0) * 100)}%`
				: undefined,
		encoderDiscreteDisplay: () => undefined,
		encoderSemanticDisplay: () => undefined,
		hasProgrammerValue: (attribute: string) => normalized.has(attribute),
		canWriteValues: true,
		applyParameter: async () => undefined,
		applyParameterRange: async () => undefined,
		releaseParameter: async () => undefined,
		stepParameter: async () => undefined,
		programmerActions: null,
	} as unknown as ParameterController;
	return (
		<div style={{ width: 1220, height: 360 }}>
			<ParameterControlView controller={controller} />
		</div>
	);
}

export const ParameterFamiliesAndTouchEncoders: Story = {
	render: () => (
		<ApplicationStateHarness>
			<ParameterFamiliesExample />
		</ApplicationStateHarness>
	),
};

export const ParameterFamiliesAndHardwareEncoders: Story = {
	render: () => (
		<ApplicationStateHarness actions={hardwareMode}>
			<ParameterFamiliesExample hardware />
		</ApplicationStateHarness>
	),
};

export const KeypadProgrammerFadePreloadHighlightAndStep: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div className="control-right-pane" style={{ width: 460, height: 620 }}>
				<div className="control-right-main">
					<ProgrammerFadeFader compact />
					<NumericPad demo />
				</div>
			</div>
		</ApplicationStateHarness>
	),
};

function playbackModel(slot: number, hardware: boolean): PlaybackCardViewModel {
	const values = [72, 48, 100, 0];
	const kinds = [
		"cue-list",
		"group-master",
		"special-master",
		"empty",
	] as const;
	return {
		page: 1,
		slot,
		row: 0,
		rowUnits: hardware ? 2 : 4,
		name: ["Opening Sequence", "Front Wash", "Bump", "Empty"][slot - 1],
		assigned: slot < 4,
		kind: kinds[slot - 1],
		selected: slot === 1,
		className:
			slot < 4
				? `playback-colored ${slot === 1 ? "running selected" : "loaded"}`
				: "empty",
		hasFader: slot < 4 && slot !== 3,
		faderValue: values[slot - 1],
		faderLabel: `Playback ${slot}`,
		faderDisplay: `${values[slot - 1]}%`,
		faderMode: slot === 1 ? "Cue 4 · Solo" : undefined,
		summary:
			slot === 1
				? { label: "4 · Solo", detail: "2.5s", progress: 0.42 }
				: slot === 2
					? { label: "12 Fixtures", detail: "48%" }
					: slot === 3
						? { label: "Special master", detail: "100%" }
						: undefined,
		actions:
			slot < 4
				? [
						{ id: "go", label: "GO" },
						{ id: "off", label: "OFF" },
						{ id: "flash", label: "FLASH" },
					]
				: [],
	};
}

function PlaybackBankExample({ mode }: { mode: "touch" | "hardware" }) {
	return (
		<div style={{ width: 920, height: mode === "touch" ? 520 : 300 }}>
			<PlaybackBankView
				mode={mode}
				items={Array.from({ length: 4 }, (_, index) => ({
					model: playbackModel(index + 1, mode === "hardware"),
					cueRows:
						index === 0 ? (
							<HardwareCueRowsView
								previous={{ number: "3", name: "Build" }}
								current={{ number: "4", name: "Solo", fadeMillis: 2500 }}
								next={{ number: "5", name: "Blackout" }}
								progress={0.42}
							/>
						) : undefined,
				}))}
			/>
		</div>
	);
}

export const PlaybackBankTouch: Story = {
	render: () => <PlaybackBankExample mode="touch" />,
};

export const PlaybackBankHardware: Story = {
	render: () => <PlaybackBankExample mode="hardware" />,
};

function HelpPane() {
	const [query, setQuery] = useState("");
	return (
		<HelpWindowView
			catalog={helpCatalog}
			defaultExpanded={["30-Programmer/index.md"]}
			onQueryChange={setQuery}
			onSelect={() => undefined}
			query={query}
			selected={helpQuickStartId}
			topic={helpQuickStartTopic}
			urlTransform={() => undefined}
		/>
	);
}

export const RepresentativeDesktopArrangement: Story = {
	render: () => (
		<ApplicationStateHarness>
			<div style={{ width: 1496, height: 761 }}>
				<GridDesktop id="application-story" name="Programming">
					<PaneView
						pane={{
							id: "stage",
							title: "Stage",
							type: "stage",
							x: 1,
							y: 1,
							width: 14,
							height: 11,
						}}
					>
						<StageRendererView options={stageOptions} selection={stageSelection} />
					</PaneView>
					<PaneView
						pane={{
							id: "dmx",
							title: "DMX",
							type: "dmx",
							x: 15,
							y: 1,
							width: 10,
							height: 11,
						}}
					>
						<DmxWindowView
							compact
							dotSize="small"
							onDotSizeChange={() => undefined}
							onSetDmxOverride={() => undefined}
							outputHealth={dmxOutputHealth}
							outputRoutes={[]}
							patchedFixtures={dmxPatchedFixtures}
							snapshot={dmxSnapshot}
						/>
					</PaneView>
					<PaneView
						pane={{
							id: "help",
							title: "Help",
							type: "help",
							x: 1,
							y: 12,
							width: 24,
							height: 7,
						}}
					>
						<HelpPane />
					</PaneView>
				</GridDesktop>
			</div>
		</ApplicationStateHarness>
	),
};
