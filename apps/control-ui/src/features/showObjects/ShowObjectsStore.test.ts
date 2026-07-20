import { describe, expect, it } from "vitest";
import type { StoredGroup, StoredPreset } from "../../api/types";
import type { ShowObject } from "./contracts";
import { ShowObjectsStore } from "./store";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function group(
	revision: number,
	name: string,
	fixtures = ["fixture-1"],
): ShowObject<"group"> {
	return {
		kind: "group",
		id: "1",
		revision,
		updated_at: "2026-07-19T00:00:00Z",
		body: { name, fixtures },
	};
}

function preset(revision: number, name: string): ShowObject<"preset"> {
	return {
		kind: "preset",
		id: "2.1",
		revision,
		updated_at: "2026-07-19T00:00:00Z",
		body: { name, number: 1, family: "Color", values: {} },
	};
}

describe("ShowObjectsStore", () => {
	it("keeps a pending server-captured Preset authoritative until response settlement", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "preset", [preset(3, "Blue")], 10);
		const generation = store.getSnapshot().authorityGeneration;
		const before = store.getSnapshot();
		const token = store.beginPending(SHOW_ID, "preset", "2.1");
		const pending = store.getSnapshot();

		expect(pending.presets).toBe(before.presets);
		expect(pending.groups).toBe(before.groups);
		expect(pending.presets).toEqual([preset(3, "Blue")]);
		expect(pending.pendingObjectKeys).toEqual(new Set(["preset:2.1"]));

		store.settlePending(
			token,
			{
				objectId: "2.1",
				revision: 4,
				object: preset(4, "Deep Blue"),
			},
			12,
			11,
			generation,
		);
		expect(store.getSnapshot().presets[0]).toMatchObject({
			revision: 4,
			body: { name: "Deep Blue" },
		});
		expect(store.getSnapshot().groups).toBe(before.groups);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("abandons pending-only actions without reprojecting either collection", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", [group(1, "Front")]);
		store.setCollection(SHOW_ID, "preset", [preset(3, "Blue")]);
		const before = store.getSnapshot();
		const token = store.beginPending(SHOW_ID, "preset", "2.1");

		store.abandon(token);

		const after = store.getSnapshot();
		expect(after.groups).toBe(before.groups);
		expect(after.presets).toBe(before.presets);
		expect(after.pendingObjectKeys.size).toBe(0);
	});

	it("keeps an event authoritative when it precedes a pending-only response", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "preset", [preset(3, "Blue")], 10);
		const generation = store.getSnapshot().authorityGeneration;
		const token = store.beginPending(SHOW_ID, "preset", "2.1");
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 12,
			eventSequence: 11,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 4,
					body: { ...preset(4, "Event body").body, icon: "◆" },
					deleted: false,
				},
			],
		});

		store.settlePending(
			token,
			{
				objectId: "2.1",
				revision: 4,
				object: preset(4, "Delayed response"),
			},
			12,
			11,
			generation,
		);
		expect(store.getSnapshot().presets[0]).toMatchObject({
			revision: 4,
			body: { name: "Event body", icon: "◆" },
		});
	});

	it("ignores a late pending response after same-show authority replacement", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "preset", [preset(1, "Old")]);
		const generation = store.getSnapshot().authorityGeneration;
		const token = store.beginPending(SHOW_ID, "preset", "2.1");

		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "preset", [preset(7, "Replacement")]);

		expect(
			store.settlePending(
				token,
				{
					objectId: "2.1",
					revision: 2,
					object: preset(2, "Late"),
				},
				2,
				1,
				generation,
			),
		).toBe(false);
		expect(store.getSnapshot().presets).toEqual([preset(7, "Replacement")]);
	});

	it("settles a legacy Preset ID returned for a canonical pending target", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "preset", [], 10);
		const generation = store.getSnapshot().authorityGeneration;
		const token = store.beginPending(SHOW_ID, "preset", "2.1");
		const legacy = { ...preset(1, "Legacy"), id: "preset-legacy" };

		store.settlePending(
			token,
			{ objectId: legacy.id, revision: legacy.revision, object: legacy },
			2,
			11,
			generation,
		);

		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		expect(store.getSnapshot().presets).toEqual([legacy]);
	});

	it("settles a revisioned Group deletion before its canonical event", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", [group(2, "Front")], 10);
		const generation = store.getSnapshot().authorityGeneration;
		const token = store.beginPending(SHOW_ID, "group", "1");

		store.settlePending(
			token,
			{ objectId: "1", revision: 3, object: null },
			12,
			11,
			generation,
		);

		expect(store.getSnapshot().groups).toEqual([]);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 12,
			eventSequence: 11,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 3,
					body: null,
					deleted: true,
				},
			],
		});
		expect(store.getSnapshot().groups).toEqual([]);
	});

	it("does not reproject a Group deletion response after its event", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", [group(2, "Front")], 10);
		const generation = store.getSnapshot().authorityGeneration;
		const token = store.beginPending(SHOW_ID, "group", "1");
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 12,
			eventSequence: 11,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 3,
					body: null,
					deleted: true,
				},
			],
		});
		const afterEvent = store.getSnapshot().groups;

		store.settlePending(
			token,
			{ objectId: "1", revision: 3, object: null },
			12,
			11,
			generation,
		);

		expect(store.getSnapshot().groups).toBe(afterEvent);
	});

	it("keeps a mismatched Group settlement pending until it is abandoned", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "group", [group(2, "Front")]);
		const generation = store.getSnapshot().authorityGeneration;
		const token = store.beginPending(SHOW_ID, "group", "1");

		expect(() =>
			store.settlePending(
				token,
				{ objectId: "other", revision: 3, object: null },
				12,
				null,
				generation,
			),
		).toThrow("Group settlement ID does not match pending mutation");
		expect(store.getSnapshot().pendingObjectKeys).toEqual(new Set(["group:1"]));

		store.abandon(token);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("guards an exact repair against a newer event on the same object", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		store.setCollection(SHOW_ID, "preset", [preset(3, "Blue")], 10);
		const stamp = store.captureObjectAuthority(SHOW_ID, "preset", "2.1");
		expect(stamp).not.toBeNull();
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 12,
			eventSequence: 11,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 4,
					body: preset(4, "Event body").body,
					deleted: false,
				},
			],
		});

		expect(store.installObjectIfAuthorityUnchanged(stamp!, null)).toBe(false);
		expect(store.getSnapshot().presets).toEqual([preset(4, "Event body")]);
	});

	it("shows a targeted optimistic Group body and rolls it back on failure", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(3, "Front")]);

		const optimistic: StoredGroup = {
			...store.getSnapshot().groups[0].body,
			name: "Front Wash",
		};
		const token = store.beginOptimistic(SHOW_ID, "group", "1", optimistic);

		expect(store.getSnapshot().groups[0].body.name).toBe("Front Wash");
		expect(store.getSnapshot().groups[0].revision).toBe(3);
		expect(store.getSnapshot().pendingObjectKeys).toEqual(new Set(["group:1"]));

		store.rollback(token, new Error("revision conflict"));

		expect(store.getSnapshot().groups).toEqual([group(3, "Front")]);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		expect(store.getSnapshot().error?.message).toBe("revision conflict");
	});

	it("reconciles an optimistic Preset with the exact authoritative event", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "preset", [preset(4, "Blue")]);
		const body: StoredPreset = {
			name: "Deep Blue",
			number: 1,
			family: "Color",
			values: { "fixture-1": { color: "#0011ff" } },
		};
		const token = store.beginOptimistic(SHOW_ID, "preset", "2.1", body);

		store.applyChange({
			showId: SHOW_ID,
			showRevision: 19,
			eventSequence: 42,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 5,
					body: { ...body, icon: "◆" },
					deleted: false,
				},
			],
		});
		store.commit(token, 5);

		expect(store.getSnapshot()).toMatchObject({
			showRevision: 19,
			eventSequence: 42,
			pendingObjectKeys: new Set(),
		});
		expect(store.getSnapshot().presets[0]).toMatchObject({
			revision: 5,
			body: { name: "Deep Blue", icon: "◆" },
		});
	});

	it("holds a mutation response floor until its canonical event arrives", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(2, "Read before event")], 10);
		const body = { ...group(2, "Local").body, name: "Local response" };
		const token = store.beginOptimistic(SHOW_ID, "group", "1", body);

		store.commit(token, 3, 12);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 20,
			eventSequence: 11,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 2,
					body: group(2, "Delayed older event").body,
					deleted: false,
				},
			],
		});
		expect(store.getSnapshot().groups[0]).toMatchObject({
			revision: 3,
			body: { name: "Local response" },
		});

		store.applyChange({
			showId: SHOW_ID,
			showRevision: 21,
			eventSequence: 12,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 3,
					body: group(3, "Canonical response").body,
					deleted: false,
				},
			],
		});
		expect(store.getSnapshot().groups[0]).toMatchObject({
			revision: 3,
			body: { name: "Canonical response" },
		});
	});

	it("does not let a snapshot older than a committed response replace it", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(1, "Snapshot")], 10);
		const token = store.beginOptimistic(
			SHOW_ID,
			"group",
			"1",
			group(1, "Committed response").body,
		);
		store.commit(token, 2, 11);

		store.setCollection(SHOW_ID, "group", [group(1, "Stale load")], 10);
		expect(store.getSnapshot().groups[0]).toMatchObject({
			revision: 2,
			body: { name: "Committed response" },
		});
	});

	it("does not let a stale snapshot undo an applied update or deletion", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(
			SHOW_ID,
			"group",
			[group(1, "Original"), { ...group(1, "Delete me"), id: "2" }],
			10,
		);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 12,
			eventSequence: 12,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 2,
					body: group(2, "Updated event").body,
					deleted: false,
				},
				{
					kind: "group",
					objectId: "2",
					objectRevision: 2,
					body: null,
					deleted: true,
				},
			],
		});

		store.setCollection(
			SHOW_ID,
			"group",
			[group(1, "Stale update"), { ...group(1, "Stale deletion"), id: "2" }],
			11,
		);
		expect(store.getSnapshot().groups).toEqual([group(2, "Updated event")]);
	});

	it("keeps the latest optimistic write visible across consecutive responses", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(1, "Original")], 5);
		const first = store.beginOptimistic(
			SHOW_ID,
			"group",
			"1",
			group(1, "First write").body,
		);
		const second = store.beginOptimistic(
			SHOW_ID,
			"group",
			"1",
			group(1, "Second write").body,
		);

		store.commit(first, 2, 6);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 6,
			eventSequence: 6,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 2,
					body: group(2, "First canonical write").body,
					deleted: false,
				},
			],
		});
		store.commit(second, 3, 7);

		expect(store.getSnapshot().groups[0]).toMatchObject({
			revision: 3,
			body: { name: "Second write" },
		});
	});

	it("keeps an event that arrives before its matching response authoritative", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "preset", [preset(1, "Blue")], 5);
		const token = store.beginOptimistic(SHOW_ID, "preset", "2.1", {
			...preset(1, "Local").body,
			name: "Local",
		});
		expect(store.getSnapshot().presets[0]?.body.name).toBe("Local");
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 8,
			eventSequence: 6,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 2,
					body: { ...preset(2, "Canonical").body, icon: "◆" },
					deleted: false,
				},
			],
		});

		store.commit(token, 2, 6);
		expect(store.getSnapshot().presets[0]).toMatchObject({
			revision: 2,
			body: { name: "Canonical", icon: "◆" },
		});
	});

	it("does not let a delayed lower revision overwrite a targeted read", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(1, "Original")]);
		store.installObject(SHOW_ID, "group", group(3, "Targeted read"));
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 4,
			eventSequence: 1,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 2,
					body: group(2, "Delayed event").body,
					deleted: false,
				},
			],
		});
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 5,
			eventSequence: 2,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 2,
					body: null,
					deleted: true,
				},
			],
		});
		expect(store.getSnapshot().groups[0]).toMatchObject({
			revision: 3,
			body: { name: "Targeted read" },
		});
	});

	it("orders a targeted read against both sides of its event floor", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(2, "Original")], 10);
		store.installObject(SHOW_ID, "group", group(3, "Targeted read"), 12);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 11,
			eventSequence: 11,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 3,
					body: group(3, "Older equal-revision event").body,
					deleted: false,
				},
			],
		});
		expect(store.getSnapshot().groups[0]?.body.name).toBe("Targeted read");

		store.applyChange({
			showId: SHOW_ID,
			showRevision: 12,
			eventSequence: 12,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 3,
					body: group(3, "Canonical event").body,
					deleted: false,
				},
			],
		});
		expect(store.getSnapshot().groups[0]?.body.name).toBe("Canonical event");
	});

	it("keeps an event authoritative when it arrives before a targeted read", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(1, "Original")], 5);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 6,
			eventSequence: 6,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 2,
					body: group(2, "Canonical event").body,
					deleted: false,
				},
			],
		});

		store.installObject(SHOW_ID, "group", group(2, "Delayed read"), 6);
		expect(store.getSnapshot().groups[0]?.body.name).toBe("Canonical event");
	});

	it("rejects an optimistic mutation from a stale show lifecycle", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		expect(() =>
			store.beginOptimistic(
				"22222222-2222-4222-8222-222222222222",
				"group",
				"1",
				group(1, "Stale").body,
			),
		).toThrow("is no longer active");
		expect(store.getSnapshot().showId).toBe(SHOW_ID);
	});

	it("updates live derived membership while leaving frozen membership fixed", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [
			group(1, "Source", ["a", "b", "c", "d"]),
			{
				...group(1, "Odd", []),
				id: "2",
				body: {
					...group(1, "Odd", []).body,
					derived_from: { source_group_id: "1", rule: { type: "odd" } },
				},
			},
			{
				...group(1, "Frozen", ["a", "c"]),
				id: "3",
				body: {
					...group(1, "Frozen", ["a", "c"]).body,
					frozen_from: {
						source_group_id: "1",
						source_revision: 1,
						captured_at: "2026-07-19T00:00:00Z",
					},
				},
			},
		]);
		expect(store.getSnapshot().groups[1].body.fixtures).toEqual(["a", "c"]);

		store.updateCollection("group", (groups) =>
			groups.map((candidate) =>
				candidate.id === "1"
					? {
							...candidate,
							body: { ...candidate.body, fixtures: ["b", "c", "d"] },
						}
					: candidate,
			),
		);
		expect(
			store.getSnapshot().groups.find((item) => item.id === "2")?.body.fixtures,
		).toEqual(["b", "d"]);
		expect(
			store.getSnapshot().groups.find((item) => item.id === "3")?.body.fixtures,
		).toEqual(["a", "c"]);
	});

	it("orders by event sequence per object and accepts recreation at revision one", () => {
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID);
		store.setCollection(SHOW_ID, "group", [group(9, "Old")]);
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 20,
			eventSequence: 50,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 10,
					body: null,
					deleted: true,
				},
			],
		});
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 21,
			eventSequence: 51,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 1,
					body: { name: "Recreated", fixtures: [] },
					deleted: false,
				},
			],
		});
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 18,
			eventSequence: 49,
			changes: [
				{
					kind: "group",
					objectId: "1",
					objectRevision: 9,
					body: { name: "Stale", fixtures: [] },
					deleted: false,
				},
			],
		});

		expect(store.getSnapshot().groups).toEqual([
			{
				kind: "group",
				id: "1",
				revision: 1,
				updated_at: "",
				body: { name: "Recreated", fixtures: [] },
			},
		]);
		expect(store.getSnapshot().showRevision).toBe(21);
	});
});
