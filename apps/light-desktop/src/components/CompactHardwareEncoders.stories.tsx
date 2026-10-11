import type { Meta, StoryObj } from "@storybook/react-vite";
import { useRef, useState } from "react";
import { HardwareControlSummaryView } from "@tosklight/ui/command";
import { ModalNumberEditor } from "@tosklight/ui/input";
import { EncoderGroupTabs } from "@tosklight/ui/encoders";
import {
	CommandSectionFixture,
	ProgrammerSurface,
} from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { StoryShowObjectsProvider } from "../../../ui-library/storybook/providers/StoryShowObjectsProvider";
import {
	compactFamilyLabels,
	type ParameterFamily,
} from "./control/parameterControls/model";
import { ParameterControlView } from "./control/parameterControls/ParameterControlView";
import type { ParameterController } from "./control/parameterControls/useParameterController";
import "./compactHardwareEncoders.story.css";

type EncoderCount = 4 | 5 | 6;
type Assignment = {
	attribute: string;
	label: string;
	value: string;
	detail?: string;
};

// Explicit example readouts: ordered spreads, physical units, discrete values and unknowns.
// These are presentation fixtures, not a simulated authoritative desk or hardware binding.
const assignments: Record<ParameterFamily, Assignment[]> = {
	Intensity: [
		{ attribute: "intensity", label: "Dimmer", value: "20 → 80%", detail: "Ordered spread" },
		{ attribute: "shutter", label: "Shutter", value: "Open" },
		{ attribute: "strobe", label: "Strobe", value: "0%" },
		{ attribute: "master", label: "Master", value: "100%" },
	],
	Color: [
		{ attribute: "color.red", label: "Red", value: "15 → 85%", detail: "Ordered spread" },
		{ attribute: "color.green", label: "Green", value: "35%" },
		{ attribute: "color.blue", label: "Blue", value: "64%" },
		{ attribute: "color.white", label: "White blend", value: "0%" },
		{ attribute: "color.temperature", label: "Temp", value: "3200 K" },
		{ attribute: "color.duv", label: "Duv", value: "+0.002" },
		{ attribute: "color.amber", label: "Amber", value: "—", detail: "No value" },
		{ attribute: "color.uv", label: "UV", value: "10%" },
	],
	Position: [
		{ attribute: "pan", label: "Pan", value: "−45 → 45°", detail: "Ordered spread" },
		{ attribute: "tilt", label: "Tilt", value: "28°" },
		{ attribute: "point", label: "Point", value: "DS centre" },
		{ attribute: "position.x", label: "X offset", value: "−2 → 2 m", detail: "Ordered spread" },
		{ attribute: "position.y", label: "Y offset", value: "0 m" },
		{ attribute: "position.z", label: "Z offset", value: "1.5 m" },
	],
	Beam: [
		{ attribute: "gobo", label: "Gobo 1", value: "Breakup" },
		{ attribute: "gobo.2", label: "Gobo 2", value: "Open" },
		{ attribute: "gobo.rotation", label: "Gobo rotate", value: "Mixed", detail: "CW / CCW" },
		{ attribute: "prism", label: "Prism 1", value: "Off" },
		{ attribute: "prism.2", label: "Prism 2", value: "Off" },
		{ attribute: "iris", label: "Iris", value: "30 → 70%", detail: "Ordered spread" },
	],
	Shapers: [
		{ attribute: "shaper.blade.1", label: "Blade 1", value: "20%" },
		{ attribute: "shaper.blade.2", label: "Blade 2", value: "40%" },
		{ attribute: "shaper.blade.3", label: "Blade 3", value: "20%" },
		{ attribute: "shaper.blade.4", label: "Blade 4", value: "40%" },
		{ attribute: "shaper.rotation", label: "Frame rotate", value: "−15°" },
	],
	Focus: [
		{ attribute: "focus", label: "Focus", value: "55%" },
		{ attribute: "zoom", label: "Zoom", value: "12 → 36°", detail: "Ordered spread" },
		{ attribute: "frost", label: "Frost", value: "0%" },
		{ attribute: "edge", label: "Edge", value: "—", detail: "No value" },
	],
	Control: [
		{ attribute: "control", label: "Control", value: "Normal" },
		{ attribute: "media.play_mode", label: "Play mode", value: "Loop" },
		{ attribute: "media.playback_speed", label: "Speed", value: "100%" },
		{ attribute: "media.playback_bpm", label: "Tempo", value: "120 BPM" },
		{ attribute: "media.scaling_mode", label: "Scaling", value: "Fit" },
	],
	Media: [
		{ attribute: "media.folder", label: "Folder", value: "Looks" },
		{ attribute: "media.file", label: "File", value: "Clouds" },
		{ attribute: "media.mask.folder", label: "Mask folder", value: "Masks" },
		{ attribute: "media.mask.file", label: "Mask file", value: "Circle" },
		{ attribute: "media.mask.invert", label: "Invert", value: "Off" },
	],
};

