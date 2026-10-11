import { describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import { projectPlaybackSlots } from "../../components/control/playbackFaderBank/projection";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	PlaybackTopologyObject,
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyTransport,
} from "./contracts";
import {
	PlaybackTopologyWriter,
	type PlaybackTopologyWriterOptions,
} from "./writer";
const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";
const ADDRESS = { page: 4, position: 680 };
function definition(name = "Original"): PlaybackDefinition {
	return {
		name,
		target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
		buttons: ["toggle", "none", "none"],
		button_count: 1,
		fader: "master",
		has_fader: false,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
	};
}
function page(
	revision = 1,
	name = "Original",
	id = "page-four",
): ShowObject<"playback_page"> {
	return {
		kind: "playback_page",
		id,
		revision,
		updated_at: "",
		body: { number: 4, name: "Page 4", assignments: { 680: definition(name) } },
	};
}
function cueList(
	revision = 1,
	id = "legacy-main-list",
	name = "Main",
): ShowObject<"cue_list"> {
	return {
		kind: "cue_list",
		id,
		revision,
		updated_at: "",
		body: {
			id: CUE_LIST_ID,
			name,
			priority: 0,
			mode: "sequence",
			looped: false,
			cues: [],
		},
	};
}
function group(): ShowObject<"group"> {
	return {
		kind: "group",
		id: "group-front",
		revision: 6,
		updated_at: "",
		body: { name: "Front", fixtures: ["fixture-1"] },
	};
}
function present<K extends ShowObjectKind>(
	object: ShowObject<K>,
): PlaybackTopologyObject<K> {
	return {
		state: "present",
		kind: object.kind as K,
		objectId: object.id,
		objectRevision: object.revision,
		body: object.body,
	};
}
function changed(
	request: PlaybackTopologyRequest,
	objects: PlaybackTopologyObject[] = [present(page(2, "Response"))],
	showRevision = 12,
	eventSequence = 41,
): PlaybackTopologyOutcome {
	const a = request.action;
	return {
		status: "changed",
		requestId: request.requestId,
		correlationId: CORRELATION_ID,
		showRevision,
		resolution:
			"cueListId" in a
				? { kind: "cue_list", cueListId: a.cueListId }
				: "address" in a
					? { kind: "playback", ...a.address }
					: { kind: "page", page: a.number },
		objects,
		eventSequence,
		replayed: false,
	};
}
function setup(
	apply: PlaybackTopologyTransport["apply"],
	loadObject?: PlaybackTopologyWriterOptions["loadObject"],
) {
	const loader =
		loadObject ??
		(vi.fn(async () => null) as PlaybackTopologyWriterOptions["loadObject"]);
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "session-a");
	store.setCollection(SHOW_ID, "cue_list", [cueList()], 10, 11);
	store.setCollection(SHOW_ID, "playback_page", [page()], 10, 11);
	store.setCollection(SHOW_ID, "group", [group()], 10, 11);
	const onError = vi.fn();
	const writer = new PlaybackTopologyWriter({
		showId: SHOW_ID,
		store,
		transport: { apply },
		loadObject: loader,
		onError,
	});
	return { store, writer, loadObject: loader, onError };
}
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((r) => {
		resolve = r;
	});
	return { promise, resolve };
}
describe("PlaybackTopologyWriter", () => {
	it("saves a Cuelist through its semantic ID and retains the legacy storage key", async () => {
		const apply = vi.fn(async (_show, _revision, request) =>
			changed(request, [present(cueList(2, "legacy-main-list", "Changed"))]),
		);
		const { store, writer } = setup(apply);

		await writer.saveCueList(CUE_LIST_ID, 1, "legacy-main-list", {
			...cueList().body,
			name: "Changed",
		});

		expect(apply).toHaveBeenCalledWith(
			SHOW_ID,
			11,
			expect.objectContaining({
				action: expect.objectContaining({
					type: "save_cue_list",
					cueListId: CUE_LIST_ID,
					expectedRevision: 1,
				}),
			}),
		);
		expect(store.getSnapshot().cueLists[0]).toMatchObject({
			id: "legacy-main-list",
			revision: 2,
			body: { id: CUE_LIST_ID, name: "Changed" },
		});
	});

	it("serializes Cuelist undo and redo and installs each authoritative revision", async () => {
		const apply = vi.fn(async (_show, _revision, request) =>
			changed(request, [present(cueList(2, "legacy-main-list", "History"))]),
		);
		const { store, writer } = setup(apply);

		await writer.undoCueList(CUE_LIST_ID, 1, "legacy-main-list");
		expect(apply.mock.calls[0]?.[2].action).toEqual({
			type: "undo_cue_list",
			cueListId: CUE_LIST_ID,
			expectedRevision: 1,
			expectedObjectId: "legacy-main-list",
		});
		expect(store.getSnapshot().cueLists[0]).toMatchObject({
			revision: 2,
			body: { name: "History" },
		});

		apply.mockImplementationOnce(async (_show, _revision, request) =>
			changed(
				request,
				[present(cueList(3, "legacy-main-list", "Forward"))],
				13,
				42,
			),
		);
		await writer.redoCueList(CUE_LIST_ID, 2, "legacy-main-list");
		expect(apply.mock.calls[1]?.[2].action.type).toBe("redo_cue_list");
		expect(store.getSnapshot().cueLists[0]).toMatchObject({
			revision: 3,
			body: { name: "Forward" },
		});
	});

	it("installs a Cuelist save that lands via the show-revision 409 retry", async () => {
		const conflict = Object.assign(new Error("stale Show revision"), {
			status: 409,
			retryable: false,
			currentRevision: 12,
		});
		const loadObject = vi.fn(async (_show: string, kind: ShowObjectKind) =>
			kind === "cue_list" ? cueList(1) : null,
		);
		const apply = vi
			.fn()
			.mockRejectedValueOnce(conflict)
			.mockImplementation(async (_show, _revision, request) =>
				changed(
					request,
					[present(cueList(2, "legacy-main-list", "Retried"))],
					13,
					42,
				),
			);
		const { store, writer } = setup(
			apply,
			loadObject as unknown as PlaybackTopologyWriterOptions["loadObject"],
		);

		// A concurrent collection re-hydration stamps the cue_list kind floor at the
		// current show event sequence while its payload still predates the retried
		// commit — the retried response must still install its newer revision.
		store.setCollection(SHOW_ID, "cue_list", [cueList(1)], 42, 12);
		const outcome = await writer.saveCueList(
			CUE_LIST_ID,
			1,
			"legacy-main-list",
			{ ...cueList().body, name: "Retried" },
		);

		expect(outcome).not.toBeNull();
		expect(apply).toHaveBeenCalledTimes(2);
		expect(store.getSnapshot().cueLists[0]).toMatchObject({
			id: "legacy-main-list",
			revision: 2,
			body: { name: "Retried" },
		});
	});

	it.each([
		"assignPlayback",
		"updatePlayback",
	] as const)("%s sends exact page-owned definition and installs atomically", async (method) => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q),
		);
		const { store, writer } = setup(apply);
		const before = store.getSnapshot();
		const notify = vi.fn();
		store.subscribe(notify);
		await writer[method](ADDRESS, definition("Response"));
		expect(apply).toHaveBeenCalledWith(
			SHOW_ID,
			11,
			expect.objectContaining({
				requestId: expect.any(String),
				action: {
					type:
						method === "assignPlayback" ? "assign_playback" : "update_playback",
					address: ADDRESS,
					definition: definition("Response"),
					expectedPageRevision: 1,
					expectedPageObjectId: "page-four",
				},
			}),
		);
		expect(
			store.getSnapshot().playbackPages[0].body.assignments[680].name,
		).toBe("Response");
		expect(store.getSnapshot().cueLists).toBe(before.cueLists);
		expect(notify).toHaveBeenCalledOnce();
	});
	it("clears only the addressed assignment using page revision", async () => {
		const cleared = page(2);
		cleared.body.assignments = {};
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q, [present(cleared)]),
		);
		const { writer, store } = setup(apply);
		await writer.clearPlayback(ADDRESS);
		expect(apply.mock.calls[0][2].action).toEqual({
			type: "clear_playback",
			address: ADDRESS,
			expectedPageRevision: 1,
			expectedPageObjectId: "page-four",
		});
		expect(store.getSnapshot().playbackPages[0].body.assignments).toEqual({});
	});
	it("assigns Group Master with exact group and page guards", async () => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q),
		);
		const { writer } = setup(apply);
		await writer.assignGroupMaster("group-front", 6, ADDRESS);
		expect(apply.mock.calls[0][2].action).toEqual({
			type: "assign_group_master",
			groupObjectId: "group-front",
			expectedGroupRevision: 6,
			address: ADDRESS,
			expectedPageRevision: 1,
			expectedPageObjectId: "page-four",
		});
		await writer.assignGroupMaster("group-front", 5, ADDRESS);
		expect(apply).toHaveBeenCalledOnce();
	});
	it("creates absent sparse page with deterministic absence guards", async () => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q, []),
		);
		const { writer } = setup(apply);
		await writer.createPage(99, undefined, " Last ");
		expect(apply.mock.calls[0][2].action).toEqual({
			type: "create_playback_page",
			number: 99,
			name: "Last",
			expectedPageRevision: 0,
			expectedPageObjectId: null,
		});
	});
	it.each([
		"renamePage",
		"deletePage",
	] as const)("%s retains existing exact storage identity", async (method) => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q, []),
		);
		const { writer } = setup(apply);
		if (method === "renamePage") await writer.renamePage(4, " Changed ");
		else await writer.deletePage(4);
		expect(apply.mock.calls[0][2].action).toMatchObject({
			number: 4,
			expectedPageRevision: 1,
			expectedPageObjectId: "page-four",
		});
	});
	it("rejects invalid names and missing existing page without transport", async () => {
		const apply = vi.fn();
		const { writer, onError } = setup(apply);
		await writer.createPage(5, undefined, "   ");
		await writer.deletePage(5);
		expect(apply).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledTimes(2);
	});
	it("refuses assignment while page authority is loading", async () => {
		const apply = vi.fn();
		const { writer, store } = setup(apply);
		store.reset(SHOW_ID, "replacement");
		await writer.assignPlayback(ADDRESS, definition());
		expect(apply).not.toHaveBeenCalled();
	});
	it("retries network failure with same intent ID", async () => {
		const failure = Object.assign(new Error("network"), {
			status: 503,
			retryable: true,
		});
		const apply = vi
			.fn()
			.mockRejectedValueOnce(failure)
			.mockImplementation(async (_s, _r, q) => changed(q));
		const { writer } = setup(apply);
		await writer.updatePlayback(ADDRESS, definition());
		expect(apply).toHaveBeenCalledTimes(2);
		expect(apply.mock.calls[1][2]).toBe(apply.mock.calls[0][2]);
	});
	it("serializes writes and uses committed page revision", async () => {
		let n = 1;
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q, [present(page(++n))], 10 + n, 40 + n),
		);
		const { writer } = setup(apply);
		await Promise.all([
			writer.updatePlayback(ADDRESS, definition()),
			writer.clearPlayback(ADDRESS),
		]);
		expect(apply.mock.calls[1][1]).toBe(12);
		expect(apply.mock.calls[1][2].action).toMatchObject({
			expectedPageRevision: 2,
		});
	});
	it("preserves captured edit basis instead of silently replacing stale revisions", async () => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) =>
			changed(q),
		);
		const { writer } = setup(apply);
		await writer.updatePlayback(ADDRESS, definition(), {
			expectedPageRevision: 0,
			expectedPageObjectId: "old-page",
		});
		expect(apply.mock.calls[0][2].action).toMatchObject({
			expectedPageRevision: 0,
			expectedPageObjectId: "old-page",
		});
	});
	it.each([
		"stop",
		"replace",
	] as const)("drops late outcome after %s", async (mode) => {
		const pending = deferred<PlaybackTopologyOutcome>();
		const apply = vi.fn<PlaybackTopologyTransport["apply"]>(
			() => pending.promise,
		);
		const { writer, store } = setup(apply);
		const result = writer.updatePlayback(ADDRESS, definition());
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		if (mode === "stop") writer.stop();
		else store.reset(SHOW_ID, "replacement");
		pending.resolve(changed(apply.mock.calls[0][2]));
		expect(await result).toBeNull();
		expect(store.getSnapshot().playbackPages[0]?.revision ?? 0).not.toBe(2);
	});
	it("rejects response for another exact address without installing", async () => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) => ({
			...changed(q),
			resolution: { kind: "playback" as const, page: 4, position: 681 },
		}));
		const { writer, store, onError } = setup(apply);
		expect(await writer.updatePlayback(ADDRESS, definition())).toBeNull();
		expect(store.getSnapshot().playbackPages[0].revision).toBe(1);
		expect(onError.mock.calls[0][0].message).toContain(
			"address does not match",
		);
	});
	it("repairs stale page conflict through captured storage ID without retrying changed object", async () => {
		const failure = Object.assign(new Error("stale page"), {
			status: 409,
			retryable: false,
			currentRevision: 14,
		});
		const apply = vi.fn().mockRejectedValue(failure);
		const loader = vi.fn(async () => page(3, "External"));
		const { writer, store } = setup(
			apply,
			loader as PlaybackTopologyWriterOptions["loadObject"],
		);
		expect(await writer.updatePlayback(ADDRESS, definition())).toBeNull();
		expect(loader).toHaveBeenCalledWith(SHOW_ID, "playback_page", "page-four");
		expect(store.getSnapshot().playbackPages[0].revision).toBe(3);
		expect(apply).toHaveBeenCalledOnce();
	});
	it("keeps occupied-destination rejection nonoptimistic", async () => {
		const apply = vi.fn().mockRejectedValue(
			Object.assign(new Error("destination occupied"), {
				status: 400,
				retryable: false,
			}),
		);
		const { writer, store } = setup(apply);
		const before = store.getSnapshot().playbackPages;
		expect(
			await writer.assignPlayback(ADDRESS, definition("Replacement")),
		).toBeNull();
		expect(store.getSnapshot().playbackPages).toBe(before);
	});

	it("preserves a newer page event arriving before the HTTP outcome", async () => {
		const pending = deferred<PlaybackTopologyOutcome>();
		const apply = vi.fn<PlaybackTopologyTransport["apply"]>(
			() => pending.promise,
		);
		const { writer, store } = setup(apply);
		const result = writer.updatePlayback(ADDRESS, definition());
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 13,
			eventSequence: 42,
			changes: [
				{
					kind: "playback_page",
					objectId: "page-four",
					objectRevision: 3,
					body: page(3, "Later").body,
					deleted: false,
				},
			],
		});
		pending.resolve(changed(apply.mock.calls[0][2]));
		await result;
		expect(
			store.getSnapshot().playbackPages[0].body.assignments[680].name,
		).toBe("Later");
		expect(store.getSnapshot().showRevision).toBe(13);
	});
	it("keeps a no-change replay projection stable", async () => {
		const apply = vi.fn(async (_s, _r, q: PlaybackTopologyRequest) => {
			const { eventSequence, ...base } = changed(q, [present(page())], 11);
			return { ...base, status: "no_change" as const, replayed: true };
		});
		const { writer, store } = setup(apply);
		const before = store.getSnapshot().playbackPages;
		await writer.updatePlayback(ADDRESS, definition());
		expect(store.getSnapshot().playbackPages).toBe(before);
	});
	it.each([
		"same-show",
		"different-show",
		"stopped",
	] as const)("handles queued intent after %s authority change", async (mode) => {
		const pending = deferred<PlaybackTopologyOutcome>();
		const apply = vi
			.fn()
			.mockImplementationOnce(() => pending.promise)
			.mockImplementation(async (_s, _r, q) => changed(q));
		const { writer, store } = setup(apply);
		const first = writer.updatePlayback(ADDRESS, definition());
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const second = writer.clearPlayback(ADDRESS);
		if (mode === "stopped") writer.stop();
		else {
			store.reset(
				mode === "same-show" ? SHOW_ID : "different-show",
				"replacement",
			);
			store.setCollection(
				mode === "same-show" ? SHOW_ID : "different-show",
				"playback_page",
				[page(4)],
				50,
				15,
			);
		}
		pending.resolve(changed(apply.mock.calls[0][2]));
		await first;
		await second;
		expect(apply).toHaveBeenCalledTimes(mode === "same-show" ? 2 : 1);
		if (mode === "same-show")
			expect(apply.mock.calls[1][2].action).toMatchObject({
				expectedPageRevision: 4,
			});
	});
	it("retries only a stale Show revision when target page remains unchanged", async () => {
		const failure = Object.assign(new Error("stale show"), {
			status: 409,
			retryable: false,
			currentRevision: 12,
		});
		const apply = vi
			.fn()
			.mockRejectedValueOnce(failure)
			.mockImplementation(async (_s, _r, q) =>
				changed(q, [present(page(2))], 13, 42),
			);
		const loader = vi.fn(async () => page());
		const { writer } = setup(
			apply,
			loader as PlaybackTopologyWriterOptions["loadObject"],
		);
		expect(await writer.updatePlayback(ADDRESS, definition())).not.toBeNull();
		expect(apply.mock.calls.map((c) => c[1])).toEqual([11, 12]);
	});
});

