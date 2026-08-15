// @vitest-environment jsdom
import { fireEvent, render, screen, within } from "@testing-library/react";
import { act } from "react";
import { createRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	TIMECODE_LANE_HEADER_WIDTH,
	TimecodeTimelineEditor,
	type TimecodeTimelineEditorHandle,
	timelineFrameX,
	timelineZoomGeometry,
} from "./TimecodeTimelineEditor";

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
		},
	],
	lanes: [],
};

describe("TimecodeTimelineEditor", () => {
	it("fits the whole timeline at 1x and reaches 17.5 CSS pixels per frame", () => {
		const onCommit = vi.fn();
		const onScrub = vi.fn();
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
								number: 1,
								name: "First",
							},
						],
					},
				]}
				audioPlayers={[]}
				waveformPeaks={[0.2, 1, 0.4]}
				onScrub={onScrub}
				onCommit={onCommit}
				onPreview={vi.fn()}
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
		const editor = screen.getByLabelText("Timecode timeline editor");
		const zoom = screen.getByLabelText("Timeline zoom");
		expect(
			viewport.compareDocumentPosition(zoom) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(canvas?.firstElementChild).toHaveClass("timecode-ruler");
		const { maximumZoom } = timelineZoomGeometry(440, 560);
		fireEvent.input(screen.getByLabelText("Timeline zoom"), {
			target: { value: maximumZoom },
		});
		expect(Number(canvas?.dataset.pixelsPerFrame)).toBeCloseTo(17.5);
		expect(Number.parseFloat(canvas?.style.width ?? "0")).toBeCloseTo(7860);
		expect(editor.lastElementChild).toContainElement(zoom);
		expect(
			screen.getByLabelText("Linked audio waveform").querySelectorAll("line"),
		).toHaveLength(3);
		expect(screen.queryByRole("button", { name: "Copy" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
		expect(screen.queryByLabelText("Speed Group")).toBeNull();
		expect(screen.queryByLabelText("Cuelist")).toBeNull();
		const marker = screen.getByTitle("Verse · 00:00:02.00");
		expect(marker).toHaveClass("timecode-timeline-marker");
		expect(marker).toHaveStyle({
			left: `${timelineFrameX(88, 17.5)}px`,
			width: "44px",
			transform: "translateX(-22px)",
		});
		expect(
			marker.querySelector(".timecode-timeline-marker-line"),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:01.00");
		const playhead = screen.getByRole("button", {
			name: "Drag playhead to seek",
		});
		expect(playhead).toHaveStyle({
			left: `${timelineFrameX(44, 17.5)}px`,
		});
		playhead.setPointerCapture = vi.fn();
		playhead.hasPointerCapture = vi.fn(() => true);
		fireEvent.pointerDown(playhead, { pointerId: 1, clientX: 88 });
		fireEvent.pointerMove(playhead, { pointerId: 1, clientX: 176 });
		expect(onScrub).toHaveBeenCalled();
		ref.current?.addAudioLane();
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				lanes: [
					expect.objectContaining({
						content: expect.objectContaining({ kind: "audio_volume" }),
					}),
				],
			}),
		);
		act(() => ref.current?.chooseCueListLane());
		expect(
			screen.getByRole("dialog", { name: "Choose Cue List" }),
		).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Add lane" }));
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				lanes: [
					expect.objectContaining({
						content: expect.objectContaining({
							kind: "cue_list",
							cue_list_id: "00000000-0000-0000-0000-000000000010",
						}),
					}),
				],
			}),
		);
		expect(screen.queryByLabelText("Marker CSV")).toBeNull();
		expect(screen.queryByRole("button", { name: "Add Marker" })).toBeNull();
	});

	it("creates and edits a clip on a patched Audio Player lane", () => {
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
		fireEvent.input(screen.getByLabelText("Audio Folder"), {
			target: { value: "12" },
		});
		fireEvent.input(screen.getByLabelText("Audio File"), {
			target: { value: "34" },
		});
		fireEvent.click(screen.getByLabelText("Repeat"));
		fireEvent.input(screen.getByLabelText("Volume %"), {
			target: { value: "65" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Add volume point" }));
		const content = commits.at(-1)?.lanes[0].content;
		expect(content).toMatchObject({
			kind: "audio_player",
			clips: [
				expect.objectContaining({
					folder: 12,
					file: 34,
					repeat: true,
					volume_keyframes: [
						expect.objectContaining({ value: 0.65 }),
						expect.objectContaining({ value: 0.65 }),
					],
				}),
			],
		});
	});

	it("moves a Cue List clip and resizes each boundary through its drag handles", () => {
		function Harness() {
			const [draft, setDraft] = useState<TimecodeDefinition>({
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
			});
			return (
				<TimecodeTimelineEditor
					definition={draft}
					frame={0}
					fps={44}
					cueLists={[
						{
							id: "00000000-0000-0000-0000-000000000010",
							name: "Opening",
							cues: [{ id: "cue-1", number: 1, name: "First" }],
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
		expect(editor.getByLabelText("Start frame")).toHaveValue("88");
		const start = clip.querySelector<HTMLElement>(".timecode-clip-edge.start");
		expect(start).toBeTruthy();
		fireEvent.pointerDown(start as HTMLElement, { pointerId: 2, clientX: 144 });
		fireEvent.pointerMove(window, { clientX: 172 });
		fireEvent.pointerUp(window);
		expect(editor.getByLabelText("Start frame")).toHaveValue("110");
		const end = clip.querySelector<HTMLElement>(".timecode-clip-edge.end");
		expect(end).toBeTruthy();
		fireEvent.pointerDown(end as HTMLElement, { pointerId: 3, clientX: 324 });
		fireEvent.pointerMove(window, { clientX: 352 });
		fireEvent.pointerUp(window);
		expect(editor.getByLabelText("End frame")).toHaveValue("198");
	});

	it("adds and drags audio-volume keyframes through the lane", () => {
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
		fireEvent.pointerDown(lane as HTMLElement, { clientX: 80, clientY: 100 });
		const keyframe = view.getByTitle(/Main audio volume · 100%/);
		fireEvent.pointerDown(keyframe, {
			pointerId: 1,
			clientX: 80,
			clientY: 100,
		});
		fireEvent.pointerMove(window, { clientX: 112, clientY: 132 });
		fireEvent.pointerUp(window);
		expect(within(view.container).getByLabelText("Volume %")).toHaveValue("80");
	});
});
