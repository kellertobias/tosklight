import { describe, expect, it, vi } from "vitest";
import type {
	SelectionActionOutcome,
	SelectionActionRequest,
	SelectionProjection,
} from "./contracts";
import { ProgrammingSelectionWriter } from "./selectionWriter";
import { ProgrammingInteractionStore } from "./store";
import {
	DESK_ID,
	FIXTURE_1,
	FIXTURE_2,
	FIXTURE_3,
	programmingSnapshot,
	selection,
	SHOW_ID,
} from "./testFixtures";

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

function readyStore(selected = selection()) {
	const store = new ProgrammingInteractionStore();
	store.reset(SHOW_ID, DESK_ID);
	store.installSnapshot(programmingSnapshot({ selected }));
	return store;
}

function outcome(
	request: SelectionActionRequest,
	selected: SelectionProjection,
	warning: string | null = null,
): SelectionActionOutcome {
	return {
		requestId: request.requestId,
		correlationId: request.requestId,
		action: "replaced",
		applied: selected.selected.length,
		selection: selected,
		eventSequence: selected.revision + 10,
		replayed: false,
		warning,
	};
}

function requestAt(
	apply: ReturnType<typeof vi.fn>,
	index: number,
): SelectionActionRequest {
	return apply.mock.calls[index]?.[1] as SelectionActionRequest;
}

