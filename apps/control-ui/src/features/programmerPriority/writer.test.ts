import { describe, expect, it, vi } from "vitest";
import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
} from "./contracts";
import { ProgrammerPriorityStore } from "./store";
import {
	CORRELATION_ID,
	deferred,
	noChangeOutcome,
	priorityProjection,
	prioritySnapshot,
	USER_ID,
} from "./testFixtures";
import type { ProgrammerPriorityTransport } from "./transport";
import { ProgrammerPriorityTransportError } from "./transport";
import { ProgrammerPriorityWriter } from "./writer";

function harness() {
	const store = new ProgrammerPriorityStore();
	store.reset(USER_ID, "session-a");
	store.installSnapshot(prioritySnapshot());
	const applyAction = vi.fn<ProgrammerPriorityTransport["applyAction"]>();
	const repair = vi.fn(async (_error: Error) => undefined);
	const onError = vi.fn();
	const transport: ProgrammerPriorityTransport = {
		loadSnapshot: vi.fn(),
		applyAction,
		subscribe: vi.fn(),
	};
	const writer = new ProgrammerPriorityWriter({
		scope: { userId: USER_ID },
		store,
		transport,
		repair,
		onError,
	});
	return { store, applyAction, repair, onError, writer };
}

function changed(
	request: ProgrammerPriorityActionRequest,
	eventSequence = 11,
): ProgrammerPriorityActionOutcome {
	return {
		requestId: request.requestId,
		correlationId: CORRELATION_ID,
		status: "changed",
		projection: priorityProjection({
			revision: request.expectedRevision + 1,
			priority: request.priority,
		}),
		eventSequence,
		replayed: false,
		warning: null,
	};
}

