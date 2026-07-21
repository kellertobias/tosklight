import { describe, expect, it, vi } from "vitest";
import type { PlaybackActionRequest } from "../../api/types";
import { PlaybackRuntimeActionWriter } from "./actionWriter";
import type { PlaybackOutcome, PlaybackProjection } from "./contracts";
import { playbackIdentity } from "./contracts";
import { PlaybackRuntimeStore } from "./store";
import {
	cueProjection,
	DESK_ID,
	deskProjection,
	playbackSnapshot,
	SHOW_ID,
} from "./testFixtures";

function outcome(
	request: PlaybackActionRequest,
	projection = cueProjection(),
	eventSequence: number | null = 12,
): PlaybackOutcome {
	return {
		request_id: request.request_id,
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: request.address,
		resolved: {
			kind: "playback",
			playback_number: projection.playback_number ?? 1,
			page: 1,
			slot: 1,
		},
		outcome: { status: eventSequence == null ? "no_change" : "applied" },
		durability: "durable",
		projection,
		related: [],
		desk: null,
		event_sequence: eventSequence,
		desk_event_sequence: null,
		replayed: false,
	};
}

function masterProjection(value: number): PlaybackProjection {
	const projection = cueProjection();
	if (projection.target !== "cue_list" || !projection.runtime)
		throw new Error("fixture must contain a running Cuelist");
	return {
		...projection,
		runtime: { ...projection.runtime, master: value, fader_position: value },
	};
}

function runtime(store: PlaybackRuntimeStore) {
	const projection = store.getSnapshot().projections.get("playback:1")?.[0];
	if (projection?.target !== "cue_list" || !projection.runtime)
		throw new Error("expected a running Cuelist projection");
	return projection.runtime;
}

function readyStore() {
	const store = new PlaybackRuntimeStore();
	const identity = playbackIdentity(1);
	store.reset(SHOW_ID, DESK_ID, "authority-a");
	store.installSnapshot(playbackSnapshot([identity]), [identity]);
	return store;
}

