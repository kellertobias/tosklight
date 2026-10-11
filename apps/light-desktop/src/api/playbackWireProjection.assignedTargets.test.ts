import { expect, it } from "vitest";
import { decodePlaybackProjection } from "./playbackWireProjection";
import { playbackTargetMatches } from "../features/playbackRuntime/topologyReconciliation";

it.each([
	["macro", "macro_id"],
	["timecode", "timecode_id"],
] as const)(
	"accepts authoritative %s runtime IDs and distinguishes same-kind retargets",
	(target, key) => {
		const value = {
			scope: { show_id: "show", show_revision: 1 },
			identity: { kind: "playback", page: 2, position: 103 },
			target,
			[key]: "original-target",
		};
		const projection = decodePlaybackProjection(value, "$");
		expect(projection).toEqual(value);
		const original = target === "macro"
			? { type: target, macro_id: "original-target" }
			: { type: target, timecode_id: "original-target" };
		const other = target === "macro"
			? { type: target, macro_id: "other-target" }
			: { type: target, timecode_id: "other-target" };
		expect(playbackTargetMatches(original, projection)).toBe(true);
		expect(playbackTargetMatches(other, projection)).toBe(false);
		expect(() =>
			decodePlaybackProjection({ ...value, [key]: undefined }, "$"),
		).toThrow(new RegExp(key));
	},
);
