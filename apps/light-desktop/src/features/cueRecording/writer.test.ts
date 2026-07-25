import { describe, expect, it, vi } from "vitest";
import { CueRecordingActionError } from "../../api/CueRecordingTransport";
import type { PlaybackProjection } from "../playbackRuntime/contracts";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import type {
	ShowObject,
	ShowObjectKind,
	ShowObjectsChange,
} from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	CueRecordingOutcome,
	CueRecordingRequest,
	CueRecordingTransport,
	RecordCueInput,
} from "./contracts";
import {
	CueRecordingWriter,
	type CueRecordingWriterOptions,
} from "./writer";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
const CUE_ID = "33333333-3333-4333-8333-333333333333";
const CORRELATION_ID = "44444444-4444-4444-8444-444444444444";

function cueList(
	revision: number,
	name: string,
	id = CUE_LIST_ID,
): ShowObject<"cue_list"> {
	return {
		kind: "cue_list",
		id,
		revision,
		updated_at: "",
		body: {
			id,
			name: "Main",
			priority: 0,
			mode: "sequence",
			looped: false,
			cues: [
				{
					id: CUE_ID,
					number: 1,
					name,
					fade_millis: 1000,
					delay_millis: 0,
					trigger: { type: "manual" },
					cue_only: false,
					changes: [],
					group_changes: [],
					phasers: [],
				},
			],
		},
	};
}

