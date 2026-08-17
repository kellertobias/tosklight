import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	copyTimelineItem,
	deleteTimelineItem,
	moveTimelineItem,
	parseMarkerCsv,
	reconcileAutomaticAudioLane,
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
	it("keeps exactly one automatic audio lane only while audio is linked", () => {
		const withoutLane = {
			...definition,
			audio: {
				asset_id: "00000000-0000-0000-0000-000000000099",
				asset_revision: 1,
			},
			lanes: [],
		};
		const ids = ["audio-lane", "audio-keyframe"];
		const linked = reconcileAutomaticAudioLane(
			withoutLane,
			() => ids.shift() ?? "unexpected",
		);
		expect(linked.lanes).toEqual([
			expect.objectContaining({
				id: "audio-lane",
				content: expect.objectContaining({
					kind: "audio_volume",
					keyframes: [expect.objectContaining({ id: "audio-keyframe" })],
				}),
			}),
		]);
		expect(
			reconcileAutomaticAudioLane({
				...linked,
				lanes: [...linked.lanes, { ...linked.lanes[0], id: "duplicate-audio" }],
			}).lanes,
		).toHaveLength(1);
		expect(
			reconcileAutomaticAudioLane({ ...linked, audio: null }).lanes,
		).toHaveLength(0);
	});

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

	it("snaps clip ends together and keeps clips non-overlapping regardless of array order", () => {
		const cueDefinition: TimecodeDefinition = {
			...definition,
			lanes: [
				{
					id: "00000000-0000-0000-0000-000000000050",
					name: "Theater Sequence",
					content: {
						kind: "cue_list",
						cue_list_id: "00000000-0000-0000-0000-000000000051",
						clips: [
							{
								id: "00000000-0000-0000-0000-000000000052",
								start_frame: 200,
								end_frame: 250,
								start_cue_id: "00000000-0000-0000-0000-000000000053",
								end_cue_id: "00000000-0000-0000-0000-000000000054",
								start_behavior: "state",
								end_behavior: "release",
							},
							{
								id: "00000000-0000-0000-0000-000000000055",
								start_frame: 100,
								end_frame: 150,
								start_cue_id: "00000000-0000-0000-0000-000000000053",
								end_cue_id: "00000000-0000-0000-0000-000000000054",
								start_behavior: "state",
								end_behavior: "release",
							},
						],
					},
				},
			],
		};
		const selection = {
			kind: "clip" as const,
			laneId: cueDefinition.lanes[0].id,
			itemId: "00000000-0000-0000-0000-000000000055",
		};

		expect(
			snapTimelineFrame(152, cueDefinition, 2, 12, {
				selection,
				movingClip: true,
			}),
		).toBe(150);
		const moved = moveTimelineItem(cueDefinition, selection, 190);
		expect(
			(
				moved.lanes[0].content as { clips: Array<{ start_frame: number }> }
			).clips.map((clip) => clip.start_frame),
		).toEqual([150, 200]);
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

	it("moves, copies, and deletes Audio Player clips with their volume envelope", () => {
		const audioDefinition: TimecodeDefinition = {
			...definition,
			lanes: [
				{
					id: "00000000-0000-0000-0000-000000000040",
					name: "Audio Player 201",
					content: {
						kind: "audio_player",
						fixture_id: "00000000-0000-0000-0000-000000000041",
						clips: [
							{
								id: "00000000-0000-0000-0000-000000000042",
								start_frame: 44,
								end_frame: 132,
								folder: 1,
								file: 2,
								repeat: true,
								volume_keyframes: [
									{
										id: "00000000-0000-0000-0000-000000000043",
										frame: 88,
										value: 0.5,
										fade_frames: 22,
										curve: "linear",
									},
								],
							},
						],
					},
				},
			],
		};
		const selection = {
			kind: "clip" as const,
			laneId: audioDefinition.lanes[0].id,
			itemId: "00000000-0000-0000-0000-000000000042",
		};
		expect(timelineItems(audioDefinition)[0].label).toContain(
			"001.002 · repeat",
		);
		const moved = moveTimelineItem(audioDefinition, selection, 88);
		expect(moved.lanes[0].content).toMatchObject({
			kind: "audio_player",
			clips: [
				{ start_frame: 88, end_frame: 176, volume_keyframes: [{ frame: 132 }] },
			],
		});
		const copied = copyTimelineItem(
			moved,
			selection,
			"00000000-0000-0000-0000-000000000044",
			88,
		);
		expect(
			(copied.definition.lanes[0].content as { clips: unknown[] }).clips,
		).toHaveLength(2);
		expect(
			(
				deleteTimelineItem(copied.definition, copied.selection).lanes[0]
					.content as {
					clips: unknown[];
				}
			).clips,
		).toHaveLength(1);
	});
});