describe("new page response and collection boundary race", () => {
	it("installs the first Group master assignment and projects the secondary cell at the equal collection floor", async () => {
		const master: PlaybackDefinition = {
			...definition("Beam Stage"),
			target: { type: "group", group_id: "1" },
			buttons: ["select", "none", "none"],
			has_fader: true,
		};
		const created: ShowObject<"playback_page"> = {
			kind: "playback_page",
			id: "2",
			revision: 1,
			updated_at: "",
			body: { number: 2, name: "Page 2", assignments: { 301: master } },
		};
		let store!: ShowObjectsStore;
		const apply = vi.fn(async (_s, _r, request: PlaybackTopologyRequest) => {
			store.setCollection(SHOW_ID, "playback_page", [], 41, 12);
			return changed(request, [present(created)], 12, 41);
		});
		const h = setup(apply);
		store = h.store;
		store.setCollection(SHOW_ID, "playback_page", [], 10, 11);
		store.setCollection(
			SHOW_ID,
			"group",
			[
				{
					...group(),
					id: "1",
					body: { name: "Beam Stage", fixtures: ["fixture-1"] },
				},
			],
			10,
			11,
		);
		const result = await h.writer.assignPlayback(
			{ page: 2, position: 301 },
			master,
		);
		expect(result?.status).toBe("changed");
		const snapshot = store.getSnapshot();
		expect(snapshot.playbackPages).toEqual([created]);
		const slots = projectPlaybackSlots({
			cueLists: [],
			groups: snapshot.groups,
			page: snapshot.playbackPages[0]?.body,
			playbackLayout: {
				rows: [
					{
						first_position: 301,
						count: 5,
						presentation: "push_encoder",
						button_count: 1,
					},
				],
			},
			columns: 5,
			firstSlot: 301,
			pageSize: 5,
		});
		expect(slots[0].playback).toMatchObject({
			address: { page: 2, position: 301 },
			name: "Beam Stage",
			target: { type: "group", group_id: "1" },
			has_fader: true,
		});
		expect(slots[0].group?.body.name).toBe("Beam Stage");
		expect(slots.slice(1).every((slot) => slot.playback === null)).toBe(true);
	});
});
