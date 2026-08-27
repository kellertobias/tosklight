import { type ReactNode, useState } from "react";
import type { ParameterFamily } from "../../../light-desktop/src/components/control/parameterControls/model";
import { ParameterControlView } from "../../../light-desktop/src/components/control/parameterControls/ParameterControlView";
import type { ParameterController } from "../../../light-desktop/src/components/control/parameterControls/useParameterController";
import { useApp } from "../../../light-desktop/src/state/AppContext";
import { Button } from "../../src";
import {
	type CommandLineMode,
	CommandSection,
	HardwareControlSummaryView,
	PlaybackToolsView,
	type ProgrammerClearState,
	ProgrammerKeypadView,
	type SpeedGroupViewModel,
} from "../../src/command";
import { TouchValueButton, VerticalTouchFaderSurface } from "../../src/faders";
import {
	HardwareCueRowsView,
	PlaybackBankView,
	type PlaybackCardViewModel,
} from "../../src/playback";
import type { SoftwareKey } from "../../src/programmerKeypad";
import { ApplicationStateHarness } from "../providers/ApplicationStateHarness";
import { StoryShowObjectsProvider } from "../providers/StoryShowObjectsProvider";
import { StaticCommandLine } from "./command";

const speedGroups: readonly SpeedGroupViewModel[] = [120, 96, 72, 48, 24].map(
	(bpm, index) => ({
		id: (["A", "B", "C", "D", "E"] as const)[index],
		bpm,
		display: String(bpm),
		active: true,
	}),
);
function ProgrammerSurface({
	hardware,
	initialFamily = "Intensity",
	onValue,
}: {
	hardware: boolean;
	initialFamily?: ParameterFamily;
	onValue?: (attribute: string, value: number) => void;
}) {
	const { state, dispatch } = useApp();
	const [family, setFamily] = useState<ParameterFamily>(initialFamily);
	const [encoderPage, setEncoderPage] = useState(1);
	const [dynamicsMode, setDynamicsMode] = useState(false);
	const [normalized, setNormalized] = useState(
		() =>
			new Map<string, number>([
				["intensity", 0.68],
				["shutter", 0.82],
				["strobe", 0],
				["master", 1],
				["color.red", 0.92],
				["color.green", 0.35],
				["color.blue", 0.64],
				["pan", 0.42],
				["tilt", 0.58],
				["zoom", 0.7],
				["media.folder", 0],
				["media.file", 0],
				["media.mask.folder", 0],
				["media.mask.file", 0],
				["media.mask.invert", 0],
			]),
	);
	const attributes: Record<ParameterFamily, Array<string | null>> = {
		Intensity: ["intensity", "shutter", "strobe", "master"],
		Color: [
			"color.red",
			"color.green",
			"color.blue",
			"color.white",
			"color.amber",
			"color.uv",
		],
		Position: ["pan", "tilt"],
		Beam: ["gobo", "gobo.2", "gobo.rotation", "prism", "prism.2", "iris"],
		Shapers: [
			"shaper.blade.1",
			"shaper.blade.2",
			"shaper.blade.3",
			"shaper.blade.4",
			"shaper.rotation",
		],
		Focus: ["focus", "zoom", "frost", "edge"],
		Control: [
			"control",
			"media.play_mode",
			"media.playback_speed",
			"media.playback_bpm",
			"media.scaling_mode",
		],
		Media: [
			"media.folder",
			"media.file",
			"media.mask.folder",
			"media.mask.file",
			"media.mask.invert",
		],
	};
	const update = (attribute: string, value: number) => {
		setNormalized((current) => new Map(current).set(attribute, value));
		onValue?.(attribute, value);
	};
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
		dynamicsMode,
		setDynamicsMode,
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
		encoderPushTurnSlots: [],
		encoderPageCount: 1,
		attributeLabels: new Map<string, string>(),
		attributeUnits: new Map<string, string | null>(),
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
		applyParameter: async (attribute: string, value: number) =>
			update(attribute, value),
		applyParameterRange: async (attribute: string, values: number[]) =>
			update(attribute, values.at(-1) ?? 0),
		releaseParameter: async (attribute: string) =>
			setNormalized((current) => {
				const next = new Map(current);
				next.delete(attribute);
				return next;
			}),
		stepParameter: async (attribute: string, delta: number) =>
			update(
				attribute,
				Math.max(0, Math.min(1, (normalized.get(attribute) ?? 0) + delta)),
			),
		programmerActions: null,
	} as unknown as ParameterController;
	return <ParameterControlView controller={controller} />;
}