function assignmentPages(family: ParameterFamily, count: EncoderCount): Assignment[][] {
	const slots = assignments[family];
	// The established Position contract keeps Angles and Point/XYZ on separate pages.
	if (family === "Position") return [slots.slice(0, 2), slots.slice(2)];
	return Array.from({ length: Math.ceil(slots.length / count) }, (_, page) =>
		slots.slice(page * count, (page + 1) * count));
}

/** Local presentation controls: one shared Fade value, page input and tap-tempo groups. */
function CompactTimingAndSpeed() {
	const [fade, setFade] = useState(3);
	const [page, setPage] = useState(1);
	const [bpms, setBpms] = useState([120, 96, 72, 48, 24]);
	const taps = useRef(new Map<string, number>());
	const [editing, setEditing] = useState<"fade" | "page" | null>(null);
	const [input, setInput] = useState("");
	const [error, setError] = useState<string | undefined>();
	return (
		<HardwareControlSummaryView
			values={[
				{ id: "programmer-fade", label: "Fade", display: `${fade.toFixed(1)}s`, ariaLabel: `Fade ${fade.toFixed(1)} seconds` },
				{ id: "page", label: "Page", display: String(page), ariaLabel: `Playback page ${page}` },
			]}
			speedGroups={bpms.map((bpm, index) => ({
				id: (["A", "B", "C", "D", "E"] as const)[index], bpm, display: String(bpm), active: true,
			}))}
			onValue={id => {
				const next = id === "page" ? "page" : "fade";
				setEditing(next);
				setInput(String(next === "page" ? page : fade));
				setError(undefined);
			}}
			onSpeedActivate={id => {
				const now = performance.now();
				const previous = taps.current.get(id);
				taps.current.set(id, now);
				if (previous === undefined) return;
				const interval = now - previous;
				if (interval < 200 || interval > 3000) return;
				setBpms(current => current.map((bpm, index) =>
					index === "ABCDE".indexOf(id) ? Math.round(60000 / interval) : bpm));
			}}
			overlays={editing && (
				<ModalNumberEditor ariaLabel={editing === "fade" ? "Shared fade time" : "Playback page"}
					title={editing === "fade" ? "Fade time" : "Playback page"}
					value={input} onChange={setInput} error={error}
					allowDecimal={editing === "fade"}
					onClose={() => setEditing(null)}
					onSubmit={raw => {
						const value = Number(raw ?? input);
						if (!Number.isFinite(value) || (raw ?? input).trim() === "" ||
							(editing === "fade" ? value < 0 || value > 20 : value < 1 || value > 999 || !Number.isInteger(value))) {
							setError(editing === "fade" ? "Enter a fade from 0 to 20 seconds." : "Enter a whole page from 1 to 999.");
							return;
						}
						if (editing === "fade") setFade(value); else setPage(value);
						setEditing(null);
					}}
				/>
			)}
		/>
	);
}

