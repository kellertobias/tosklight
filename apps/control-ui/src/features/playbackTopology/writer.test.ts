import { describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import type {
	ShowObject,
	ShowObjectKind,
	ShowObjectsChange,
} from "../showObjects/contracts";
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

function playback(
	revision: number,
	name = "Original",
	id = "legacy-seven",
	number = 7,
): ShowObject<"playback"> {
	return {
		kind: "playback",
		id,
		revision,
		updated_at: "",
		body: playbackBody(name, number),
	};
}

function playbackBody(name = "Original", number = 7): PlaybackDefinition {
	return {
		number,
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
	revision: number,
	slots: Record<string, number> = { 2: 7 },
	id = "legacy-page-four",
): ShowObject<"playback_page"> {
	return {
		kind: "playback_page",
		id,
		revision,
		updated_at: "",
		body: { number: 4, name: "Page 4", slots },
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

function present<K extends "cue_list" | "playback" | "playback_page">(
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
	objects: PlaybackTopologyObject[] = [
		present(playback(2, "Response")),
		present(page(2)),
	],
	showRevision = 12,
	eventSequence = 41,
): PlaybackTopologyOutcome {
	const action = request.action;
	if (action.type === "save_cue_list")
		return {
			status: "changed",
			requestId: request.requestId,
			correlationId: CORRELATION_ID,
			showRevision,
			resolution: { kind: "cue_list", cueListId: action.cueListId },
			objects,
			eventSequence,
			replayed: false,
		};
	return {
		status: "changed",
		requestId: request.requestId,
		correlationId: CORRELATION_ID,
		showRevision,
		resolution: {
			kind: "page_slot",
			page: action.page,
			slot: action.slot,
			playbackNumber: 7,
		},
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
		(vi.fn(
			async () => null as ShowObject | null,
		) as unknown as PlaybackTopologyWriterOptions["loadObject"]);
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "session-a");
	store.setCollection(SHOW_ID, "cue_list", [cueList()], 10, 11);
	store.setCollection(SHOW_ID, "playback", [playback(1)], 10, 11);
	store.setCollection(SHOW_ID, "playback_page", [page(1)], 10, 11);
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
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
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

	it("sends one revisioned application action and installs its objects atomically", async () => {
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				request: PlaybackTopologyRequest,
			) => changed(request),
		);
		const { store, writer } = setup(apply);
		const before = store.getSnapshot();
		const observed: Array<ReturnType<typeof store.getSnapshot>> = [];
		store.subscribe(() => observed.push(store.getSnapshot()));

		await expect(
			writer.configureSlot(4, 2, playbackBody("Response")),
		).resolves.toMatchObject({ status: "changed" });

		expect(apply).toHaveBeenCalledOnce();
		expect(apply.mock.calls[0][0]).toBe(SHOW_ID);
		expect(apply.mock.calls[0][1]).toBe(11);
		expect(apply.mock.calls[0][2]).toMatchObject({
			action: {
				type: "configure_slot",
				expectedPageRevision: 1,
				expectedPlaybackRevision: 1,
			},
		});
		const after = store.getSnapshot();
		expect(after.showRevision).toBe(12);
		expect(after.playbacks[0].body.name).toBe("Response");
		expect(after.playbackPages[0].revision).toBe(2);
		expect(after.cueLists).toBe(before.cueLists);
		expect(observed).toHaveLength(1);
		expect(observed[0].playbacks[0].revision).toBe(2);
		expect(observed[0].playbackPages[0].revision).toBe(2);
	});

	it("maps an existing Playback by exact source identity and publishes only its Page", async () => {
		const mappedPage = page(2, { 2: 7, 3: 7 });
		const apply = vi.fn(async (_show, _revision, request) =>
			changed(request, [present(mappedPage)]),
		);
		const { store, writer } = setup(apply);
		const source = store.getSnapshot().playbacks;
		let publications = 0;
		store.subscribe(() => publications++);

		await expect(writer.mapExistingPlayback(4, 3, 7)).resolves.toMatchObject({
			status: "changed",
		});
		store.applyChange(
			showChange(41, [
				{
					...mappedPage,
					body: { ...mappedPage.body, name: "Late duplicate event" },
				},
			]),
		);

		expect(apply).toHaveBeenCalledOnce();
		expect(apply.mock.calls[0]).toMatchObject([
			SHOW_ID,
			11,
			{
				action: {
					type: "map_existing_playback",
					page: 4,
					slot: 3,
					playbackNumber: 7,
					expectedPageRevision: 1,
					expectedPageObjectId: "legacy-page-four",
					expectedPlaybackRevision: 1,
					expectedPlaybackObjectId: "legacy-seven",
				},
			},
		]);
		expect(store.getSnapshot().playbacks).toBe(source);
		expect(store.getSnapshot().playbackPages).toEqual([mappedPage]);
		expect(publications).toBe(1);
	});

	it("carries an exact absent Page identity when creating a new mapping", async () => {
		const createdPage = {
			...page(1, { 1: 7 }, "5"),
			body: { number: 5, name: "Page 5", slots: { 1: 7 } },
		};
		const apply = vi.fn(async (_show, _revision, request) =>
			changed(request, [present(createdPage)]),
		);
		const { writer } = setup(apply);

		await writer.mapExistingPlayback(5, 1, 7);

		expect(apply.mock.calls[0][2]).toMatchObject({
			action: {
				type: "map_existing_playback",
				expectedPageRevision: 0,
				expectedPageObjectId: null,
				expectedPlaybackRevision: 1,
				expectedPlaybackObjectId: "legacy-seven",
			},
		});
	});

	it("replays a retryable request once with the exact same request ID", async () => {
		const retryable = Object.assign(new Error("offline"), {
			status: 0,
			retryable: true,
		});
		const apply = vi
			.fn()
			.mockRejectedValueOnce(retryable)
			.mockImplementation(
				async (
					_show: string,
					_revision: number,
					request: PlaybackTopologyRequest,
				) => changed(request, [present(page(2, { 2: 7, 3: 7 }))]),
			);
		const { writer } = setup(apply);

		await writer.mapExistingPlayback(4, 3, 7);

		expect(apply).toHaveBeenCalledTimes(2);
		expect(apply.mock.calls[1][2]).toBe(apply.mock.calls[0][2]);
		expect(apply.mock.calls[1][2].requestId).toBe(
			apply.mock.calls[0][2].requestId,
		);
	});

	it("serializes concurrent intents so the second uses committed revisions", async () => {
		const pending = deferred<PlaybackTopologyOutcome>();
		let firstRequest!: PlaybackTopologyRequest;
		const apply = vi.fn(async (_show, _revision, input) => {
			if (apply.mock.calls.length === 1) {
				firstRequest = input;
				return pending.promise;
			}
			return changed(
				input,
				[present(playback(3, "Second")), present(page(3))],
				13,
				42,
			);
		});
		const { writer } = setup(apply);

		const first = writer.configureSlot(4, 2, playbackBody("First"));
		const second = writer.configureSlot(4, 2, playbackBody("Second"));
		await Promise.resolve();
		expect(apply).toHaveBeenCalledOnce();

		pending.resolve(changed(firstRequest));
		await Promise.all([first, second]);

		expect(apply).toHaveBeenCalledTimes(2);
		expect(apply.mock.calls[1][1]).toBe(12);
		expect(apply.mock.calls[1][2]).toMatchObject({
			action: {
				expectedPageRevision: 2,
				expectedPlaybackRevision: 2,
			},
		});
	});

	it("preserves the edit-base revisions captured before a queued configuration", async () => {
		const apply = vi.fn(async (_show, _revision, request) => changed(request));
		const { store, writer } = setup(apply);
		store.setCollection(SHOW_ID, "playback", [playback(2)], 20, 21);
		store.setCollection(SHOW_ID, "playback_page", [page(2)], 20, 21);

		await writer.configureSlot(4, 2, playbackBody("Stale edit"), {
			expectedPageRevision: 1,
			expectedPageObjectId: "legacy-page-four",
			expectedPlaybackRevision: 1,
			expectedPlaybackObjectId: "legacy-seven",
		});

		expect(apply.mock.calls[0][2]).toMatchObject({
			action: {
				expectedPageRevision: 1,
				expectedPlaybackRevision: 1,
			},
		});
	});

	it("preserves a mapped Page event that arrives before its HTTP outcome", async () => {
		const pending = deferred<PlaybackTopologyOutcome>();
		let request!: PlaybackTopologyRequest;
		const apply = vi.fn(async (_show, _revision, input) => {
			request = input;
			return pending.promise;
		});
		const { store, writer } = setup(apply);
		const operation = writer.mapExistingPlayback(4, 3, 7);
		await Promise.resolve();
		const eventPage = page(2, { 2: 7, 3: 7 });
		store.applyChange(showChange(41, [eventPage]));
		const afterEvent = store.getSnapshot();

		pending.resolve(changed(request, [present(eventPage)]));
		await operation;

		expect(store.getSnapshot().playbackPages).toBe(afterEvent.playbackPages);
		expect(store.getSnapshot().playbackPages[0].body.slots["3"]).toBe(7);
	});

	it("makes a replayed no-change outcome projection-stable", async () => {
		const apply = vi.fn(async (_show, _revision, request) => ({
			...changed(request, [present(page(1))]),
			status: "no_change" as const,
			showRevision: 11,
			replayed: true,
			eventSequence: undefined as never,
		}));
		const { store, writer } = setup(apply);
		const before = store.getSnapshot();

		await writer.mapExistingPlayback(4, 2, 7);

		const after = store.getSnapshot();
		expect(after.playbacks).toBe(before.playbacks);
		expect(after.playbackPages).toBe(before.playbackPages);
	});

	it("clears one Playback and every returned Page in one publication", async () => {
		const second = {
			...page(2, { 7: 7 }, "legacy-page-two"),
			body: { number: 2, name: "Page 2", slots: { 7: 7 } },
		};
		const apply = vi.fn(async (_show, _revision, request) =>
			changed(request, [
				present(page(2, {})),
				present({
					...second,
					revision: 3,
					body: { ...second.body, slots: {} },
				}),
				{
					state: "deleted",
					kind: "playback",
					objectId: "legacy-seven",
					objectRevision: 2,
				},
			]),
		);
		const { store, writer } = setup(apply);
		store.setCollection(SHOW_ID, "playback_page", [page(1), second], 10, 11);
		let publications = 0;
		store.subscribe(() => publications++);

		await writer.clearMappedPlayback(4, 2);

		expect(store.getSnapshot().playbacks).toEqual([]);
		expect(store.getSnapshot().playbackPages).toHaveLength(2);
		expect(
			store
				.getSnapshot()
				.playbackPages.every(
					(object) => !Object.values(object.body.slots).includes(7),
				),
		).toBe(true);
		expect(publications).toBe(1);
	});

	it("repairs a revision conflict by legacy storage identity without optimistic drift", async () => {
		const conflict = Object.assign(new Error("stale Page"), {
			status: 409,
			retryable: false,
			currentRevision: 13,
		});
		const repairedPage = page(2, {}, "legacy-page-four");
		const loadObject = vi.fn(
			async (_show: string, kind: ShowObjectKind, id: string) => {
				if (kind === "playback_page" && id === "legacy-page-four")
					return repairedPage;
				if (kind === "playback" && id === "legacy-seven") return null;
				return null;
			},
		);
		const { store, writer, onError } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadObject as unknown as PlaybackTopologyWriterOptions["loadObject"],
		);

		await expect(writer.clearMappedPlayback(4, 2)).resolves.toBeNull();

		expect(loadObject).toHaveBeenCalledWith(
			SHOW_ID,
			"playback_page",
			"legacy-page-four",
		);
		expect(loadObject).toHaveBeenCalledWith(
			SHOW_ID,
			"playback",
			"legacy-seven",
		);
		expect(store.getSnapshot().showRevision).toBe(13);
		expect(store.getSnapshot().playbackPages).toEqual([repairedPage]);
		expect(store.getSnapshot().playbacks).toEqual([]);
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("repairs an existing map through the destination Page and source Playback", async () => {
		const conflict = Object.assign(new Error("stale source Playback"), {
			status: 409,
			retryable: false,
			currentRevision: 13,
		});
		const source = playback(4, "Source", "source-eight", 8);
		const repairedPage = page(2, { 2: 8 });
		const loadObject = vi.fn(
			async (_show: string, kind: ShowObjectKind, id: string) => {
				if (kind === "playback_page" && id === "legacy-page-four")
					return repairedPage;
				if (kind === "playback" && id === "source-eight") return source;
				return null;
			},
		);
		const { store, writer } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadObject as unknown as PlaybackTopologyWriterOptions["loadObject"],
		);
		store.setCollection(
			SHOW_ID,
			"playback",
			[playback(1), playback(3, "Source", "source-eight", 8)],
			10,
			11,
		);

		await expect(writer.mapExistingPlayback(4, 2, 8)).resolves.toBeNull();

		expect(loadObject).toHaveBeenCalledWith(
			SHOW_ID,
			"playback_page",
			"legacy-page-four",
		);
		expect(loadObject).toHaveBeenCalledWith(
			SHOW_ID,
			"playback",
			"source-eight",
		);
		expect(loadObject).not.toHaveBeenCalledWith(
			SHOW_ID,
			"playback",
			"legacy-seven",
		);
		expect(store.getSnapshot().showRevision).toBe(13);
	});

	it("repairs an absent Page conflict through its deterministic Page identity", async () => {
		const conflict = Object.assign(new Error("Page was created concurrently"), {
			status: 409,
			retryable: false,
			currentRevision: 13,
		});
		const concurrentPage = {
			...page(1, {}, "5"),
			body: { number: 5, name: "Concurrent", slots: {} },
		};
		const repairedSource = playback(2);
		const loadObject = vi.fn(
			async (_show: string, kind: ShowObjectKind, id: string) => {
				if (kind === "playback_page" && id === "5") return concurrentPage;
				if (kind === "playback" && id === "legacy-seven") return repairedSource;
				return null;
			},
		);
		const { store, writer, onError } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadObject as unknown as PlaybackTopologyWriterOptions["loadObject"],
		);

		await expect(writer.mapExistingPlayback(5, 1, 7)).resolves.toBeNull();

		expect(loadObject).toHaveBeenCalledWith(SHOW_ID, "playback_page", "5");
		expect(loadObject).toHaveBeenCalledWith(
			SHOW_ID,
			"playback",
			"legacy-seven",
		);
		expect(
			store.getSnapshot().playbackPages.find((object) => object.id === "5"),
		).toEqual(concurrentPage);
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("leaves the mapped Page and source unchanged when an action rolls back", async () => {
		const rejected = Object.assign(new Error("forbidden"), {
			status: 403,
			retryable: false,
		});
		const { store, writer, onError } = setup(
			vi.fn(async () => {
				throw rejected;
			}),
		);
		const before = store.getSnapshot();

		await expect(writer.mapExistingPlayback(4, 3, 7)).resolves.toBeNull();

		expect(store.getSnapshot().playbacks).toBe(before.playbacks);
		expect(store.getSnapshot().playbackPages).toBe(before.playbackPages);
		expect(onError).toHaveBeenLastCalledWith(rejected);
	});

	it("repairs a Cuelist conflict through its legacy storage key", async () => {
		const conflict = Object.assign(new Error("stale Cuelist"), {
			status: 409,
			retryable: false,
			currentRevision: 13,
		});
		const repaired = cueList(2, "legacy-main-list", "Concurrent");
		const loadObject = vi.fn(async () => repaired);
		const { store, writer } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadObject as unknown as PlaybackTopologyWriterOptions["loadObject"],
		);

		await writer.saveCueList(CUE_LIST_ID, 1, "legacy-main-list", {
			...cueList().body,
			name: "Local",
		});

		expect(loadObject).toHaveBeenCalledWith(
			SHOW_ID,
			"cue_list",
			"legacy-main-list",
		);
		expect(store.getSnapshot().cueLists).toEqual([repaired]);
	});

	it("drops a late map outcome after same-show authority replacement", async () => {
		const pending = deferred<PlaybackTopologyOutcome>();
		let request!: PlaybackTopologyRequest;
		const { store, writer, onError } = setup(
			vi.fn(async (_show, _revision, input) => {
				request = input;
				return pending.promise;
			}),
		);
		const operation = writer.mapExistingPlayback(4, 3, 7);
		await Promise.resolve();
		store.reset(SHOW_ID, "session-b");
		pending.resolve(changed(request, [present(page(2, { 2: 7, 3: 7 }))]));

		await expect(operation).resolves.toBeNull();
		expect(store.getSnapshot().playbacks).toEqual([]);
		expect(onError).not.toHaveBeenCalled();
	});
});

function showChange(
	sequence: number,
	objects: Array<ShowObject<"playback"> | ShowObject<"playback_page">>,
): ShowObjectsChange {
	return {
		showId: SHOW_ID,
		showRevision: 12,
		eventSequence: sequence,
		changes: objects.map((object) => ({
			kind: object.kind,
			objectId: object.id,
			objectRevision: object.revision,
			body: object.body,
			deleted: false,
		})) as ShowObjectsChange["changes"],
	};
}
