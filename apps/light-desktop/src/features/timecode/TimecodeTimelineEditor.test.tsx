// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	TimecodeTimelineEditor,
	type TimecodeTimelineEditorHandle,
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
		expect(Number(canvas?.dataset.pixelsPerFrame)).toBeCloseTo(720 / 440);
		const { maximumZoom } = timelineZoomGeometry(440, 720);
		fireEvent.input(screen.getByLabelText("Timeline zoom"), {
			target: { value: maximumZoom },
		});
		expect(Number(canvas?.dataset.pixelsPerFrame)).toBeCloseTo(17.5);
		expect(Number.parseFloat(canvas?.style.width ?? "0")).toBeCloseTo(7700);
		expect(
			screen.getByLabelText("Linked audio waveform").querySelectorAll("line"),
		).toHaveLength(3);
		expect(screen.queryByRole("button", { name: "Copy" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
		expect(screen.queryByLabelText("Speed Group")).toBeNull();
		expect(screen.queryByLabelText("Cuelist")).toBeNull();
		expect(screen.getByTitle("Verse · 00:00:02.00")).toHaveClass(
			"timecode-timeline-marker",
		);
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:01.00");
		const playhead = screen.getByRole("button", { name: "Drag playhead to seek" });
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
		ref.current?.addCueListLane();
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
});