const playbackKinds = [
	"cue-list",
	"group-master",
	"speed-group",
	"dynamic",
	"special-master",
	"special-master",
	"cue-list",
	"empty",
] as const;
const playbackNames = [
	"Opening Sequence",
	"Front Wash",
	"Speed Group A",
	"Circle Dynamic",
	"Playback Fade Time",
	"Grand Master",
	"House Presets",
	"Empty",
] as const;
function playbackModel(
	itemIndex: number,
	row: number,
	hardware: boolean,
	values: readonly number[],
): PlaybackCardViewModel {
	const exampleIndex = itemIndex % playbackKinds.length;
	const assigned = playbackKinds[exampleIndex] !== "empty";
	const value = values[itemIndex];
	const faderRow = row === 1;
	return {
		page: 1,
		slot: itemIndex + 1,
		row,
		rowUnits: faderRow ? (hardware ? 2 : 4) : 1,
		name: playbackNames[exampleIndex],
		assigned,
		kind: playbackKinds[exampleIndex],
		className: faderRow ? undefined : "playback-row-compact",
		hasFader: assigned && faderRow,
		faderValue: value,
		faderLabel: `Playback ${itemIndex + 1}`,
		faderDisplay: `${value}%`,
		faderMode: exampleIndex === 0 ? "Cue 4 · Solo" : undefined,
		summary: assigned
			? {
					label:
						exampleIndex === 0
							? "4 · Solo"
							: exampleIndex === 1
								? "12 Fixtures"
								: playbackNames[exampleIndex],
					detail: exampleIndex === 0 ? "2.5s" : `${value}%`,
					progress: exampleIndex === 0 ? 0.42 : undefined,
				}
			: undefined,
		actions: assigned
			? faderRow
				? [
						{ id: "go-minus", label: "GO −" },
						{ id: "go", label: "GO +" },
						{ id: "flash", label: "FLASH" },
					]
				: [{ id: "go", label: "GO +" }]
			: [],
	};
}

function PlaybackSurface({ hardware }: { hardware: boolean }) {
	const [values, setValues] = useState([
		72, 48, 100, 62, 35, 100, 24, 0, 72, 48, 100, 62, 35, 100, 24, 0,
	]);
	return (
		<PlaybackBankView
			mode={hardware ? "hardware" : "touch"}
			columns={8}
			rowWeights={hardware ? [1, 2] : [1, 4]}
			items={values.map((_, index) => ({
				model: playbackModel(index, Math.floor(index / 8), hardware, values),
				cueRows:
					hardware && index % 8 === 0 ? (
						<HardwareCueRowsView
							previous={{ number: "3", name: "Build" }}
							current={{ number: "4", name: "Solo", fadeMillis: 2500 }}
							next={{ number: "5", name: "Blackout" }}
							progress={0.42}
						/>
					) : undefined,
				callbacks: {
					onFaderChange: (value: number) =>
						setValues((current) =>
							current.map((entry, valueIndex) =>
								valueIndex === index ? value : entry,
							),
						),
				},
			}))}
		/>
	);
}

function ProgrammerToolsFixture({
	clearState,
	previousEnabled,
	nextEnabled,
}: {
	clearState: ProgrammerClearState;
	previousEnabled: boolean;
	nextEnabled: boolean;
}) {
	const [lastKey, setLastKey] = useState("Ready");
	const [activeKeys, setActiveKeys] = useState<SoftwareKey[]>([]);
	const [highlight, setHighlight] = useState(true);
	const press = (key: SoftwareKey) => {
		setLastKey(key);
		if (key !== "SET" && key !== "SHIFT") return;
		setActiveKeys((current) =>
			current.includes(key)
				? current.filter((item) => item !== key)
				: [...current, key],
		);
	};
	return (
		<>
			<ProgrammerKeypadView
				programmerFade={
					<div className="programmer-fade-fader compact">
						<TouchValueButton
							label="Prog. Fade"
							value={3}
							maximum={20}
							display="3.0 s"
						/>
					</div>
				}
				highlightControls={
					<section
						aria-label="Highlight and selection stepping"
						className={`highlight-controls ${highlight ? "active" : ""}`}
					>
						<Button
							active={highlight}
							aria-pressed={highlight}
							className={`highlight-toggle ${highlight ? "highlight-armed" : "highlight-off"}`}
							data-keypad-key="HIGH"
							onClick={() => setHighlight((current) => !current)}
						>
							HIGH
						</Button>
						<Button
							className="highlight-previous"
							data-keypad-key="PREV"
							disabled={!previousEnabled}
						>
							PREV
						</Button>
						<Button
							className="highlight-next"
							data-keypad-key="NEXT"
							disabled={!nextEnabled}
						>
							NEXT
						</Button>
						<Button className="highlight-all" data-keypad-key="ALL">
							ALL
						</Button>
					</section>
				}
				clearState={clearState}
				activeKeys={activeKeys}
				onPress={press}
			/>
			<output className="visually-hidden" aria-label="Last programmer key">
				{lastKey}
			</output>
		</>
	);
}

