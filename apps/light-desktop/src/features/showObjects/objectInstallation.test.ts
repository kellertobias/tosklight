import { describe, expect, it } from "vitest";
import type { ShowObject } from "./contracts";
import { ShowObjectsStore } from "./store";
const SHOW = "11111111-1111-4111-8111-111111111111";
const page: ShowObject<"playback_page"> = {
	kind: "playback_page",
	id: "2",
	revision: 1,
	updated_at: "",
	body: { number: 2, name: "Page 2", assignments: {} },
};
function install(store: ShowObjectsStore, sequence: number) {
	store.installObjects(
		SHOW,
		[{ kind: "playback_page", objectId: "2", object: page }],
		sequence,
		20,
		"seal",
	);
}
function deleted(store: ShowObjectsStore, sequence: number) {
	store.applyChange({
		showId: SHOW,
		showRevision: 21,
		eventSequence: sequence,
		changes: [
			{
				kind: "playback_page",
				objectId: "2",
				objectRevision: 2,
				body: null,
				deleted: true,
			},
		],
	});
}
describe("new object response boundary guards", () => {
	it("allows equal collection-boundary creation but does not reproject a repeated response", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW);
		store.setCollection(SHOW, "playback_page", [], 10);
		install(store, 10);
		expect(store.getSnapshot().playbackPages).toEqual([page]);
		const before = store.getSnapshot().playbackPages;
		install(store, 10);
		expect(store.getSnapshot().playbackPages).toBe(before);
	});
	it.each([
		11, 12,
	])("preserves an exact deletion tombstone at sequence %s against delayed create", (sequence) => {
		const store = new ShowObjectsStore();
		store.reset(SHOW);
		store.setCollection(SHOW, "playback_page", [], 10);
		deleted(store, sequence);
		install(store, 11);
		expect(store.getSnapshot().playbackPages).toEqual([]);
	});
	it("does not resurrect an object absent from a strictly later collection snapshot", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW);
		store.setCollection(SHOW, "playback_page", [], 12);
		install(store, 11);
		expect(store.getSnapshot().playbackPages).toEqual([]);
	});
	it("keeps a newer deletion authoritative after the create response and replay", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW);
		store.setCollection(SHOW, "playback_page", [], 10);
		install(store, 10);
		deleted(store, 11);
		install(store, 10);
		expect(store.getSnapshot().playbackPages).toEqual([]);
	});
});