describe("ProgrammingSelectionWriter", () => {
	it("applies the selection optimistically before the request completes", async () => {
		const store = readyStore();
		const response = deferred<SelectionActionOutcome>();
		const apply = vi.fn().mockReturnValue(response.promise);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
		});

		const write = writer.replace({ resolvedFixtures: [FIXTURE_2] });

		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);
		expect(store.getSnapshot().pendingCapabilities).toEqual(
			new Set(["selection"]),
		);
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		response.resolve(outcome(requestAt(apply, 0), selection(2, [FIXTURE_2])));

		await expect(write).resolves.toMatchObject({ action: "replaced" });
		expect(store.getSnapshot().selection).toEqual(
			selection(2, [FIXTURE_2]),
		);
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());
	});

	it("preserves strict FIFO and does not coalesce same-member semantics", async () => {
		const store = readyStore();
		const firstResponse = deferred<SelectionActionOutcome>();
		const secondResponse = deferred<SelectionActionOutcome>();
		const apply = vi
			.fn()
			.mockReturnValueOnce(firstResponse.promise)
			.mockReturnValueOnce(secondResponse.promise);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
		});

		const first = writer.replace({ resolvedFixtures: [FIXTURE_1] });
		const second = writer.replace({ resolvedFixtures: [FIXTURE_1] });
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
		expect(requestAt(apply, 0).action).toMatchObject({
			type: "replace",
			expectedRevision: 1,
		});

		firstResponse.resolve(
			outcome(requestAt(apply, 0), selection(2, [FIXTURE_1])),
		);
		await expect(first).resolves.not.toBeNull();
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
		expect(requestAt(apply, 1).action).toMatchObject({
			type: "replace",
			expectedRevision: 2,
		});
		expect(requestAt(apply, 1).requestId).not.toBe(
			requestAt(apply, 0).requestId,
		);

		secondResponse.resolve(
			outcome(requestAt(apply, 1), selection(3, [FIXTURE_1])),
		);
		await expect(second).resolves.not.toBeNull();
		expect(store.authoritativeSelectionRevision()).toBe(3);
	});

	it("orders pre-execution selection and defers later selection", async () => {
		const store = readyStore();
		const firstResponse = deferred<SelectionActionOutcome>();
		const secondResponse = deferred<SelectionActionOutcome>();
		const execution = deferred<"executed">();
		const apply = vi
			.fn()
			.mockReturnValueOnce(firstResponse.promise)
			.mockReturnValueOnce(secondResponse.promise);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
		});
		const run = vi.fn(() => execution.promise);

		const before = writer.replace({ resolvedFixtures: [FIXTURE_2] });
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const barrier = writer.runAfterPendingWrites(run, "write_failed" as const);
		const after = writer.replace({ resolvedFixtures: [FIXTURE_3] });
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_3]);
		expect(run).not.toHaveBeenCalled();

		firstResponse.resolve(
			outcome(requestAt(apply, 0), selection(2, [FIXTURE_2])),
		);
		await expect(before).resolves.not.toBeNull();
		await vi.waitFor(() => expect(run).toHaveBeenCalledOnce());
		expect(apply).toHaveBeenCalledOnce();

		execution.resolve("executed");
		await expect(barrier).resolves.toBe("executed");
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
		expect(requestAt(apply, 1).action).toMatchObject({
			type: "replace",
			expectedRevision: 2,
		});
		secondResponse.resolve(
			outcome(requestAt(apply, 1), selection(3, [FIXTURE_3])),
		);
		await expect(after).resolves.not.toBeNull();
	});

	it("reconciles when the authoritative event arrives before the response", async () => {
		const store = readyStore();
		const response = deferred<SelectionActionOutcome>();
		const apply = vi.fn().mockReturnValue(response.promise);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
		});
		const write = writer.replace({ resolvedFixtures: [FIXTURE_2] });
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const authority = selection(2, [FIXTURE_2]);
		store.applyChange({ deskId: DESK_ID, selection: authority }, 27);

		response.resolve(outcome(requestAt(apply, 0), authority));
		await expect(write).resolves.not.toBeNull();

		expect(store.getSnapshot().selection).toEqual(authority);
		expect(store.getSnapshot().eventSequence).toBe(27);
	});

	it("keeps a newer OSC event when an older response arrives", async () => {
		const store = readyStore();
		const response = deferred<SelectionActionOutcome>();
		const apply = vi.fn().mockReturnValue(response.promise);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
		});
		const write = writer.replace({ resolvedFixtures: [FIXTURE_2] });
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const newerOscAuthority = selection(3, [FIXTURE_3]);
		store.applyChange(
			{ deskId: DESK_ID, selection: newerOscAuthority },
			28,
		);
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);

		response.resolve(
			outcome(requestAt(apply, 0), selection(2, [FIXTURE_2])),
		);
		await expect(write).resolves.not.toBeNull();

		expect(store.getSnapshot().selection).toEqual(newerOscAuthority);
		expect(store.authoritativeSelectionRevision()).toBe(3);
	});

	it("repairs a 409 conflict without retrying the HTTP request", async () => {
		const store = readyStore();
		const conflict = Object.assign(new Error("selection revision conflict"), {
			status: 409,
		});
		const apply = vi.fn().mockRejectedValue(conflict);
		const repaired = selection(4, [FIXTURE_3]);
		const loadSnapshot = vi.fn().mockResolvedValue(
			programmingSnapshot({ sequence: 31, selected: repaired }),
		);
		const onError = vi.fn();
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot,
			onError,
		});

		await expect(
			writer.replace({ resolvedFixtures: [FIXTURE_2] }),
		).resolves.toBeNull();

		expect(apply).toHaveBeenCalledOnce();
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(store.getSnapshot().selection).toEqual(repaired);
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("reports a failed repair once and leaves the store usable", async () => {
		const store = readyStore();
		const conflict = Object.assign(new Error("selection revision conflict"), {
			status: 409,
		});
		const apply = vi.fn().mockRejectedValue(conflict);
		const onError = vi.fn();
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn().mockRejectedValue(new Error("snapshot unavailable")),
			onError,
		});

		await expect(
			writer.replace({ resolvedFixtures: [FIXTURE_2] }),
		).resolves.toBeNull();

		expect(onError).toHaveBeenCalledOnce();
		expect(onError.mock.calls[0]?.[0]).toEqual(
			new Error("Selection repair failed: snapshot unavailable"),
		);
		expect(store.getSnapshot().selection).toEqual(selection());
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());
	});

	it("retries one network failure with the same request ID", async () => {
		const store = readyStore();
		const apply = vi
			.fn()
			.mockRejectedValueOnce(new Error("connection reset"))
			.mockImplementationOnce(
				(_deskId: string, request: SelectionActionRequest) =>
					Promise.resolve(outcome(request, selection(2, [FIXTURE_2]))),
			);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
		});

		await expect(
			writer.replace({ resolvedFixtures: [FIXTURE_2] }),
		).resolves.not.toBeNull();

		expect(apply).toHaveBeenCalledTimes(2);
		expect(requestAt(apply, 1)).toBe(requestAt(apply, 0));
		expect(requestAt(apply, 1).requestId).toBe(
			requestAt(apply, 0).requestId,
		);
	});

	it("invalidates an active write and ignores its late completion after stop", async () => {
		const store = readyStore();
		const response = deferred<SelectionActionOutcome>();
		const apply = vi.fn().mockReturnValue(response.promise);
		const onError = vi.fn();
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
			onError,
		});
		const write = writer.replace({ resolvedFixtures: [FIXTURE_2] });
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);

		writer.stop();
		await expect(write).resolves.toBeNull();
		expect(store.getSnapshot().selection).toEqual(selection());
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());

		response.resolve(outcome(requestAt(apply, 0), selection(2, [FIXTURE_2])));
		await Promise.resolve();
		await Promise.resolve();
		expect(store.getSnapshot().selection).toEqual(selection());
		expect(onError).not.toHaveBeenCalled();
	});

	it("commits authoritative selection while surfacing a persistence warning", async () => {
		const store = readyStore();
		const onError = vi.fn();
		const apply = vi.fn(
			(_deskId: string, request: SelectionActionRequest) =>
				Promise.resolve(
					outcome(
						request,
						selection(2, [FIXTURE_2]),
						"selection applied but persistence failed",
					),
				),
		);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
			onError,
		});

		await expect(
			writer.replace({ resolvedFixtures: [FIXTURE_2] }),
		).resolves.not.toBeNull();

		expect(store.getSnapshot().selection).toEqual(
			selection(2, [FIXTURE_2]),
		);
		expect(store.authoritativeSelectionRevision()).toBe(2);
		expect(onError).toHaveBeenCalledOnce();
		expect(onError.mock.calls[0]?.[0]).toEqual(
			new Error("selection applied but persistence failed"),
		);
	});

	it("rejects an invalid optimistic rule without poisoning later writes", async () => {
		const store = readyStore();
		const onError = vi.fn();
		const apply = vi.fn(
			(_deskId: string, request: SelectionActionRequest) =>
				Promise.resolve(outcome(request, selection(2, [FIXTURE_2]))),
		);
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply,
			loadSnapshot: vi.fn(),
			onError,
		});

		await expect(
			writer.applyRule({ type: "every_nth", n: 0, offset: 0 }),
		).resolves.toBeNull();

		expect(apply).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledOnce();
		expect(store.getSnapshot().selection).toEqual(selection());
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());

		await expect(
			writer.replace({ resolvedFixtures: [FIXTURE_2] }),
		).resolves.not.toBeNull();
		expect(store.getSnapshot().selection).toEqual(
			selection(2, [FIXTURE_2]),
		);
	});

	it("rolls back a definitive client rejection without reloading authority", async () => {
		const store = readyStore();
		const rejection = Object.assign(new Error("fixture does not exist"), {
			status: 404,
		});
		const loadSnapshot = vi.fn();
		const onError = vi.fn();
		const writer = new ProgrammingSelectionWriter({
			deskId: DESK_ID,
			store,
			apply: vi.fn().mockRejectedValue(rejection),
			loadSnapshot,
			onError,
		});

		await expect(
			writer.replace({ resolvedFixtures: [FIXTURE_2] }),
		).resolves.toBeNull();

		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(store.getSnapshot().selection).toEqual(selection());
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());
		expect(onError).toHaveBeenLastCalledWith(rejection);
	});
});