function playback(
	revision: number,
	name = "Main",
	number = 7,
	cueListId = CUE_LIST_ID,
): ShowObject<"playback"> {
	return {
		kind: "playback",
		id: String(number),
		revision,
		updated_at: "",
		body: {
			number,
			name,
			target: { type: "cue_list", cue_list_id: cueListId },
			buttons: ["go_minus", "go", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
		},
	};
}

function page(
	revision: number,
	slots: Record<string, number> = { 2: 7 },
	number = 4,
): ShowObject<"playback_page"> {
	return {
		kind: "playback_page",
		id: String(number),
		revision,
		updated_at: "",
		body: { number, name: `Page ${number}`, slots },
	};
}

function runtimeProjection(
	target: "cue_list" | "missing" = "cue_list",
	showRevision = 8,
): PlaybackProjection {
	const common = {
		scope: { show_id: SHOW_ID, show_revision: showRevision },
		requested: { kind: "playback" as const, playback_number: 7 },
		playback_number: 7,
	};
	return target === "missing"
		? { ...common, target }
		: { ...common, target, cue_list_id: CUE_LIST_ID, runtime: null };
}

function outcome(
	requestId: string,
	options: {
		status?: "changed" | "no_change";
		replayed?: boolean;
		cueList?: ShowObject<"cue_list">;
		playback?: ShowObject<"playback"> | null;
		page?: ShowObject<"playback_page"> | null;
		runtime?: PlaybackProjection | null;
	} = {},
): CueRecordingOutcome {
	const base = {
		requestId,
		correlationId: CORRELATION_ID,
		replayed: options.replayed ?? false,
		capturedSource: "normal" as const,
		showRevision: options.status === "no_change" ? 7 : 8,
		recordedCue: { id: CUE_ID, number: 1, deleted: false },
		projections: {
			cueList: options.cueList ?? cueList(2, "Response"),
			playback: options.playback === undefined ? playback(2) : options.playback,
			page: options.page === undefined ? page(2) : options.page,
		},
	};
	if (options.status === "no_change") return { ...base, status: "no_change" };
	return {
		...base,
		status: "changed",
		showEventSequence: 12,
		runtime:
			options.runtime === undefined
				? { projection: runtimeProjection(), eventSequence: 21 }
				: options.runtime == null
					? null
					: { projection: options.runtime, eventSequence: 21 },
	};
}

function input(
	target: RecordCueInput["target"] = {
		kind: "page_slot",
		page: 4,
		slot: 2,
	},
): RecordCueInput {
	return {
		target,
		operation: "overwrite",
		cueNumber: 1,
		timing: {},
		cueOnly: false,
		capturePolicy: "current_capture",
		activationPolicy: "hold",
	};
}

function setup(
	record: CueRecordingTransport["record"],
	loadObject = vi.fn(
		async (_showId: string, _kind: ShowObjectKind, _objectId: string) =>
			null as ShowObject | null,
	),
	selectedPlayback: () => number | null = () => 7,
) {
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "user-a:session-a");
	store.setCollection(SHOW_ID, "cue_list", [cueList(1, "Original")], 10);
	store.setCollection(SHOW_ID, "playback", [playback(1)], 10);
	store.setCollection(SHOW_ID, "playback_page", [page(1)], 10);
	store.installShowRevision(SHOW_ID, 7);
	const playbackRuntimeStore = new PlaybackRuntimeStore();
	playbackRuntimeStore.reset(SHOW_ID, "desk-a");
	const onError = vi.fn();
	const writer = new CueRecordingWriter({
		showId: SHOW_ID,
		store,
		playbackRuntimeStore,
		transport: { record },
		selectedPlayback,
		loadObject: loadObject as CueRecordingWriterOptions["loadObject"],
		onError,
	});
	return { store, playbackRuntimeStore, writer, loadObject, onError };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

describe("CueRecordingWriter", () => {
	it("installs one response atomically and ignores its later canonical event", async () => {
		const record = vi.fn(
			async (_showId: string, _revision: number, request: CueRecordingRequest) =>
				outcome(request.requestId),
		);
		const { store, playbackRuntimeStore, writer } = setup(record);
		const unrelatedGroups = store.getSnapshot().groups;

		await expect(writer.record(input())).resolves.toMatchObject({
			status: "changed",
		});
		const afterResponse = store.getSnapshot();
		expect(afterResponse).toMatchObject({ showRevision: 8 });
		expect(afterResponse.cueLists[0].body.cues[0].name).toBe("Response");
		expect(afterResponse.playbacks[0].revision).toBe(2);
		expect(afterResponse.playbackPages[0].revision).toBe(2);
		expect(afterResponse.groups).toBe(unrelatedGroups);
		expect(
			playbackRuntimeStore.getSnapshot().projections.get("playback:7")?.[0]
				.target,
		).toBe("cue_list");

		store.applyChange(
			showChange(12, cueList(2, "Duplicate event"), playback(2), page(2)),
		);
		playbackRuntimeStore.applyProjection(runtimeProjection("missing"), 21);

		expect(store.getSnapshot().cueLists).toBe(afterResponse.cueLists);
		expect(store.getSnapshot().playbacks).toBe(afterResponse.playbacks);
		expect(store.getSnapshot().playbackPages).toBe(afterResponse.playbackPages);
		expect(store.getSnapshot().cueLists[0].body.cues[0].name).toBe("Response");
		expect(
			playbackRuntimeStore.getSnapshot().projections.get("playback:7")?.[0]
				.target,
		).toBe("cue_list");
	});

	it("preserves equal-revision Playback topology from a Cue response", async () => {
		const record = vi.fn(
			async (_showId: string, _revision: number, request: CueRecordingRequest) =>
				outcome(request.requestId, {
					playback: playback(1),
					page: page(1),
				}),
		);
		const { store, writer } = setup(record);
		const storedPlayback = {
			...playback(1),
			updated_at: "2026-07-20T12:00:00Z",
		};
		const storedPage = {
			...page(1),
			updated_at: "2026-07-20T12:00:01Z",
		};
		store.setCollection(SHOW_ID, "playback", [storedPlayback], 10);
		store.setCollection(SHOW_ID, "playback_page", [storedPage], 10);
		const before = store.getSnapshot();

		await expect(writer.record(input())).resolves.toMatchObject({
			status: "changed",
		});

		const after = store.getSnapshot();
		expect(after.cueLists).not.toBe(before.cueLists);
		expect(after.playbacks).toBe(before.playbacks);
		expect(after.playbackPages).toBe(before.playbackPages);
		expect(after.playbacks[0]).toBe(storedPlayback);
		expect(after.playbackPages[0]).toBe(storedPage);
		expect(after.playbacks[0].updated_at).toBe("2026-07-20T12:00:00Z");
		expect(after.playbackPages[0].updated_at).toBe("2026-07-20T12:00:01Z");
	});

	it("retains Show and runtime events that arrive before the HTTP outcome", async () => {
		const pending = deferred<CueRecordingOutcome>();
		const record = vi.fn(
			async (
				_showId: string,
				_revision: number,
				_request: CueRecordingRequest,
			) => pending.promise,
		);
		const { store, playbackRuntimeStore, writer } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][2];

		store.applyChange(
			showChange(12, cueList(2, "Event first"), playback(2), page(2)),
		);
		playbackRuntimeStore.applyProjection(runtimeProjection("cue_list"), 21);
		const afterEvent = store.getSnapshot();
		pending.resolve(
			outcome(request.requestId, {
				cueList: cueList(2, "Late response"),
				runtime: runtimeProjection("missing"),
			}),
		);

		await writing;
		expect(store.getSnapshot().cueLists).toBe(afterEvent.cueLists);
		expect(store.getSnapshot().playbacks).toBe(afterEvent.playbacks);
		expect(store.getSnapshot().playbackPages).toBe(afterEvent.playbackPages);
		expect(store.getSnapshot().cueLists[0].body.cues[0].name).toBe("Event first");
		expect(
			playbackRuntimeStore.getSnapshot().projections.get("playback:7")?.[0]
				.target,
		).toBe("cue_list");
	});

	it("replays one ambiguous request with the identical request ID", async () => {
		const record = vi.fn(
			async (_showId: string, _revision: number, request: CueRecordingRequest) => {
				if (record.mock.calls.length === 1)
					throw new CueRecordingActionError(
						"connection lost",
						"unavailable",
						0,
						null,
						true,
					);
				return outcome(request.requestId);
			},
		);
		const { writer } = setup(record);

		await writer.record(input());

		expect(record).toHaveBeenCalledTimes(2);
		expect(record.mock.calls[0][1]).toBe(7);
		expect(record.mock.calls[0][2].requestId).toBe(
			record.mock.calls[1][2].requestId,
		);
	});

	it("settles replayed no-change without cloning projections or runtime state", async () => {
		const record = vi.fn(
			async (_showId: string, _revision: number, request: CueRecordingRequest) =>
				outcome(request.requestId, {
					status: "no_change",
					replayed: true,
					cueList: cueList(1, "Ignored response"),
					playback: playback(1, "Ignored response"),
					page: page(1, { 2: 999 }),
				}),
		);
		const { store, playbackRuntimeStore, writer } = setup(record);
		const before = store.getSnapshot();
		const runtimeBefore = playbackRuntimeStore.getSnapshot();

		await expect(writer.record(input())).resolves.toMatchObject({
			status: "no_change",
			replayed: true,
		});

		const after = store.getSnapshot();
		expect(after.cueLists).toBe(before.cueLists);
		expect(after.playbacks).toBe(before.playbacks);
		expect(after.playbackPages).toBe(before.playbackPages);
		expect(after.cueLists[0].body.cues[0].name).toBe("Original");
		expect(playbackRuntimeStore.getSnapshot()).toBe(runtimeBefore);
	});

	it("blocks after a cursor gap until scoped hydration restores the Show revision", async () => {
		const record = vi.fn(
			async (_showId: string, _revision: number, request: CueRecordingRequest) =>
				outcome(request.requestId),
		);
		const { store, writer, onError } = setup(record);
		store.beginEventResync();

		expect(await writer.record(input())).toBeNull();
		expect(record).not.toHaveBeenCalled();
		expect(onError).toHaveBeenLastCalledWith(
			expect.objectContaining({
				message: "Authoritative Show revision is loading",
			}),
		);

		store.installShowRevision(SHOW_ID, 7);
		await expect(writer.record(input())).resolves.toMatchObject({
			status: "changed",
		});
		expect(record).toHaveBeenCalledOnce();
	});

	it("rolls back a rejected action without changing scoped authority", async () => {
		const failure = new CueRecordingActionError(
			"not allowed",
			"forbidden",
			403,
			null,
			false,
		);
		const { store, playbackRuntimeStore, writer, loadObject, onError } = setup(
			vi.fn(async () => {
				throw failure;
			}),
		);
		const before = store.getSnapshot();
		const runtimeBefore = playbackRuntimeStore.getSnapshot();

		expect(await writer.record(input())).toBeNull();
		expect(store.getSnapshot()).toBe(before);
		expect(playbackRuntimeStore.getSnapshot()).toBe(runtimeBefore);
		expect(loadObject).not.toHaveBeenCalled();
		expect(onError).toHaveBeenLastCalledWith(failure);
	});

	it("repairs only the Page-to-Playback-to-Cuelist chain on conflict", async () => {
		const conflict = conflictError(9);
		const repairedPage = page(9);
		const repairedPlayback = playback(9, "Repaired playback");
		const repairedCueList = cueList(9, "Repaired Cue");
		const loadObject = vi.fn(
			async (_showId: string, kind: ShowObjectKind, objectId: string) => {
				if (kind === "playback_page" && objectId === "4") return repairedPage;
				if (kind === "playback" && objectId === "7") return repairedPlayback;
				if (kind === "cue_list" && objectId === CUE_LIST_ID)
					return repairedCueList;
				return null;
			},
		);
		const { store, writer, onError } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadObject,
		);

		expect(
			await writer.record(input({ kind: "page_slot", page: 4, slot: 2 })),
		).toBeNull();

		expect(loadObject.mock.calls).toEqual([
			[SHOW_ID, "playback_page", "4"],
			[SHOW_ID, "playback", "7"],
			[SHOW_ID, "cue_list", CUE_LIST_ID],
		]);
		expect(store.getSnapshot()).toMatchObject({ showRevision: 9 });
		expect(store.getSnapshot().playbackPages).toEqual([repairedPage]);
		expect(store.getSnapshot().playbacks).toEqual([repairedPlayback]);
		expect(store.getSnapshot().cueLists).toEqual([repairedCueList]);
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("repairs the request-time selected Playback when selection changes in flight", async () => {
		const pending = deferred<CueRecordingOutcome>();
		let selectedPlayback = 7;
		const loadObject = vi.fn(
			async (_showId: string, kind: ShowObjectKind, objectId: string) => {
				if (kind === "playback" && objectId === "7") return playback(9);
				if (kind === "cue_list" && objectId === CUE_LIST_ID)
					return cueList(9, "Repaired");
				return null;
			},
		);
		const { writer } = setup(
			vi.fn(async () => pending.promise),
			loadObject,
			() => selectedPlayback,
		);
		const writing = writer.record(input({ kind: "selected_playback" }));
		await Promise.resolve();
		selectedPlayback = 8;
		pending.reject(conflictError(9));

		expect(await writing).toBeNull();
		expect(loadObject.mock.calls).toEqual([
			[SHOW_ID, "playback", "7"],
			[SHOW_ID, "cue_list", CUE_LIST_ID],
		]);
	});

	it("does not let a stale conflict repair overwrite a newer event", async () => {
		const repair = deferred<ShowObject | null>();
		const loadObject = vi.fn(async () => repair.promise);
		const { store, writer } = setup(
			vi.fn(async () => {
				throw conflictError(9);
			}),
			loadObject,
		);
		const writing = writer.record(
			input({ kind: "cue_list", cueListId: CUE_LIST_ID }),
		);
		await vi.waitFor(() => expect(loadObject).toHaveBeenCalledOnce());
		store.applyChange(
			showChange(13, cueList(10, "Newer event"), null, null, 10),
		);
		repair.resolve(cueList(9, "Stale repair"));

		expect(await writing).toBeNull();
		expect(store.getSnapshot().cueLists).toEqual([
			cueList(10, "Newer event"),
		]);
	});

	it("ignores a late outcome after same-show server authority replacement", async () => {
		const pending = deferred<CueRecordingOutcome>();
		const record = vi.fn(
			async (
				_showId: string,
				_revision: number,
				_request: CueRecordingRequest,
			) => pending.promise,
		);
		const { store, writer, loadObject, onError } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][2];

		installReplacementAuthority(store);
		pending.resolve(outcome(request.requestId));

		expect(await writing).toBeNull();
		expect(loadObject).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().cueLists).toEqual([
			cueList(20, "Replacement"),
		]);
	});

	it("ignores a late conflict after same-show server authority replacement", async () => {
		const pending = deferred<CueRecordingOutcome>();
		const record = vi.fn(
			async (
				_showId: string,
				_revision: number,
				_request: CueRecordingRequest,
			) => pending.promise,
		);
		const { store, writer, loadObject, onError } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());

		installReplacementAuthority(store);
		pending.reject(conflictError(20));

		expect(await writing).toBeNull();
		expect(loadObject).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot()).toMatchObject({ showRevision: 20 });
		expect(store.getSnapshot().cueLists).toEqual([
			cueList(20, "Replacement"),
		]);
	});

	it("rejects a mismatched transport outcome without installing it", async () => {
		const { store, writer, onError } = setup(
			vi.fn(async () => outcome("foreign-request")),
		);
		const before = store.getSnapshot();

		expect(await writer.record(input())).toBeNull();
		expect(store.getSnapshot()).toBe(before);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Cue recording response request ID does not match",
			}),
		);
	});
});

