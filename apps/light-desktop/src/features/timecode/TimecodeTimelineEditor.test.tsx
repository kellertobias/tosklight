// @vitest-environment jsdom
import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { act, createRef, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	TIMECODE_LANE_HEADER_WIDTH,
	TimecodeTimelineEditor,
	type TimecodeTimelineEditorHandle,
	timelineFrameX,
} from "./TimecodeTimelineEditor";
import { useTimecodeEncoderDeck } from "./timecodeEncoderBridge";

function TimecodeEncoderProbe() {
	const deck = useTimecodeEncoderDeck();
	if (!deck) return null;
	return (
		<div
			role="group"
			aria-label="Timecode encoder probe"
			data-keyframe-labels={deck.keyframe.map((slot) => slot.label).join("|")}
			data-keyframe-displays={deck.keyframe
				.map((slot) => slot.display)
				.join("|")}
			data-timeline-labels={deck.timeline.map((slot) => slot.label).join("|")}
			data-selection-label={deck.selectionLabel}
		>
			<button type="button" onClick={() => deck.keyframe[3]?.set(2)}>
				Set timeline zone
			</button>
			<button type="button" onClick={() => deck.keyframe[2]?.set(132)}>
				Set playhead
			</button>
			<button type="button" onClick={() => deck.keyframe[1]?.set(42)}>
				Set keyframe value
			</button>
			<button type="button" onClick={() => deck.keyframe[0]?.set(132)}>
				Set selected position
			</button>
			<button
				type="button"
				onClick={() => {
					const slot = deck.keyframe[1];
					if (slot) slot.set(slot.value + 1);
				}}
			>
				Set selected color
			</button>
		</div>
	);
}

const definition: TimecodeDefinition = {
	id: "00000000-0000-0000-0000-000000000001",
	number: 1,
	name: "Song",
	duration_frame: 440,
	transport_offset_frame: 0,
	auto_start: false,
	audio: {
		asset_id: "00000000-0000-0000-0000-000000000002",
		asset_revision: 1,
	},
	markers: [
		{
			id: "00000000-0000-0000-0000-000000000003",
			frame: 88,
			name: "Verse",
			color: "#33aa77",
		},
	],
	lanes: [
		{
			id: "00000000-0000-0000-0000-000000000004",
			name: "Audio",
			content: {
				kind: "audio_volume",
				keyframes: [
					{
						id: "00000000-0000-0000-0000-000000000005",
						frame: 0,
						value: 1,
						fade_frames: 0,
						curve: "linear",
					},
				],
			},
		},
	],
};

afterEach(cleanup);

