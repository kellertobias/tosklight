// @vitest-environment jsdom
import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { act } from "react";
import { createRef, useState } from "react";
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
		<div aria-label="Timecode encoder probe">
			<button type="button" onClick={() => deck.timeline[0]?.set(2)}>
				Set timeline zoom
			</button>
			<button type="button" onClick={() => deck.timeline[1]?.set(132)}>
				Set playhead
			</button>
			<button type="button" onClick={() => deck.keyframe[1]?.set(42)}>
				Set keyframe value
			</button>
			<button type="button" onClick={() => deck.keyframe[3]?.set(2)}>
				Set keyframe easing
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
		fireEvent.click(screen.getByRole("button", { name: "Set timeline zoom" }));
		const canvas = screen
			.getByLabelText("Timecode timeline viewport")
			.querySelector<HTMLElement>(".timecode-timeline-canvas");
		expect(Number(canvas?.dataset.pixelsPerFrame)).toBeCloseTo((560 / 440) * 2);

		fireEvent.click(screen.getByRole("button", { name: "Set playhead" }));
		expect(onScrub).toHaveBeenCalledWith(132);
		fireEvent.click(screen.getByRole("button", { name: "Set keyframe value" }));
		expect(screen.getByTitle(/Audio · 42%/)).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Set keyframe easing" }));
		expect(screen.getByTitle(/Audio · 42% · ease_out/)).toBeTruthy();
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
		const picker = within(
			screen.getByRole("dialog", { name: "Choose Speed Group" }),
		).getByRole("button", { name: "Speed Group B" });
		fireEvent.click(picker);
		expect(screen.queryByRole("option", { name: "Speed Group A" })).toBeNull();
		expect(screen.queryByRole("option", { name: "Speed Group C" })).toBeNull();
		for (const group of ["B", "D", "E"])
			expect(
				screen.getByRole("option", { name: `Speed Group ${group}` }),
			).toBeInTheDocument();
		fireEvent.click(screen.getByRole("option", { name: "Speed Group D" }));
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
		expect(
			screen.getByLabelText("Selected lane and keyframe actions"),
		).toBeInTheDocument();
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
			width: "44px",
			transform: "translateX(-22px)",
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
		expect(
			screen.getByRole("dialog", { name: "Choose Cue List" }),
		).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Add lane" }));
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				lanes: expect.arrayContaining([
					expect.objectContaining({
						content: expect.objectContaining({
							kind: "cue_list",
							cue_list_id: "00000000-0000-0000-0000-000000000010",
						}),
					}),
				]),
			}),
		);
		expect(screen.queryByLabelText("Marker CSV")).toBeNull();
		expect(screen.queryByRole("button", { name: "Add Marker" })).toBeNull();
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
		fireEvent.click(screen.getByRole("button", { name: "+ clip" }));
		expect(screen.getByTitle(/Audio Player 201 · 000\.000/)).toBeTruthy();
		expect(screen.queryByLabelText("Audio Folder")).toBeNull();
		expect(
			screen.getByRole("button", { name: "Delete selected keyframe" }),
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
				name: "Insert keyframe at 00:00:00.00",
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
		fireEvent.click(
			screen.getByRole("button", { name: "Delete selected keyframe" }),
		);
		expect(view.queryByTitle(/Main audio volume · 80%/)).toBeNull();
	});
});