function conflictError(currentRevision: number) {
	return new CueRecordingActionError(
		"revision conflict",
		"conflict",
		409,
		currentRevision,
		false,
	);
}

function showChange(
	eventSequence: number,
	cueListObject: ShowObject<"cue_list">,
	playbackObject: ShowObject<"playback"> | null,
	pageObject: ShowObject<"playback_page"> | null,
	showRevision = 8,
): ShowObjectsChange {
	return {
		showId: SHOW_ID,
		showRevision,
		eventSequence,
		changes: [
			{
				kind: "cue_list",
				objectId: cueListObject.id,
				objectRevision: cueListObject.revision,
				body: cueListObject.body,
				deleted: false,
			},
			...(playbackObject
				? [
						{
							kind: "playback" as const,
							objectId: playbackObject.id,
							objectRevision: playbackObject.revision,
							body: playbackObject.body,
							deleted: false,
						},
					]
				: []),
			...(pageObject
				? [
						{
							kind: "playback_page" as const,
							objectId: pageObject.id,
							objectRevision: pageObject.revision,
							body: pageObject.body,
							deleted: false,
						},
					]
				: []),
		],
	};
}

function installReplacementAuthority(store: ShowObjectsStore) {
	store.reset(SHOW_ID, "user-a:session-b");
	store.setCollection(SHOW_ID, "cue_list", [cueList(20, "Replacement")]);
	store.setCollection(SHOW_ID, "playback", [playback(20)]);
	store.setCollection(SHOW_ID, "playback_page", [page(20)]);
	store.installShowRevision(SHOW_ID, 20);
}