function pageOutcome(page: number, eventSequence: number | null = 11) {
	return {
		desk_id: DESK_ID,
		page,
		event_sequence: eventSequence,
		page_creation_event_sequence: null,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

describe("PlaybackRuntimeActionWriter", () => {
	it("optimistically selects a page and deduplicates its later desk event", async () => {
		const store = readyStore();
		const pending = deferred<ReturnType<typeof pageOutcome>>();
		const applyDeskPage = vi.fn(() => pending.promise);
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage,
		});

		const result = writer.setActivePage(2);
		expect(store.getSnapshot().desk?.active_page).toBe(2);
		pending.resolve(pageOutcome(2));

		await expect(result).resolves.toBe(true);
		expect(applyDeskPage).toHaveBeenCalledWith(DESK_ID, 2);
		expect(store.applyDesk(deskProjection(2), 11)).toBe(false);
		expect(store.getSnapshot().desk?.active_page).toBe(2);
	});

	it("settles a page response after its authoritative desk event", async () => {
		const store = readyStore();
		const pending = deferred<ReturnType<typeof pageOutcome>>();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage: () => pending.promise,
		});

		const result = writer.setActivePage(3);
		expect(store.applyDesk(deskProjection(3), 12)).toBe(true);
		pending.resolve(pageOutcome(3, 12));

		await expect(result).resolves.toBe(true);
		expect(store.getSnapshot().desk?.active_page).toBe(3);
	});

	it("rolls back a failed page selection without disabling a retry", async () => {
		const store = readyStore();
		const applyDeskPage = vi
			.fn()
			.mockRejectedValueOnce(new Error("page rejected"))
			.mockResolvedValueOnce(pageOutcome(2, 12));
		const onError = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage,
			onError,
		});

		await expect(writer.setActivePage(4)).resolves.toBe(false);
		expect(store.getSnapshot().desk?.active_page).toBe(1);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error?.message).toBe("page rejected");
		expect(onError).toHaveBeenLastCalledWith(
			expect.objectContaining({ message: "page rejected" }),
		);

		await expect(writer.setActivePage(2)).resolves.toBe(true);
		expect(applyDeskPage).toHaveBeenCalledTimes(2);
		expect(store.getSnapshot().desk?.active_page).toBe(2);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error).toBeNull();
		expect(onError).toHaveBeenLastCalledWith(null);
	});

	it("rejects a mismatched page response and restores desk authority", async () => {
		const store = readyStore();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage: async () => pageOutcome(7),
		});

		await expect(writer.setActivePage(6)).resolves.toBe(false);
		expect(store.getSnapshot().desk?.active_page).toBe(1);
		expect(store.getSnapshot().error?.message).toBe(
			"Playback page response does not match the active desk request",
		);
	});

	it("rejects a compatibility response that silently created a Page", async () => {
		const store = readyStore();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage: async () => ({
				...pageOutcome(6),
				page_creation_event_sequence: 10,
			}),
		});

		await expect(writer.setActivePage(6)).resolves.toBe(false);
		expect(store.getSnapshot().desk?.active_page).toBe(1);
		expect(store.getSnapshot().error?.message).toBe(
			"Playback page selection unexpectedly created a Page",
		);
	});

	it("rejects page numbers outside the desk contract before writing", async () => {
		const store = readyStore();
		const applyDeskPage = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage,
		});

		await expect(writer.setActivePage(128)).resolves.toBe(false);
		expect(applyDeskPage).not.toHaveBeenCalled();
		expect(store.getSnapshot().desk?.active_page).toBe(1);
		expect(store.getSnapshot().error?.message).toBe(
			"Playback page must be an integer between 1 and 127",
		);
	});

	it("refuses page writes until exact desk authority is hydrated", async () => {
		const store = new PlaybackRuntimeStore();
		store.reset(SHOW_ID, DESK_ID, "authority-a");
		const applyDeskPage = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage,
		});

		await expect(writer.setActivePage(2)).resolves.toBe(false);
		expect(applyDeskPage).not.toHaveBeenCalled();
		expect(store.getSnapshot().error?.message).toBe(
			"Authoritative Playback desk is loading",
		);
	});

	it("ignores a page response after authority replacement", async () => {
		const store = readyStore();
		const pending = deferred<ReturnType<typeof pageOutcome>>();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: vi.fn(),
			applyDeskPage: () => pending.promise,
		});

		const result = writer.setActivePage(5);
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		pending.resolve(pageOutcome(5));

		await expect(result).resolves.toBe(false);
		expect(store.getSnapshot().desk).toBeNull();
		expect(store.getSnapshot().error).toBeNull();
	});

	it("replays a retry with the exact request ID and virtual surface metadata", async () => {
		const store = readyStore();
		const requests: PlaybackActionRequest[] = [];
		const retryable = Object.assign(new Error("temporarily offline"), {
			retryable: true,
		});
		const applyAction = vi.fn(async (_showId, _deskId, request) => {
			requests.push(request);
			if (requests.length === 1) throw retryable;
			return { ...outcome(request, cueProjection(1, 1)), replayed: true };
		});
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction,
		});

		const result = await writer.poolPlaybackAction(1, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});

		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(requests[1]).toBe(requests[0]);
		expect(requests[1].request_id).toBe(requests[0].request_id);
		expect(requests[0]).toMatchObject({
			address: { kind: "playback", playback_number: 1 },
			action: { type: "configured_button", number: 1, pressed: true },
			surface: "virtual",
		});
		expect(result?.replayed).toBe(true);
		expect(runtime(store).cue_index).toBe(1);
	});

	it("rolls back an optimistic master after a terminal failure", async () => {
		const store = readyStore();
		const pending = deferred<PlaybackOutcome>();
		let attempts = 0;
		const onError = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: async (_showId, _deskId, request) => {
				attempts += 1;
				return attempts === 1
					? pending.promise
					: outcome(request, masterProjection(0.6), 14);
			},
			onError,
		});

		const result = writer.poolPlaybackAction(1, "master", {
			value: 0.35,
			surface: "virtual",
		});
		expect(runtime(store).master).toBe(0.35);
		pending.reject(Object.assign(new Error("conflict"), { status: 409 }));

		expect(await result).toBeNull();
		expect(runtime(store).master).toBe(1);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error?.message).toBe("conflict");
		expect(onError).toHaveBeenCalledWith(expect.any(Error));

		await expect(
			writer.poolPlaybackAction(1, "master", {
				value: 0.6,
				surface: "virtual",
			}),
		).resolves.not.toBeNull();
		expect(attempts).toBe(2);
		expect(runtime(store).master).toBe(0.6);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error).toBeNull();
		expect(onError).toHaveBeenLastCalledWith(null);
	});

	it("keeps GO authority ready after failure and clears the error on retry", async () => {
		const store = readyStore();
		const beforeCue = runtime(store).cue_index;
		let attempts = 0;
		const onError = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: async (_showId, _deskId, request) => {
				attempts += 1;
				if (attempts === 1) throw new Error("GO rejected");
				return outcome(request, cueProjection(1, 1), 14);
			},
			onError,
		});

		await expect(
			writer.poolPlaybackAction(1, "go", { surface: "virtual" }),
		).resolves.toBeNull();
		expect(runtime(store).cue_index).toBe(beforeCue);
		expect(store.getSnapshot().pendingKeys.size).toBe(0);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error?.message).toBe("GO rejected");
		expect(onError).toHaveBeenLastCalledWith(
			expect.objectContaining({ message: "GO rejected" }),
		);

		await expect(
			writer.poolPlaybackAction(1, "go", { surface: "virtual" }),
		).resolves.not.toBeNull();
		expect(attempts).toBe(2);
		expect(runtime(store).cue_index).toBe(1);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error).toBeNull();
		expect(onError).toHaveBeenLastCalledWith(null);
	});

	it("does not let a late GO outcome hide a session failure", async () => {
		const store = readyStore();
		const pending = deferred<PlaybackOutcome>();
		let request: PlaybackActionRequest | null = null;
		const onError = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: (_showId, _deskId, value) => {
				request = value;
				return pending.promise;
			},
			onError,
		});

		const result = writer.poolPlaybackAction(1, "go", { surface: "virtual" });
		const sessionError = new Error("Playback session failed");
		store.setError(sessionError);
		if (!request) throw new Error("request was not captured");
		pending.resolve(outcome(request, cueProjection(1, 2), 15));

		await expect(result).resolves.not.toBeNull();
		expect(runtime(store).cue_index).toBe(2);
		expect(store.getSnapshot().status).toBe("error");
		expect(store.getSnapshot().error).toBe(sessionError);
		expect(onError).not.toHaveBeenCalled();
	});

	it("deduplicates an event arriving after the authoritative response", async () => {
		const store = readyStore();
		const projection = cueProjection(1, 2);
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: async (_showId, _deskId, request) =>
				outcome(request, projection, 12),
		});

		await writer.poolPlaybackAction(1, "go", { surface: "virtual" });
		const afterResponse = store.getSnapshot();

		expect(store.applyProjection(projection, 12)).toBe(false);
		expect(store.getSnapshot()).toBe(afterResponse);
		expect(runtime(store).cue_index).toBe(2);
	});

	it("settles against an event arriving before the response", async () => {
		const store = readyStore();
		const pending = deferred<PlaybackOutcome>();
		let request: PlaybackActionRequest | null = null;
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: (_showId, _deskId, value) => {
				request = value;
				return pending.promise;
			},
		});
		const projection = cueProjection(1, 3);

		const result = writer.poolPlaybackAction(1, "go", { surface: "virtual" });
		store.applyProjection(projection, 13);
		if (!request) throw new Error("request was not captured");
		pending.resolve(outcome(request, projection, 13));

		expect(await result).not.toBeNull();
		expect(runtime(store).cue_index).toBe(3);
		expect(store.getSnapshot().eventSequence).toBe(13);
	});

	it("rejects a late response after same-scope authority replacement", async () => {
		const store = readyStore();
		const pending = deferred<PlaybackOutcome>();
		let request: PlaybackActionRequest | null = null;
		const onError = vi.fn();
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction: (_showId, _deskId, value) => {
				request = value;
				return pending.promise;
			},
			onError,
		});

		const result = writer.poolPlaybackAction(1, "go", { surface: "virtual" });
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		if (!request) throw new Error("request was not captured");
		pending.resolve(outcome(request, cueProjection(1, 4), 14));

		expect(await result).toBeNull();
		expect(store.getSnapshot().projections.size).toBe(0);
		expect(store.getSnapshot().error).toBeNull();
		expect(onError).not.toHaveBeenCalled();
	});

	it("sends a held-button safety release after its writer scope was replaced", async () => {
		const store = readyStore();
		const pressOutcome = deferred<PlaybackOutcome>();
		const requests: PlaybackActionRequest[] = [];
		let releaseAttempts = 0;
		const retryable = Object.assign(new Error("reconnecting"), {
			retryable: true,
		});
		const applyAction = vi.fn(async (_showId, _deskId, request) => {
			requests.push(request);
			if (request.action.type === "configured_button" && request.action.pressed)
				return pressOutcome.promise;
			releaseAttempts += 1;
			if (releaseAttempts === 1) throw retryable;
			return outcome(request, cueProjection(), null);
		});
		const writer = new PlaybackRuntimeActionWriter({
			showId: SHOW_ID,
			deskId: DESK_ID,
			store,
			applyAction,
		});
		const press = writer.poolPlaybackAction(1, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		await Promise.resolve();
		writer.stop();
		store.reset(SHOW_ID, DESK_ID, "authority-b");
		pressOutcome.resolve(outcome(requests[0], cueProjection(), null));
		await expect(press).resolves.toBeNull();

		await expect(
			writer.poolPlaybackAction(1, "button", {
				button: 1,
				pressed: false,
				surface: "virtual",
			}),
		).resolves.not.toBeNull();

		expect(applyAction).toHaveBeenCalledTimes(3);
		expect(requests[2]).toBe(requests[1]);
		expect(requests[1]).toMatchObject({
			action: { type: "configured_button", number: 1, pressed: false },
			surface: "virtual",
		});
		expect(store.getSnapshot().projections.size).toBe(0);
	});
});
