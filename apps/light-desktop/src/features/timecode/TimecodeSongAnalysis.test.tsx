import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimecodeAudioAnalysisSnapshot } from "../../api/client/timecodes";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { snapTimelineFrame } from "./editorModel";
import { useTimecodeSongAnalysisPanel } from "./TimecodeSongAnalysis";

const definition: TimecodeDefinition = {
	id: "tc",
	number: 1,
	name: "Song",
	duration_frame: 44 * 60,
	transport_offset_frame: 0,
	auto_start: false,
	audio: { asset_id: "a", asset_revision: 1 },
	markers: [{ id: "m", frame: 1408, name: "Existing", color: null }],
	lanes: [],
};

const ready: TimecodeAudioAnalysisSnapshot = {
	timecode_id: "tc",
	asset_id: "a",
	asset_revision: 1,
	state: "ready",
	progress: 1,
	error: null,
	analysis: {
		digest: "ab",
		bpm: 120,
		beat_frames: [0, 22, 44, 66, 88],
		downbeat_frames: [0, 88],
		sections: [
			{ frame: 0, label: "Intro", energy: 0.1 },
			{ frame: 704, label: "Chorus", energy: 1 },
			{ frame: 1410, label: "Verse", energy: 0.4 },
		],
	},
};

function Panel({ onCommit, snapshot = ready }: { onCommit(value: TimecodeDefinition): void; snapshot?: TimecodeAudioAnalysisSnapshot }) {
	const { panel } = useTimecodeSongAnalysisPanel({
		analysis: { snapshot, error: null, available: true, start: vi.fn(), cancel: vi.fn() },
		definition,
		fps: 44,
		onCommit,
	});
	return <>{panel}</>;
}

describe("song analysis", () => {
	afterEach(cleanup);
	it("snaps a dragged frame to a nearby beat only when beat frames are given", () => {
		const loose = { ...definition, markers: [] };
		expect(snapTimelineFrame(30, loose, 1, 12)).toBe(30);
		expect(snapTimelineFrame(30, loose, 1, 12, { beatFrames: [22, 44] })).toBe(22);
	});

	it("offers sections no marker covers, and accepts a renamed one as a marker", () => {
		const onCommit = vi.fn();
		render(<Panel onCommit={onCommit} />);
		expect(screen.getByRole("status")).toHaveTextContent("120.0 BPM · 2 suggested sections");
		const items = screen.getAllByRole("listitem");
		expect(items).toHaveLength(2);
		fireEvent.change(screen.getByRole("textbox", { name: "Name for section at 0:16.0" }), { target: { value: "Big chorus" } });
		fireEvent.click(screen.getByRole("button", { name: "Accept Big chorus at 0:16.0" }));
		const committed = onCommit.mock.calls[0][0] as TimecodeDefinition;
		expect(committed.markers.map((marker) => [marker.frame, marker.name])).toEqual([
			[704, "Big chorus"],
			[1408, "Existing"],
		]);
	});

	it("discards a suggestion without storing anything, and shows progress while running", () => {
		const onCommit = vi.fn();
		const { rerender } = render(<Panel onCommit={onCommit} />);
		fireEvent.click(screen.getByRole("button", { name: "Discard Intro at 0:00.0" }));
		expect(screen.getAllByRole("listitem")).toHaveLength(1);
		expect(onCommit).not.toHaveBeenCalled();
		rerender(<Panel onCommit={onCommit} snapshot={{ ...ready, state: "running", progress: 0.4, analysis: null }} />);
		expect(screen.getByRole("status")).toHaveTextContent("Analysing audio… 40%");
		expect(screen.getByRole("button", { name: "Cancel" })).toBeVisible();
	});
});
