import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/generated/light-wire";
import {
	copyTimelineItem,
	deleteTimelineItem,
	moveTimelineItem,
	parseMarkerCsv,
	snapTimelineFrame,
	timelineItems,
} from "./editorModel";

const definition: TimecodeDefinition = {
	id: "00000000-0000-0000-0000-000000000001",
	number: 1,
	name: "Song",
	duration_frame: 440,
	transport_offset_frame: 0,
	auto_start: false,
	markers: [
		{ id: "00000000-0000-0000-0000-000000000010", frame: 220, name: "Hit" },
	],
	lanes: [
		{
			id: "00000000-0000-0000-0000-000000000020",
			name: "Main audio",
			content: {
				kind: "audio_volume",
				keyframes: [
					{
						id: "00000000-0000-0000-0000-000000000021",
						frame: 44,
						value: 1,
						fade_frames: 0,
						curve: "linear",
					},
				],
			},
		},
	],
};

describe("Timecode editor model", () => {
	it("projects labels and moves frame-accurate keyframes with marker snapping", () => {
		const selection = {
			kind: "volume" as const,
			laneId: definition.lanes[0].id,
			itemId: "00000000-0000-0000-0000-000000000021",
		};
		expect(
			timelineItems(definition).find((item) => item.kind === "volume")?.label,
		).toContain("100%");
		expect(snapTimelineFrame(216, definition, 2)).toBe(220);
		const moved = moveTimelineItem(definition, selection, 220);
		expect(moved.lanes[0].content).toMatchObject({
			kind: "audio_volume",
			keyframes: [{ frame: 220 }],
		});
	});

	it("copies and deletes the exact selected item", () => {
		const selection = {
			kind: "volume" as const,
			laneId: definition.lanes[0].id,
			itemId: "00000000-0000-0000-0000-000000000021",
		};
		const copied = copyTimelineItem(
			definition,
			selection,
			"00000000-0000-0000-0000-000000000022",
			44,
		);
		expect(
			timelineItems(copied.definition).filter((item) => item.kind === "volume"),
		).toHaveLength(2);
		expect(
			timelineItems(copied.definition).find((item) =>
				item.selection.itemId.endsWith("22"),
			)?.frame,
		).toBe(88);
		expect(
			timelineItems(
				deleteTimelineItem(copied.definition, copied.selection),
			).filter((item) => item.kind === "volume"),
		).toHaveLength(1);
	});

	it("imports strict frame or timecode CSV with append-ready identities", () => {
		vi.spyOn(crypto, "randomUUID")
			.mockReturnValueOnce("00000000-0000-0000-0000-000000000031")
			.mockReturnValueOnce("00000000-0000-0000-0000-000000000032");
		expect(
			parseMarkerCsv(
				'position,name,color\n44,"Verse, one",#ff00aa\n00:00:05:00,Hit,',
				44,
				440,
			),
		).toEqual([
			{
				id: "00000000-0000-0000-0000-000000000031",
				frame: 44,
				name: "Verse, one",
				color: "#ff00aa",
			},
			{
				id: "00000000-0000-0000-0000-000000000032",
				frame: 220,
				name: "Hit",
			},
		]);
		expect(() => parseMarkerCsv("name,color\nBad,#fff", 44, 440)).toThrow(
			"position",
		);
	});
});
