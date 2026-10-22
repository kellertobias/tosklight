import { expect, it, vi } from "vitest";
import type { CueList } from "../../api/types";
import type {
	TimecodeDefinition,
	TimecodeCueListClip,
} from "../../api/types/timecode";
import {
	cueClipEndBounds,
	extendCueClipEnd,
	createCueClipEndWriter,
} from "./extendCueClipEnd";
const clip: TimecodeCueListClip = {
	id: "clip",
	start_frame: 44,
	end_frame: 396,
	start_cue_id: "one",
	end_cue_id: "three",
	start_behavior: "state",
	end_behavior: "hold",
	cue_starts: [{ cue_id: "two", offset_frame: 132 }],
	in_fade_frames: 300,
	out_fade_frames: 300,
};
const cue = (id: string, fade: number, trigger: object) => ({
	id,
	number: id,
	name: id,
	fade_millis: fade,
	delay_millis: 0,
	trigger,
	changes: [],
});
const body = {
	id: "list",
	name: "Opening",
	mode: "sequence",
	cues: [
		cue("one", 1000, { type: "manual" }),
		cue("two", 2000, { type: "manual" }),
		cue("three", 1000, { type: "follow", delay_millis: 0 }),
	],
} as unknown as CueList;
const context = {
	cueList: body,
	timingDefaults: { sequenceFadeMillis: 1000, releaseFadeMillis: 500 },
};
const definition: TimecodeDefinition = {
	id: "timecode",
	number: 1,
	name: "Opening",
	duration_frame: 440,
	transport_offset_frame: 0,
	auto_start: false,
	markers: [],
	lanes: [
		{
			id: "lane",
			name: "Opening",
			content: {
				kind: "cue_list",
				cue_list_id: "list",
				clips: [
					clip,
					{
						...clip,
						id: "next",
						start_frame: 420,
						end_frame: 440,
						start_cue_id: "one",
						end_cue_id: "one",
						cue_starts: [],
					},
				],
			},
		},
	],
};
function current(value: TimecodeDefinition) {
	const content = value.lanes[0].content;
	if (content.kind !== "cue_list") throw Error("wrong lane");
	return content.clips[0];
}
it("bounds end after the actual last Follow trigger and before the next clip", () => {
	expect(cueClipEndBounds(definition, "lane", "clip", context)).toEqual({
		minimum: 265,
		maximum: 420,
	});
	expect(
		current(extendCueClipEnd(definition, "lane", "clip", 999, context))
			.end_frame,
	).toBe(420);
	expect(
		current(extendCueClipEnd(definition, "lane", "clip", 0, context)).end_frame,
	).toBe(265);
});
it("preserves earlier trigger positions and Cuelist timing when extending", () => {
	const before = structuredClone(body);
	const next = current(
		extendCueClipEnd(definition, "lane", "clip", 410, context),
	);
	expect(next).toEqual({ ...clip, end_frame: 410 });
	expect(next.cue_starts).toBe(clip.cue_starts);
	expect(body).toEqual(before);
	expect(current(definition)).toBe(clip);
});
it("clamps gain fades when shortened without scaling Cue offsets", () => {
	const next = current(
		extendCueClipEnd(definition, "lane", "clip", 270, context),
	);
	expect(next.in_fade_frames).toBe(226);
	expect(next.out_fade_frames).toBe(226);
	expect(next.cue_starts).toEqual(clip.cue_starts);
});
it("uses duration when there is no next clip and refuses unknown multi-Cue timing", () => {
	const solo = {
		...definition,
		lanes: [
			{
				...definition.lanes[0],
				content: {
					kind: "cue_list" as const,
					cue_list_id: "list",
					clips: [clip],
				},
			},
		],
	};
	expect(cueClipEndBounds(solo, "lane", "clip", context)?.maximum).toBe(440);
	expect(cueClipEndBounds(solo, "lane", "clip")).toBeNull();
	expect(extendCueClipEnd(solo, "lane", "clip", 500)).toBe(solo);
	expect(extendCueClipEnd(solo, "lane", "clip", NaN, context)).toBe(solo);
});
it("does not rewrite no-op edits or missing clips", () => {
	expect(extendCueClipEnd(definition, "lane", "clip", 396, context)).toBe(
		definition,
	);
	expect(extendCueClipEnd(definition, "lane", "missing", 410, context)).toBe(
		definition,
	);
});

it("the bound writer resolves authoritative Cuelist context and skips no-ops", () => {
	const commit = vi.fn();
	const write = createCueClipEndWriter({
		definition,
		cueLists: [{ id: body.id, name: body.name, cues: body.cues, body }],
		timingDefaults: context.timingDefaults,
		onCommit: commit,
	});
	write("lane", "clip", 396);
	expect(commit).not.toHaveBeenCalled();
	write("lane", "clip", 410);
	expect(commit).toHaveBeenCalledOnce();
	expect(current(commit.mock.calls[0][0]).end_frame).toBe(410);
});