/** Read-only encoder miniature; only group/page buttons have interaction. */
function CompactEncoderSummary({ controller, slots }: {
	controller: ParameterController;
	slots: Assignment[];
}) {
	return (
		<section className="compact-encoder-summary" aria-label="Hardware times, speed groups and encoders">
			<CompactTimingAndSpeed />
			<nav aria-label="Compact encoder groups and pages">
				<EncoderGroupTabs
					className="compact-encoder-groups"
					groups={controller.encoderGroups.map(group => ({
						id: group.label as ParameterFamily,
						label: group.label,
						compactLabel: compactFamilyLabels[group.label as ParameterFamily],
						pageCount: group.pages.length,
					}))}
					activeGroup={controller.family}
					page={controller.encoderPage}
					onChange={controller.selectEncoderGroup}
				/>
			</nav>
			<div className="compact-encoder-readouts"
				style={{ gridTemplateColumns: `repeat(${controller.visibleEncoderCount}, minmax(0, 1fr))` }}
				aria-label={`${controller.family} page ${controller.encoderPage} encoder assignments`}>
				{Array.from({ length: controller.visibleEncoderCount }, (_, index) => {
					const slot = slots[index];
					const label = slot?.label ?? "Unassigned";
					const value = slot?.value ?? "—";
					return (
						<div key={index} className={`compact-encoder-readout ${slot ? "assigned" : "unassigned"}`}
							aria-label={`Encoder ${index + 1}: ${label}, ${value}${slot?.detail ? `, ${slot.detail}` : ""}`}>
							<header><small>{index + 1}</small><span title={label}>{label}</span></header>
							<strong>{value.replaceAll(" → ", "→")}</strong>
							{slot?.detail && <em>{slot.detail === "Ordered spread" ? "Spread" : slot.detail}</em>}
						</div>
					);
				})}
			</div>
		</section>
	);
}

function PagedDesk({ base, count }: { base: ParameterController; count: EncoderCount }) {
	const pages = assignmentPages(base.family, count);
	const page = Math.min(base.encoderPage, pages.length);
	const slots = pages[page - 1];
	const controller = {
		...base,
		visibleEncoderCount: count,
		encoderPage: page,
		encoderPageCount: pages.length,
		encoderGroups: base.encoderGroups.map(group => ({
			...group,
			pages: assignmentPages(group.label as ParameterFamily, count).map((_, index) => ({ number: index + 1, slots: [] })),
		})),
		encoderSlots: Array.from({ length: count }, (_, index) => slots[index]?.attribute ?? null),
		encoderPushTurnSlots: Array.from({ length: count }, () => null),
		attributeLabels: new Map(Object.values(assignments).flat().map(slot => [slot.attribute, slot.label])),
		encoderSemanticDisplay: (attribute: string) => slots.find(slot => slot.attribute === attribute)?.value,
	} as ParameterController;
	return (
		<CommandSectionFixture inheritAppState hardware initialMode="playbacks"
			programmer={<ParameterControlView controller={controller} />}
			hardwareTools={mode => mode === "playbacks"
				? <CompactEncoderSummary controller={controller} slots={slots} />
				: <CompactTimingAndSpeed />}
		/>
	);
}

function Prototype({ encoderCount = 6, initialFamily = "Color", width = 1496 }: {
	encoderCount?: EncoderCount;
	initialFamily?: ParameterFamily;
	width?: number;
}) {
	return (
		<ApplicationStateHarness actions={[{ type: "SET_MIDI_PROFILE", value: true }]}>
			<StoryShowObjectsProvider>
				<div className="app-shell compact-encoder-prototype"
					style={{ width, maxWidth: "calc(100vw - 48px)", height: "min(761px, calc(100vh - 48px))" }}>
					<div className="compact-encoder-workspace">
						<span>Hardware connected · {encoderCount} encoders · Storybook example values</span>
						<h1>Playbacks with encoder assignments</h1>
						<p>One shared fade time, playback page, large speed buttons and encoder values stay visible together.</p>
						<p>Tap a group to select it. Tap the active group again to cycle its pages.</p>
					</div>
					<ProgrammerSurface hardware initialFamily={initialFamily}
						render={controller => <PagedDesk base={controller} count={encoderCount} />} />
				</div>
			</StoryShowObjectsProvider>
		</ApplicationStateHarness>
	);
}

const meta = {
	title: "ToskLight/Compact hardware encoders",
	parameters: { layout: "fullscreen" },
	globals: { mode: "hardware" },
	args: { encoderCount: 6 as EncoderCount },
	argTypes: { encoderCount: { control: "inline-radio", options: [4, 5, 6] } },
	render: (args: { encoderCount: EncoderCount }) => <Prototype {...args} />,
} satisfies Meta<{ encoderCount: EncoderCount }>;
export default meta;
type Story = StoryObj<typeof meta>;

export const EncoderAssignments: Story = {};
export const FourEncoders: Story = { args: { encoderCount: 4 } };
export const FiveEncoders: Story = { args: { encoderCount: 5 } };
export const PositionWithUnassignedSlots: Story = {
	render: args => <Prototype {...args} initialFamily="Position" />,
};
export const NarrowDesk: Story = { render: args => <Prototype {...args} width={1024} /> };
