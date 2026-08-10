// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/generated/light-wire";
import { TimecodeTimelineEditor } from "./TimecodeTimelineEditor";

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
	markers: [],
	lanes: [],
};

describe("TimecodeTimelineEditor", () => {
	it("offers touch-visible editing, waveform, zoom and explicit CSV mode", () => {
		const onCommit = vi.fn();
		render(
			<TimecodeTimelineEditor
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
				onScrub={vi.fn()}
				onCommit={onCommit}
				onPreview={vi.fn()}
				onBeginGesture={vi.fn()}
				onEndGesture={vi.fn()}
			/>,
		);
		expect(screen.getByLabelText("Timeline zoom")).toBeTruthy();
		expect(
			screen.getByLabelText("Linked audio waveform").querySelectorAll("line"),
		).toHaveLength(3);
		expect(
			screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled"),
		).toBe(true);
		fireEvent.click(screen.getByRole("button", { name: "Add audio lane" }));
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				lanes: [
					expect.objectContaining({
						content: expect.objectContaining({ kind: "audio_volume" }),
					}),
				],
			}),
		);
		fireEvent.click(screen.getByRole("button", { name: "Add Cuelist lane" }));
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

		fireEvent.click(screen.getByRole("button", { name: "Import marker CSV" }));
		fireEvent.change(screen.getByLabelText("Marker CSV"), {
			target: { value: "position,name\n00:00:02:00,Verse" },
		});
		expect(screen.getByDisplayValue("Append")).toBeTruthy();
		fireEvent.click(screen.getByRole("button", { name: "Apply marker CSV" }));
		expect(onCommit).toHaveBeenCalledWith(
			expect.objectContaining({
				markers: [expect.objectContaining({ frame: 88, name: "Verse" })],
			}),
		);
	});
});