function PageControlsFixture() {
	const [page, setPage] = useState(1);
	return (
		<div className="playback-page-controls">
			<Button
				aria-label="Previous playback page"
				className="playback-page-chevron"
				onClick={() => setPage((current) => Math.max(1, current - 1))}
			>
				<svg viewBox="0 0 24 24" aria-hidden="true">
					<path d="m5 15 7-7 7 7" />
				</svg>
			</Button>
			<Button className="playback-page-current">
				<span>Page</span>
				<strong>{page}</strong>
				<small>Main</small>
			</Button>
			<Button
				aria-label="Next playback page"
				className="playback-page-chevron"
				onClick={() => setPage((current) => current + 1)}
			>
				<svg viewBox="0 0 24 24" aria-hidden="true">
					<path d="m5 9 7 7 7-7" />
				</svg>
			</Button>
		</div>
	);
}

function FullFader({
	label,
	value,
	maximum,
}: {
	label: string;
	value: number;
	maximum: number;
}) {
	const [current, setCurrent] = useState(value);
	return (
		<div className="programmer-fade-fader full">
			<VerticalTouchFaderSurface
				label={label}
				value={current}
				maximum={maximum}
				display={`${current.toFixed(1)} s`}
				directInput
				hardware={false}
				onChange={setCurrent}
			/>
		</div>
	);
}

function PlaybackToolsFixture() {
	const [setArmed, setSetArmed] = useState(false);
	const [shiftArmed, setShiftArmed] = useState(false);
	return (
		<PlaybackToolsView
			pageControls={<PageControlsFixture />}
			programmerFade={<FullFader label="Prog. Fade" value={3} maximum={20} />}
			cueFade={<FullFader label="Cue Fade" value={2.5} maximum={60} />}
			releaseFade={<FullFader label="Release" value={2} maximum={60} />}
			speedGroups={speedGroups}
			setArmed={setArmed}
			shiftArmed={shiftArmed}
			onCommandKey={(key) => {
				if (key === "SET") setSetArmed((current) => !current);
				if (key === "SHIFT") setShiftArmed((current) => !current);
			}}
		/>
	);
}

function HardwareToolsFixture() {
	const [page, setPage] = useState(1);
	return (
		<HardwareControlSummaryView
			values={[
				{ id: "programmer-fade", label: "Prog Fade", display: "3.0s" },
				{ id: "cue-fade", label: "Cue Fade", display: "2.5s" },
				{ id: "page", label: "Page", display: String(page) },
			]}
			speedGroups={speedGroups}
			onValue={(id) => {
				if (id === "page") setPage((current) => current + 1);
			}}
		/>
	);
}

export interface CommandSectionFixtureProps {
	initialMode?: CommandLineMode;
	hardware?: boolean;
	clearState?: ProgrammerClearState;
	previousEnabled?: boolean;
	nextEnabled?: boolean;
	preloadArmed?: boolean;
	/** Optional production surface used by full-application discussion stories. */
	programmer?: ReactNode;
	/** Reuse an enclosing application provider in full-application stories. */
	inheritAppState?: boolean;
	/** Initial regular Programmer family for full-application review stories. */
	initialProgrammerFamily?: ParameterFamily;
	/** Observe regular Programmer value changes without introducing custom encoders. */
	onProgrammerValue?: (attribute: string, value: number) => void;
}

function CommandSectionFixtureContent({
	initialMode = "programmer",
	hardware = false,
	clearState = "idle",
	previousEnabled = true,
	nextEnabled = true,
	preloadArmed = false,
	programmer,
	initialProgrammerFamily,
	onProgrammerValue,
}: CommandSectionFixtureProps) {
	const [mode, setMode] = useState(initialMode);
	return (
		<CommandSection
			mode={mode}
			hardware={hardware}
			commandLine={
				<StaticCommandLine
					commandLine={
						mode === "programmer" ? "FIXTURE 1 THRU 12 AT 68" : "GO 1"
					}
					hardware={hardware}
					mode={mode}
					preloadArmed={preloadArmed}
					onToggleMode={() =>
						setMode((current) =>
							current === "programmer" ? "playbacks" : "programmer",
						)
					}
				/>
			}
			programmer={
				programmer ?? (
					<ProgrammerSurface
						hardware={hardware}
						initialFamily={initialProgrammerFamily}
						onValue={onProgrammerValue}
					/>
				)
			}
			playbacks={<PlaybackSurface hardware={hardware} />}
			programmerTools={
				<ProgrammerToolsFixture
					clearState={clearState}
					previousEnabled={previousEnabled}
					nextEnabled={nextEnabled}
				/>
			}
			playbackTools={<PlaybackToolsFixture />}
			hardwareTools={<HardwareToolsFixture />}
		/>
	);
}

export function CommandSectionFixture(props: CommandSectionFixtureProps) {
	if (props.inheritAppState) return <CommandSectionFixtureContent {...props} />;
	return (
		<ApplicationStateHarness>
			<StoryShowObjectsProvider>
				<CommandSectionFixtureContent {...props} />
			</StoryShowObjectsProvider>
		</ApplicationStateHarness>
	);
}
