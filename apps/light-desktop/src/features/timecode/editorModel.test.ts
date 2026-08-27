import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	copyTimelineItem,
	cueListClipScale,
	deleteTimelineItem,
	moveTimelineItem,
	parseMarkerCsv,
	reconcileAutomaticAudioLane,
	reorderTimelineLane,
	resizeTimelineClip,
	scaleCueListTimings,
	snapTimelineFrame,
	timelineItems,
	withClipFade,
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

	it("reorders complete lanes by their persisted identities", () => {
		const second = { ...definition.lanes[0], id: "second", name: "Second" };
		const third = { ...definition.lanes[0], id: "third", name: "Third" };
		const reordered = reorderTimelineLane(
			{ ...definition, lanes: [definition.lanes[0], second, third] },
			definition.lanes[0].id,
			"third",
		);
		expect(reordered.lanes.map((lane) => lane.name)).toEqual([
			"Second",
			"Third",
			"Main audio",
		]);
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
								cue_starts: [],
								in_fade_frames: 0,
								out_fade_frames: 0,
							},
							{
								id: "00000000-0000-0000-0000-000000000055",
								start_frame: 100,
								end_frame: 150,
								start_cue_id: "00000000-0000-0000-0000-000000000053",
								end_cue_id: "00000000-0000-0000-0000-000000000054",
								start_behavior: "state",
								end_behavior: "release",
								cue_starts: [],
								in_fade_frames: 0,
								out_fade_frames: 0,
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

describe("Scaling a Cuelist clip", () => {
	const cueList = {
		id: "list-1",
		name: "Opening",
		mode: "sequence" as const,
		priority: 1,
		looped: false,
		cues: [
			{
				id: "cue-1",
				number: "1",
				name: "First",
				delay_millis: 1_000,
				fade_millis: 2_000,
				out_delay_millis: 500,
				out_fade_millis: 4_000,
				values: [],
			},
			{
				id: "cue-2",
				number: "2",
				name: "Second",
				delay_millis: 3_000,
				fade_millis: 1_000,
				values: [],
			},
			{
				id: "cue-3",
				number: "3",
				name: "Outside the clip",
				delay_millis: 900,
				fade_millis: 900,
				values: [],
			},
		],
	} as unknown as Parameters<typeof scaleCueListTimings>[0];

	it("measures the factor an edge drag applies", () => {
		expect(
			cueListClipScale({ start_frame: 100, end_frame: 200 }, 100, 400),
		).toBe(3);
		expect(cueListClipScale({ start_frame: 0, end_frame: 100 }, 50, 100)).toBe(
			0.5,
		);
	});

	it("stretches every Cue timing the clip drives and leaves the rest alone", () => {
		const scaled = scaleCueListTimings(cueList, "cue-1", "cue-2", 2);

		expect(scaled.cues[0]).toMatchObject({
			delay_millis: 2_000,
			fade_millis: 4_000,
			out_delay_millis: 1_000,
			out_fade_millis: 8_000,
		});
		expect(scaled.cues[1]).toMatchObject({
			delay_millis: 6_000,
			fade_millis: 2_000,
		});
		// A Cue outside the clip's range keeps its own timing.
		expect(scaled.cues[2]).toMatchObject({
			delay_millis: 900,
			fade_millis: 900,
		});
	});

	it("leaves an inherited timing inherited rather than writing a scaled value", () => {
		const scaled = scaleCueListTimings(cueList, "cue-1", "cue-2", 2);

		expect(scaled.cues[1].out_delay_millis).toBeUndefined();
		expect(scaled.cues[1].out_fade_millis).toBeUndefined();
	});

	it("is a no-op for a scale that changes nothing or cannot be applied", () => {
		expect(scaleCueListTimings(cueList, "cue-1", "cue-2", 1)).toBe(cueList);
		expect(scaleCueListTimings(cueList, "cue-1", "cue-2", 0)).toBe(cueList);
		expect(scaleCueListTimings(cueList, "missing", "cue-2", 2)).toBe(cueList);
	});
});

describe("Resizing a Cuelist clip", () => {
	const clipDefinition = {
		id: "00000000-0000-0000-0000-0000000000a1",
		number: 2,
		name: "Cued",
		duration_frame: 880,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [
			{
				id: "lane-cues",
				name: "Opening",
				content: {
					kind: "cue_list" as const,
					cue_list_id: "list-1",
					clips: [
						{
							id: "clip-1",
							start_frame: 100,
							end_frame: 200,
							start_cue_id: "cue-1",
							end_cue_id: "cue-2",
							start_behavior: "state" as const,
							end_behavior: "release" as const,
							cue_starts: [{ cue_id: "cue-2", offset_frame: 50 }],
						},
					],
				},
			},
		],
	} as unknown as TimecodeDefinition;

	it("keeps a placed transition where it sits inside the clip", () => {
		const resized = resizeTimelineClip(
			clipDefinition,
			{ kind: "clip", laneId: "lane-cues", itemId: "clip-1" },
			"end",
			400,
		);
		const lane = resized.lanes[0].content;
		if (lane.kind !== "cue_list") throw new Error("expected the Cuelist lane");

		expect(lane.clips[0]).toMatchObject({ start_frame: 100, end_frame: 400 });
		// Half way through a 100-frame clip stays half way through a 300-frame clip.
		expect(lane.clips[0].cue_starts).toEqual([
			{ cue_id: "cue-2", offset_frame: 150 },
		]);
	});

	it("never leaves a transition outside the clip it belongs to", () => {
		const resized = resizeTimelineClip(
			clipDefinition,
			{ kind: "clip", laneId: "lane-cues", itemId: "clip-1" },
			"end",
			120,
		);
		const lane = resized.lanes[0].content;
		if (lane.kind !== "cue_list") throw new Error("expected the Cuelist lane");
		const length = lane.clips[0].end_frame - lane.clips[0].start_frame;

		for (const start of lane.clips[0].cue_starts) {
			expect(start.offset_frame).toBeGreaterThanOrEqual(0);
			expect(start.offset_frame).toBeLessThanOrEqual(length);
		}
	});
});

describe("withClipFade", () => {
	const clipLane = () => ({
		id: "lane-cues",
		name: "Opening",
		content: {
			kind: "cue_list" as const,
			cue_list_id: "list-1",
			clips: [
				{
					id: "clip-1",
					start_frame: 100,
					end_frame: 200,
					start_cue_id: "cue-1",
					end_cue_id: "cue-2",
					start_behavior: "state" as const,
					end_behavior: "release" as const,
					cue_starts: [],
					in_fade_frames: 0,
					out_fade_frames: 0,
				},
			],
		},
	});
	const definition = (): TimecodeDefinition => ({
		id: "timecode-1",
		number: 1,
		name: "Opener",
		duration_frame: 880,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [clipLane()],
	});
	const clipAfter = (
		kind: "in" | "out",
		frames: number,
	): { in_fade_frames: number; out_fade_frames: number } => {
		const lane = withClipFade(definition(), "lane-cues", "clip-1", kind, frames)
			.lanes[0];
		if (lane?.content.kind !== "cue_list") throw new Error("expected a Cuelist lane");
		return lane.content.clips[0] as never;
	};

	it("stores the fade the operator dragged", () => {
		expect(clipAfter("in", 25)).toMatchObject({
			in_fade_frames: 25,
			out_fade_frames: 0,
		});
		expect(clipAfter("out", 40)).toMatchObject({
			in_fade_frames: 0,
			out_fade_frames: 40,
		});
	});

	it("clamps a fade to the clip it belongs to", () => {
		// The clip is a hundred frames long, so neither fade can be longer than that.
		expect(clipAfter("in", 400).in_fade_frames).toBe(100);
		expect(clipAfter("out", -12).out_fade_frames).toBe(0);
	});
});