describe("ProgrammerPriorityWriter", () => {
	it("optimistically publishes and accepts the HTTP response before its event", async () => {
		const { store, applyAction, writer } = harness();
		applyAction.mockImplementationOnce(async (_scope, request) =>
			changed(request),
		);

		const pending = writer.setPriority({ priority: 20, requestId: "response" });
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: ["response"],
			projection: { priority: 20 },
		});
		await expect(pending).resolves.toMatchObject({ status: "changed" });
		expect(applyAction.mock.calls[0]?.[1]).toEqual({
			requestId: "response",
			expectedRevision: 1,
			priority: 20,
		});
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			authorityRevision: 2,
			projection: { priority: 20 },
		});
	});

	it("accepts the exact event before the HTTP response", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerPriorityActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setPriority({ priority: 30, requestId: "event" });
		await Promise.resolve();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing request");
		const outcome = changed(request);

		store.applyChange(
			{ type: "upsert", projection: outcome.projection },
			outcome.eventSequence ?? 11,
		);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["event"]);
		response.resolve(outcome);
		await expect(pending).resolves.toEqual(outcome);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("retries one ambiguous request with the identical body", async () => {
		const { applyAction, writer } = harness();
		applyAction
			.mockRejectedValueOnce(
				new ProgrammerPriorityTransportError(
					"connection reset",
					"unavailable",
					0,
					null,
					true,
				),
			)
			.mockImplementationOnce(async (_scope, request) =>
				noChangeOutcome(request.requestId, priorityProjection(), true),
			);

		await expect(
			writer.setPriority({ priority: 0, requestId: "replay" }),
		).resolves.toMatchObject({ status: "no_change", replayed: true });
		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(applyAction.mock.calls[1]?.[1]).toBe(applyAction.mock.calls[0]?.[1]);
	});

	it("serializes writes and advances the revision only once per outcome", async () => {
		const { applyAction, writer } = harness();
		const firstResponse = deferred<ProgrammerPriorityActionOutcome>();
		applyAction
			.mockReturnValueOnce(firstResponse.promise)
			.mockImplementationOnce(async (_scope, request) => changed(request, 12));
		const first = writer.setPriority({ priority: 10, requestId: "first" });
		const second = writer.setPriority({ priority: 20, requestId: "second" });
		await Promise.resolve();
		expect(applyAction).toHaveBeenCalledOnce();
		const firstRequest = applyAction.mock.calls[0]?.[1];
		if (!firstRequest) throw new Error("missing first request");
		firstResponse.resolve(changed(firstRequest));
		await Promise.all([first, second]);

		expect(applyAction.mock.calls.map(([, request]) => request)).toEqual([
			{ requestId: "first", expectedRevision: 1, priority: 10 },
			{ requestId: "second", expectedRevision: 2, priority: 20 },
		]);
	});

	it("rolls back a definitive rejection", async () => {
		const { store, applyAction, onError, writer } = harness();
		applyAction.mockRejectedValueOnce(
			new ProgrammerPriorityTransportError(
				"invalid priority",
				"invalid",
				400,
				null,
				false,
			),
		);

		const pending = writer.setPriority({ priority: 12, requestId: "invalid" });
		expect(store.getSnapshot().projection?.priority).toBe(12);
		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { priority: 0 },
		});
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "invalid priority" }),
		);
	});

	it("repairs a revision conflict narrowly before rolling back", async () => {
		const { store, applyAction, repair, writer } = harness();
		applyAction.mockRejectedValueOnce(
			new ProgrammerPriorityTransportError(
				"revision conflict",
				"conflict",
				409,
				2,
				false,
			),
		);
		repair.mockImplementationOnce(async () => {
			store.installRepairSnapshot(
				prioritySnapshot({ cursor: 20, revision: 2, priority: 4 }),
			);
		});

		await expect(
			writer.setPriority({ priority: 12, requestId: "conflict" }),
		).resolves.toBeNull();
		expect(repair).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			authorityRevision: 2,
			pendingRequestIds: [],
			projection: { priority: 4 },
		});
	});

	it("drops a late response after a tombstone or authority replacement", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<ProgrammerPriorityActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setPriority({ priority: 15, requestId: "removed" });
		await Promise.resolve();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing request");
		store.applyChange({ type: "remove", userId: USER_ID, revision: 2 }, 11);
		response.resolve(changed(request));

		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot().projection).toBeNull();

		const replacement = deferred<ProgrammerPriorityActionOutcome>();
		store.installRepairSnapshot(
			prioritySnapshot({ cursor: 12, revision: 2, priority: 2 }),
		);
		applyAction.mockReturnValueOnce(replacement.promise);
		const late = writer.setPriority({ priority: 9, requestId: "late" });
		await Promise.resolve();
		const lateRequest = applyAction.mock.calls[1]?.[1];
		if (!lateRequest) throw new Error("missing late request");
		store.reset(USER_ID, "session-b");
		replacement.resolve(changed(lateRequest, 13));
		await expect(late).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			authorityKey: "session-b",
			projection: null,
		});
	});

	it("cancels tombstoned queued work and accepts an explicit retry after recreation", async () => {
		const { store, applyAction, writer } = harness();
		const firstResponse = deferred<ProgrammerPriorityActionOutcome>();
		applyAction.mockReturnValueOnce(firstResponse.promise);
		const first = writer.setPriority({ priority: 10, requestId: "first" });
		const cancelled = writer.setPriority({ priority: 20, requestId: "queued" });
		await Promise.resolve();
		const firstRequest = applyAction.mock.calls[0]?.[1];
		if (!firstRequest) throw new Error("missing first request");
		store.applyChange({ type: "remove", userId: USER_ID, revision: 2 }, 11);
		firstResponse.resolve(changed(firstRequest));

		await expect(first).resolves.toBeNull();
		await expect(cancelled).resolves.toBeNull();
		expect(applyAction).toHaveBeenCalledOnce();

		store.applyChange(
			{
				type: "upsert",
				projection: priorityProjection({ revision: 2, priority: 3 }),
			},
			12,
		);
		applyAction.mockImplementationOnce(async (_scope, request) =>
			changed(request, 13),
		);
		await expect(
			writer.setPriority({ priority: 20, requestId: "retry" }),
		).resolves.toMatchObject({ status: "changed" });
		expect(applyAction.mock.calls[1]?.[1]).toMatchObject({
			requestId: "retry",
			expectedRevision: 2,
			priority: 20,
		});
	});
});
