import { describe, expect, it } from "vitest";
import type { PlaybackProjection } from "./contracts";
import { groupIdentity } from "./contracts";
import { selectGroupProjections } from "./groupProjectionSelection";
import { PlaybackRuntimeStore } from "./store";
import {
	DESK_ID,
	groupProjection,
	playbackSnapshot,
	SHOW_ID,
} from "./testFixtures";

function missingGroup(groupId: string): PlaybackProjection {
	return {
		scope: { show_id: SHOW_ID, show_revision: 4 },
		requested: { kind: "group", group_id: groupId },
		playback_number: null,
		target: "missing",
	};
}

describe("selectGroupProjections", () => {
	it("resolves a Group the server reports absent instead of loading forever", () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID);
		const identities = [groupIdentity("front"), groupIdentity("previous-show")];
		store.installSnapshot(
			playbackSnapshot(identities, 10, [
				groupProjection("front", 0.5, 2),
				missingGroup("previous-show"),
			]),
			identities,
		);

		const selection = selectGroupProjections(store.getSnapshot(), [
			"front",
			"previous-show",
		]);

		expect(selection.ready).toBe(true);
		expect(selection.projections.get("front")).toMatchObject({
			target: "group",
			master: 0.5,
		});
		expect(selection.projections.get("previous-show")).toBeUndefined();
	});

	it("keeps a stored empty Group and an unanswered Group distinct from an absent one", () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID);
		const identities = [groupIdentity("empty")];
		store.installSnapshot(
			playbackSnapshot(identities, 10, [groupProjection("empty", 1)]),
			identities,
		);

		const stored = selectGroupProjections(store.getSnapshot(), ["empty"]);
		expect(stored.ready).toBe(true);
		expect(stored.projections.get("empty")).toMatchObject({ target: "group" });

		const unanswered = selectGroupProjections(store.getSnapshot(), [
			"empty",
			"not-yet-loaded",
		]);
		expect(unanswered.ready).toBe(false);
	});
});