describe("TimecodeTimelineEditor", () => {
	it("publishes timeline navigation and selected-keyframe encoder groups", () => {
		const onScrub = vi.fn();
		function Harness() {
			const [draft, setDraft] = useState(definition);
			return (
				<>
					<TimecodeTimelineEditor
						definition={draft}
						frame={44}
						fps={44}
						cueLists={[]}
						audioPlayers={[]}
						onScrub={onScrub}
						onCommit={setDraft}
						onPreview={setDraft}
						onBeginGesture={vi.fn()}
						onEndGesture={vi.fn()}
					/>
					<TimecodeEncoderProbe />
				</>
			);
		}
		render(<Harness />);
		fireEvent.click(
			screen.getByRole("button", { name: /Audio.*audio volume/i }),
		);
		expect(screen.getByLabelText("Timecode encoder probe")).toHaveAttribute(
			"data-keyframe-labels",
			"Keyframe position|Keyframe value|Playhead|Timeline zone",
		);
		expect(screen.getByLabelText("Timecode encoder probe")).toHaveAttribute(
			"data-timeline-labels",
			"Lane|Keyframe selection|Playhead|Timeline zone",
		);
		fireEvent.click(screen.getByRole("button", { name: "Set timeline zone" }));
		const canvas = screen
			.getByLabelText("Timecode timeline viewport")
			.querySelector<HTMLElement>(".timecode-timeline-canvas");
		expect(Number(canvas?.dataset.pixelsPerFrame)).toBeCloseTo((560 / 440) * 2);
		const overview = screen.getByRole("scrollbar", {
			name: "Timeline overview",
		});
		expect(
			overview.querySelector(".timecode-timeline-overview-lanes"),
		).toHaveAttribute("data-lane-height", "3");
		overview.setPointerCapture = vi.fn();
		vi.spyOn(overview, "getBoundingClientRect").mockReturnValue({
			x: 0,
			y: 0,
			top: 0,
			left: 0,
			right: 720,
			bottom: 20,
			width: 720,
			height: 20,
			toJSON: () => ({}),
		});
		fireEvent.pointerDown(overview, { pointerId: 9, clientX: 540 });
		expect(screen.getByLabelText("Timecode timeline viewport").scrollLeft).toBe(
			560,
		);

		fireEvent.click(screen.getByRole("button", { name: "Set playhead" }));
		expect(onScrub).toHaveBeenCalledWith(132);
		fireEvent.click(screen.getByRole("button", { name: "Set keyframe value" }));
		expect(screen.getByTitle(/Audio · 42%/)).toBeTruthy();
	});

	it("reduces overview lanes to a one-pixel floor as lane density grows", () => {
		const denseDefinition: TimecodeDefinition = {
			...definition,
			lanes: Array.from({ length: 24 }, (_, index) => ({
				id: `00000000-0000-0000-0001-${String(index).padStart(12, "0")}`,
				name: `Lane ${index + 1}`,
				content: {
					kind: "audio_volume" as const,
					keyframes: [],
				},
			})),
		};
		render(
			<TimecodeTimelineEditor
				definition={denseDefinition}
				frame={0}
				fps={44}
				cueLists={[]}
				audioPlayers={[]}
				onScrub={vi.fn()}
				onCommit={vi.fn()}
				onPreview={vi.fn()}
				onBeginGesture={vi.fn()}
				onEndGesture={vi.fn()}
			/>,
		);
		const overview = screen.getByRole("scrollbar", {
			name: "Timeline overview",
		});
		expect(
			overview.querySelector(".timecode-timeline-overview-lanes"),
		).toHaveAttribute("data-lane-height", "1");
	});

	it("shows marker actions and marker-specific encoders for a selected marker", () => {
		function Harness() {
			const [draft, setDraft] = useState<TimecodeDefinition>({
				...definition,
				markers: [
					...definition.markers,
					{
						id: "00000000-0000-0000-0000-000000000006",
						frame: 176,
						name: "Bridge",
						color: "#58d4ef",
					},
				],
			});
			return (
				<>
					<TimecodeTimelineEditor
						definition={draft}
						frame={44}
						fps={44}
						cueLists={[]}
						audioPlayers={[]}
						markersLocked
						onScrub={vi.fn()}
						onCommit={setDraft}
						onPreview={setDraft}
						onBeginGesture={vi.fn()}
						onEndGesture={vi.fn()}
					/>
					<TimecodeEncoderProbe />
				</>
			);
		}
		render(<Harness />);
		fireEvent.pointerDown(screen.getByTitle("Verse · 00:00:02.00"));

		const actions = screen.getByRole("group", {
			name: "Selected marker actions",
		});
		expect(actions).toHaveTextContent("Verse");
		expect(
			screen.getByRole("button", { name: "Previous Marker" }),
		).toBeEnabled();
		expect(screen.getByRole("button", { name: "Next Marker" })).toBeEnabled();
		expect(screen.getByLabelText("Timecode encoder probe")).toHaveAttribute(
			"data-selection-label",
			"Selected Marker",
		);
		expect(screen.getByLabelText("Timecode encoder probe")).toHaveAttribute(
			"data-keyframe-labels",
			"Marker position|Marker color|Playhead|Timeline zone",
		);
		expect(screen.getByLabelText("Timecode encoder probe")).toHaveAttribute(
			"data-keyframe-displays",
			"00:00:02.00|Green|00:00:01.00|100%",
		);
		const colorButton = screen.getByRole("button", {
			name: "Marker color: Green. Select next color",
		});
		fireEvent.click(colorButton);
		expect(
			screen.getByRole("button", {
				name: "Marker color: Yellow. Select next color",
			}),
		).toBeInTheDocument();
		for (let index = 0; index < 3; index += 1)
			fireEvent.click(
				screen.getByRole("button", { name: /Marker color: .*next color/ }),
			);
		const whiteButton = screen.getByRole("button", {
			name: "Marker color: White. Select next color",
		});
		expect(
			whiteButton.style.getPropertyValue("--timecode-marker-text-color"),
		).toBe("#11121a");

		fireEvent.click(screen.getByRole("button", { name: "Set Name" }));
		const nameModal = screen.getByRole("dialog", { name: "Marker name" });
		expect(
			within(nameModal).getByLabelText("Full text keyboard"),
		).toBeInTheDocument();
		for (let index = 0; index < 5; index += 1)
			fireEvent.keyDown(window, { key: "Backspace" });
		for (const key of "Chorus") fireEvent.keyDown(window, { key });
		fireEvent.keyDown(window, { key: "Enter" });
		expect(actions).toHaveTextContent("Chorus");
		expect(screen.queryByRole("dialog", { name: "Marker name" })).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Place Marker" }));
		fireEvent.change(screen.getByLabelText("Marker timecode"), {
			target: { value: "00:00:03.00" },
		});
		fireEvent.blur(screen.getByLabelText("Marker timecode"));
		expect(screen.getByTitle("Chorus · 00:00:03.00")).toBeTruthy();

		fireEvent.click(screen.getByRole("button", { name: "Next Marker" }));
		expect(actions).toHaveTextContent("Bridge");
		fireEvent.click(
			screen.getByRole("button", { name: "Set selected position" }),
		);
		expect(screen.getByTitle("Bridge · 00:00:03.00")).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Set selected color" }));
		expect(screen.getByTitle("Bridge · 00:00:03.00")).toHaveStyle({
			color: "#33aa77",
		});
	});

	it("draws a full-width stepped Speed Group value line through its keyframes", () => {
		render(
			<TimecodeTimelineEditor
				definition={{
					...definition,
					audio: null,
					lanes: [
						{
							id: "speed-a",
							name: "Speed Group A",
							content: {
								kind: "speed_group",
								group: "A",
								keyframes: [
									{ id: "speed-1", frame: 44, bpm: 120, phase: 0 },
									{ id: "speed-2", frame: 220, bpm: 90, phase: 0 },
									{ id: "speed-3", frame: 352, bpm: 150, phase: 0 },
								],
							},
						},
					],
				}}
				frame={0}
				fps={44}
				cueLists={[]}
				audioPlayers={[]}
				onScrub={vi.fn()}
				onCommit={vi.fn()}
				onPreview={vi.fn()}
				onBeginGesture={vi.fn()}
				onEndGesture={vi.fn()}
			/>,
		);
		const line = screen.getByRole("img", {
			name: "Speed Group A value line",
		});
		expect(line.querySelector("polyline")).toHaveAttribute(
			"points",
			"0,50 10,50 50,50 50,88 80,88 80,12 100,12",
		);
		expect(screen.getByTitle(/Speed Group A · 90 BPM/)).toHaveStyle({
			top: "88%",
		});
	});

	it("offers only unused Speed Groups when adding a lane", () => {
		const ref = createRef<TimecodeTimelineEditorHandle>();
		const onCommit = vi.fn();
		render(
			<TimecodeTimelineEditor
				ref={ref}
				definition={{
					...definition,
					lanes: [
						{
							id: "speed-a",
							name: "Speed Group A",
							content: { kind: "speed_group", group: "A", keyframes: [] },
						},
						{
							id: "speed-c",
							name: "Speed Group C",
							content: { kind: "speed_group", group: "C", keyframes: [] },
						},
					],
				}}
				frame={0}
				fps={44}
				cueLists={[]}
				audioPlayers={[]}
				onScrub={vi.fn()}
				onCommit={onCommit}
				onPreview={vi.fn()}
				onBeginGesture={vi.fn()}
				onEndGesture={vi.fn()}
			/>,
		);
		act(() => ref.current?.chooseSpeedLane());
		const dialog = screen.getByRole("dialog", { name: "Choose Speed Group" });
		expect(
			within(dialog).queryByRole("button", { name: /Speed Group A/ }),
		).toBeNull();
		expect(
			within(dialog).queryByRole("button", { name: /Speed Group C/ }),
		).toBeNull();
		for (const group of ["B", "D", "E"])
			expect(
				within(dialog).getByRole("button", {
					name: new RegExp(`Speed Group ${group}`),
				}),
			).toBeInTheDocument();
		const speedGroupD = within(dialog).getByRole("button", {
			name: /Speed Group D/,
		});
		fireEvent.click(speedGroupD);
		expect(speedGroupD).toHaveAttribute("aria-pressed", "true");
		fireEvent.click(screen.getByRole("button", { name: "Add lane" }));
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				lanes: expect.arrayContaining([
					expect.objectContaining({
						content: expect.objectContaining({
							kind: "speed_group",
							group: "D",
						}),
					}),
				]),
			}),
		);
	});

	it("adds the default Cue List from a scrollable pool and selects its new lane", () => {
		const ref = createRef<TimecodeTimelineEditorHandle>();
		function Harness() {
			const [draft, setDraft] = useState<TimecodeDefinition>({
				...definition,
				lanes: [],
			});
			return (
				<TimecodeTimelineEditor
					ref={ref}
					definition={draft}
					frame={0}
					fps={44}
					cueLists={[
						{
							id: "cue-list-opening",
							name: "Opening",
							cues: [{ id: "cue-opening-1", number: "1", name: "First" }],
						},
					]}
					audioPlayers={[]}
					onScrub={vi.fn()}
					onCommit={setDraft}
					onPreview={setDraft}
					onBeginGesture={vi.fn()}
					onEndGesture={vi.fn()}
				/>
			);
		}
		render(<Harness />);
		act(() => ref.current?.chooseCueListLane());
		const dialog = screen.getByRole("dialog", { name: "Choose Cue List" });
		expect(
			dialog.querySelector(".timecode-cuelist-chooser-scroll"),
		).toBeTruthy();
		expect(
			within(dialog).getByRole("button", { name: "Cue List 1: Opening" }),
		).toHaveAttribute("aria-pressed", "true");
		fireEvent.click(within(dialog).getByRole("button", { name: "Add lane" }));
		const lane = document.querySelector(".timecode-editor-lane.lane-cue_list");
		expect(lane).toHaveClass("selected");
		expect(
			within(lane as HTMLElement).getByRole("button", {
				name: /Opening.*cue list/i,
			}),
		).toBeInTheDocument();
	});

	it("fits the whole timeline at 1x with continuous headers and ruler stripes", () => {
		const onCommit = vi.fn();
		const onScrub = vi.fn();
		const onPreview = vi.fn();
		const ref = createRef<TimecodeTimelineEditorHandle>();
		render(
			<TimecodeTimelineEditor
				ref={ref}
				definition={definition}
				frame={44}
				fps={44}
				cueLists={[
					{
						id: "00000000-0000-0000-0000-000000000010",
						name: "Opening",
						cues: [
							{
								id: "00000000-0000-0000-0000-000000000011",
								number: "1",
								name: "First",
							},
						],
					},
					{
						id: "00000000-0000-0000-0000-000000000020",
						name: "Finale",
						cues: [
							{
								id: "00000000-0000-0000-0000-000000000021",
								number: "1",
								name: "Last",
							},
						],
					},
				]}
				audioPlayers={[]}
				waveformPeaks={[0.2, 1, 0.4]}
				markersLocked
				onScrub={onScrub}
				onCommit={onCommit}
				onPreview={onPreview}
				onBeginGesture={vi.fn()}
				onEndGesture={vi.fn()}
			/>,
		);
		const viewport = screen.getByLabelText("Timecode timeline viewport");
		const canvas = viewport.querySelector<HTMLElement>(
			".timecode-timeline-canvas",
		);
		expect(canvas?.style.width).toBe("720px");
		expect(canvas?.dataset.timeOriginPx).toBe(
			String(TIMECODE_LANE_HEADER_WIDTH),
		);
		expect(Number(canvas?.dataset.pixelsPerFrame)).toBeCloseTo(560 / 440);
		expect(canvas?.firstElementChild).toHaveClass("timecode-ruler-stripes");
		expect(viewport.querySelector(".timecode-lane-header-column")).toBeTruthy();
		expect(
			viewport.querySelectorAll(".timecode-ruler-stripes i").length,
		).toBeGreaterThan(1);
		expect(screen.queryByLabelText("Timeline zoom")).toBeNull();
		fireEvent.click(
			screen.getByRole("button", { name: /Audio.*audio volume/i }),
		);
		expect(
			screen.getByLabelText("Selected lane and keyframe actions"),
		).toBeInTheDocument();
		expect(
			screen.getByLabelText("Selected lane and keyframe actions"),
		).toHaveTextContent("Audio");
		expect(screen.getByRole("button", { name: "Prev Keyframe" })).toBeTruthy();
		expect(screen.getByRole("button", { name: "Next Keyframe" })).toBeTruthy();
		expect(
			screen.getByRole("button", { name: "Delete Keyframe" }),
		).toBeEnabled();
		expect(screen.queryByText(/inspect, copy, move, or delete/)).toBeNull();
		expect(
			screen.getByLabelText("Linked audio waveform").querySelectorAll("line"),
		).toHaveLength(3);
		expect(screen.queryByRole("button", { name: "Copy" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
		expect(screen.queryByLabelText("Speed Group")).toBeNull();
		expect(screen.queryByLabelText("Cuelist")).toBeNull();
		const marker = screen.getByTitle("Verse · 00:00:02.00");
		expect(marker).toHaveClass("timecode-timeline-marker");
		expect(marker).toHaveAttribute("aria-disabled", "true");
		expect(marker).toHaveStyle({ color: "#33aa77" });
		expect(marker).toHaveStyle({
			left: `${timelineFrameX(88, Number(canvas?.dataset.pixelsPerFrame))}px`,
		});
		expect(
			marker.querySelector(".timecode-timeline-marker-line"),
		).toBeInTheDocument();
		fireEvent.pointerDown(marker, { pointerId: 7, clientX: 272 });
		fireEvent.pointerMove(window, { clientX: 400 });
		fireEvent.pointerUp(window);
		expect(onPreview).not.toHaveBeenCalled();
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:01.00");
		const playhead = screen.getByRole("button", {
			name: "Drag playhead to seek",
		});
		expect(playhead).toHaveStyle({
			left: `${timelineFrameX(44, Number(canvas?.dataset.pixelsPerFrame))}px`,
		});
		playhead.setPointerCapture = vi.fn();
		playhead.hasPointerCapture = vi.fn(() => true);
		fireEvent.pointerDown(playhead, { pointerId: 1, clientX: 88 });
		fireEvent.pointerMove(playhead, { pointerId: 1, clientX: 176 });
		expect(onScrub).toHaveBeenCalled();
		act(() => ref.current?.chooseCueListLane());
		const cueListDialog = screen.getByRole("dialog", {
			name: "Choose Cue List",
		});
		expect(
			cueListDialog.querySelector(".timecode-cuelist-chooser-grid"),
		).toBeTruthy();
		expect(
			within(cueListDialog).getByRole("button", {
				name: "Cue List 1: Opening",
			}),
		).toHaveAttribute("aria-pressed", "true");
		fireEvent.click(
			within(cueListDialog).getByRole("button", { name: "Cue List 2: Finale" }),
		);
		expect(
			within(cueListDialog).getByRole("button", { name: "Cue List 2: Finale" }),
		).toHaveAttribute("aria-pressed", "true");
		fireEvent.click(screen.getByRole("button", { name: "Add lane" }));
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				lanes: expect.arrayContaining([
					expect.objectContaining({
						content: expect.objectContaining({
							kind: "cue_list",
							cue_list_id: "00000000-0000-0000-0000-000000000020",
						}),
					}),
				]),
			}),
		);
		expect(screen.queryByLabelText("Marker CSV")).toBeNull();
		expect(screen.queryByRole("button", { name: "Add Marker" })).toBeNull();
	});

	it("keeps the marker hit area inside the timeline and drags a marker to frame zero", () => {
		function Harness() {
			const [draft, setDraft] = useState(definition);
			return (
				<TimecodeTimelineEditor
					definition={draft}
					frame={0}
					fps={44}
					cueLists={[]}
					audioPlayers={[]}
					onScrub={vi.fn()}
					onCommit={setDraft}
					onPreview={setDraft}
					onBeginGesture={vi.fn()}
					onEndGesture={vi.fn()}
				/>
			);
		}
		render(<Harness />);
		const canvas = screen
			.getByLabelText("Timecode timeline viewport")
			.querySelector<HTMLElement>(".timecode-timeline-canvas");
		const pixelsPerFrame = Number(canvas?.dataset.pixelsPerFrame);
		const marker = screen.getByTitle("Verse · 00:00:02.00");
		fireEvent.pointerDown(marker, { pointerId: 3, clientX: 300 });
		expect(marker).toHaveClass("selected");
		expect(marker.style.getPropertyValue("--timecode-marker-color")).toBe(
			"#33aa77",
		);
		fireEvent.pointerMove(window, {
			pointerId: 3,
			clientX: 300 - 88 * pixelsPerFrame,
		});
		fireEvent.pointerUp(window, { pointerId: 3 });
		const moved = screen.getByTitle("Verse · 00:00:00.00");
		expect(moved).toHaveStyle({ left: `${TIMECODE_LANE_HEADER_WIDTH}px` });
		expect(moved).not.toHaveStyle({ transform: "translateX(-22px)" });
	});

	it("creates a clip on a patched Audio Player lane without the old inspector", () => {
		const commits: TimecodeDefinition[] = [];
		function Harness() {
			const [draft, setDraft] = useState<TimecodeDefinition>({
				...definition,
				audio: null,
				lanes: [
					{
						id: "00000000-0000-0000-0000-000000000040",
						name: "Audio Player 201",
						content: {
							kind: "audio_player",
							fixture_id: "00000000-0000-0000-0000-000000000041",
							clips: [],
						},
					},
				],
			});
			return (
				<TimecodeTimelineEditor
					definition={draft}
					frame={44}
					fps={44}
					cueLists={[]}
					audioPlayers={[
						{
							fixtureId: "00000000-0000-0000-0000-000000000041",
							name: "Audio Player 201",
						},
					]}
					onScrub={vi.fn()}
					onCommit={(next) => {
						commits.push(next);
						setDraft(next);
					}}
					onPreview={setDraft}
					onBeginGesture={vi.fn()}
					onEndGesture={vi.fn()}
				/>
			);
		}
		render(<Harness />);
		expect(screen.queryByRole("button", { name: "+ clip" })).toBeNull();
		fireEvent.click(
			screen.getByRole("button", { name: /Audio Player 201.*audio player/i }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Add clip at 00:00:01.00" }),
		);
		expect(screen.getByTitle(/Audio Player 201 · 000\.000/)).toBeTruthy();
		expect(screen.queryByLabelText("Audio Folder")).toBeNull();
		expect(
			screen.getByRole("button", { name: "Delete Keyframe" }),
		).toBeDisabled();
		const content = commits.at(-1)?.lanes[0].content;
		expect(content).toMatchObject({
			kind: "audio_player",
			clips: [
				expect.objectContaining({
					folder: 0,
					file: 0,
					repeat: false,
				}),
			],
		});
	});

	it("moves a Cue List clip and resizes each boundary through its drag handles", () => {
		const latest = { current: definition };
		function Harness() {
			const [draft, setDraft] = useState<TimecodeDefinition>(() => ({
				...definition,
				lanes: [
					{
						id: "lane-1",
						name: "Opening",
						content: {
							kind: "cue_list",
							cue_list_id: "00000000-0000-0000-0000-000000000010",
							clips: [
								{
									id: "clip-1",
									start_frame: 44,
									end_frame: 132,
									start_cue_id: "cue-1",
									end_cue_id: "cue-1",
									start_behavior: "state",
									end_behavior: "release",
								},
							],
						},
					},
				],
			}));
			latest.current = draft;
			return (
				<TimecodeTimelineEditor
					definition={draft}
					frame={0}
					fps={44}
					cueLists={[
						{
							id: "00000000-0000-0000-0000-000000000010",
							name: "Opening",
							cues: [{ id: "cue-1", number: "1", name: "First" }],
						},
					]}
					audioPlayers={[]}
					onScrub={vi.fn()}
					onCommit={setDraft}
					onPreview={setDraft}
					onBeginGesture={vi.fn()}
					onEndGesture={vi.fn()}
				/>
			);
		}
		const view = render(<Harness />);
		const editor = within(view.container);
		const clip = editor.getByTitle(/Opening · state start/);
		fireEvent.pointerDown(clip, { pointerId: 1, clientX: 72 });
		fireEvent.pointerMove(window, { clientX: 128 });
		fireEvent.pointerUp(window);
		let cueLane = latest.current.lanes[0].content;
		expect(cueLane).toMatchObject({
			kind: "cue_list",
			clips: [expect.objectContaining({ start_frame: 88, end_frame: 176 })],
		});
		const start = clip.querySelector<HTMLElement>(".timecode-clip-edge.start");
		expect(start).toBeTruthy();
		fireEvent.pointerDown(start as HTMLElement, { pointerId: 2, clientX: 144 });
		fireEvent.pointerMove(window, { clientX: 172 });
		fireEvent.pointerUp(window);
		cueLane = latest.current.lanes[0].content;
		expect(cueLane).toMatchObject({
			kind: "cue_list",
			clips: [expect.objectContaining({ start_frame: 110 })],
		});
		const end = clip.querySelector<HTMLElement>(".timecode-clip-edge.end");
		expect(end).toBeTruthy();
		fireEvent.pointerDown(end as HTMLElement, { pointerId: 3, clientX: 324 });
		fireEvent.pointerMove(window, { clientX: 352 });
		fireEvent.pointerUp(window);
		cueLane = latest.current.lanes[0].content;
		expect(cueLane).toMatchObject({
			kind: "cue_list",
			clips: [expect.objectContaining({ end_frame: 198 })],
		});
	});

	it("selects a whole lane and edits keyframes through the compact action strip", () => {
		function Harness() {
			const [draft, setDraft] = useState<TimecodeDefinition>({
				...definition,
				lanes: [
					{
						id: "audio-volume",
						name: "Main audio volume",
						content: { kind: "audio_volume", keyframes: [] },
					},
				],
			});
			return (
				<TimecodeTimelineEditor
					definition={draft}
					frame={0}
					fps={44}
					cueLists={[]}
					audioPlayers={[]}
					onScrub={vi.fn()}
					onCommit={setDraft}
					onPreview={setDraft}
					onBeginGesture={vi.fn()}
					onEndGesture={vi.fn()}
				/>
			);
		}
		const view = render(<Harness />);
		const lane =
			view.container.querySelector<HTMLElement>(".lane-audio_volume");
		expect(lane).toBeTruthy();
		fireEvent.pointerDown(
			within(lane as HTMLElement).getByLabelText("Linked audio waveform"),
		);
		expect(lane).toHaveClass("selected");
		expect(screen.queryByRole("button", { name: "+ keyframe" })).toBeNull();
		fireEvent.click(
			screen.getByRole("button", {
				name: "Insert Keyframe",
			}),
		);
		const keyframe = view.getByTitle(/Main audio volume · 100%/);
		fireEvent.pointerDown(keyframe, {
			pointerId: 1,
			clientX: 80,
			clientY: 100,
		});
		fireEvent.pointerMove(window, { clientX: 112, clientY: 132 });
		fireEvent.pointerUp(window);
		expect(view.getByTitle(/Main audio volume · 80%/)).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Delete Keyframe" }));
		expect(view.queryByTitle(/Main audio volume · 80%/)).toBeNull();
	});
});
