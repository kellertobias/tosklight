import { describe, expect, it } from "vitest";
import { decodePlaybackIdentity } from "../../api/playbackWireProjection";
import { identityKey, projectionKeys } from "./contracts";
import { PlaybackRuntimeStore } from "./store";
import {
	cueProjection,
	playbackSnapshot,
	SHOW_ID,
	DESK_ID,
	CUE_LIST_ID,
} from "./testFixtures";

describe("exact direct Cuelist projection isolation", () => {
	it("validates the additive selector and keeps aggregate feedback through absent direct state", () => {
		const identity = decodePlaybackIdentity(
			{ kind: "direct_cue_list", cue_list_id: CUE_LIST_ID },
			"identity",
		);
		expect(identityKey(identity)).toBe(`direct-cuelist:${CUE_LIST_ID}`);
		const base = cueProjection();
		if (base.target !== "cue_list") throw new Error("Expected cue fixture");
		const aggregate = {
			...base,
			requested: { kind: "cue_list" as const, cue_list_id: CUE_LIST_ID },
			playback_number: null,
		};
		const direct = { ...aggregate, requested: identity, runtime: null };
		expect(projectionKeys(direct)).toEqual([`direct-cuelist:${CUE_LIST_ID}`]);
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID);
		store.installSnapshot(
			playbackSnapshot([aggregate.requested], 10, [aggregate]),
			[aggregate.requested],
		);
		store.applyProjection(direct, 11);
		expect(
			store.getSnapshot().projections.get(`cuelist:${CUE_LIST_ID}`)?.[0],
		).toBe(aggregate);
		const installed = store
			.getSnapshot()
			.projections.get(identityKey(identity))?.[0];
		if (installed?.target !== "cue_list")
			throw new Error("Expected exact direct cue state");
		expect(installed.runtime).toBeNull();
	});
});
