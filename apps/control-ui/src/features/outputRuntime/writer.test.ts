import { describe, expect, it, vi } from "vitest";
import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
} from "./contracts";
import { OutputRuntimeStore } from "./store";
import {
	changedOutcome,
	deferred,
	DESK_ID,
	noChangeOutcome,
	OTHER_SHOW_ID,
	outputProjection,
	outputSnapshot,
	SHOW_ID,
} from "./testFixtures";
import type { OutputRuntimeTransport } from "./transport";
import { OutputRuntimeTransportError } from "./transport";
import { OutputRuntimeWriter } from "./writer";

function harness() {
	const store = new OutputRuntimeStore();
	store.reset(SHOW_ID, DESK_ID, "session-a");
	store.installSnapshot(outputSnapshot());
	const applyAction = vi.fn<OutputRuntimeTransport["applyAction"]>();
	const repair = vi.fn(async (_error: Error) => undefined);
	const onError = vi.fn();
	const transport: OutputRuntimeTransport = {
		loadSnapshot: vi.fn(),
		applyAction,
		subscribe: vi.fn(),
	};
	const writer = new OutputRuntimeWriter({
		scope: { showId: SHOW_ID, deskId: DESK_ID },
		store,
		transport,
		repair,
		onError,
	});
	return { store, applyAction, repair, onError, writer };
}

function changed(
	request: OutputRuntimeActionRequest,
	eventSequence = 11,
): OutputRuntimeActionOutcome {
	return changedOutcome(
		request.requestId,
		outputProjection({
			revision: request.expectedRevision + 1,
			grandMaster: request.grandMaster ?? 1,
			blackout: request.blackout ?? false,
		}),
		eventSequence,
	);
}

describe("OutputRuntimeWriter", () => {
	it("sends one atomic optimistic Grand Master and blackout action", async () => {
		const { store, applyAction, writer } = harness();
		applyAction.mockImplementationOnce(async (_scope, request) =>
			changed(request),
		);

		const pending = writer.setOutput({
			grandMaster: 0.4,
			blackout: true,
			requestId: "combined",
		});
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: ["combined"],
			projection: { grandMaster: 0.4, blackout: true },
		});
		await expect(pending).resolves.toMatchObject({ status: "changed" });
		expect(applyAction).toHaveBeenCalledOnce();
		expect(applyAction.mock.calls[0]?.[1]).toEqual({
			requestId: "combined",
			expectedShowId: SHOW_ID,
			expectedRevision: 1,
			grandMaster: 0.4,
			blackout: true,
		});
	});

	it("accepts the exact event before its HTTP response", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<OutputRuntimeActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setOutput({
			grandMaster: 0.3,
			blackout: true,
			requestId: "event-first",
		});
		await Promise.resolve();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing request");
		const outcome = changed(request);

		store.applyChange(
			{ projection: outcome.projection },
			outcome.eventSequence ?? 11,
		);
		expect(store.getSnapshot().pendingRequestIds).toEqual(["event-first"]);
		response.resolve(outcome);
		await expect(pending).resolves.toEqual(outcome);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("retries an ambiguous request with the identical body", async () => {
		const { applyAction, writer } = harness();
		applyAction
			.mockRejectedValueOnce(
				new OutputRuntimeTransportError(
					"connection reset",
					"unavailable",
					0,
					null,
					true,
				),
			)
			.mockImplementationOnce(async (_scope, request) =>
				noChangeOutcome(request.requestId, outputProjection(), true),
			);

		await expect(
			writer.setOutput({ grandMaster: 1, requestId: "replay" }),
		).resolves.toMatchObject({ status: "no_change", replayed: true });
		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(applyAction.mock.calls[1]?.[1]).toBe(applyAction.mock.calls[0]?.[1]);
	});

	it("serializes writes and advances independent revision authority", async () => {
		const { applyAction, writer } = harness();
		const firstResponse = deferred<OutputRuntimeActionOutcome>();
		applyAction
			.mockReturnValueOnce(firstResponse.promise)
			.mockImplementationOnce(async (_scope, request) =>
				changedOutcome(
					request.requestId,
					outputProjection({
						revision: request.expectedRevision + 1,
						grandMaster: 0.5,
						blackout: true,
					}),
					12,
				),
			);
		const first = writer.setOutput({ grandMaster: 0.5, requestId: "first" });
		const second = writer.setOutput({ blackout: true, requestId: "second" });
		await Promise.resolve();
		expect(applyAction).toHaveBeenCalledOnce();
		const firstRequest = applyAction.mock.calls[0]?.[1];
		if (!firstRequest) throw new Error("missing first request");
		firstResponse.resolve(changed(firstRequest));
		await Promise.all([first, second]);

		expect(applyAction.mock.calls.map(([, request]) => request)).toEqual([
			{
				requestId: "first",
				expectedShowId: SHOW_ID,
				expectedRevision: 1,
				grandMaster: 0.5,
				blackout: undefined,
			},
			{
				requestId: "second",
				expectedShowId: SHOW_ID,
				expectedRevision: 2,
				grandMaster: undefined,
				blackout: true,
			},
		]);
	});

	it("rolls back a definitive rejection", async () => {
		const { store, applyAction, onError, writer } = harness();
		applyAction.mockRejectedValueOnce(
			new OutputRuntimeTransportError(
				"invalid output",
				"invalid",
				400,
				null,
				false,
			),
		);

		const pending = writer.setOutput({
			blackout: true,
			requestId: "invalid",
		});
		expect(store.getSnapshot().projection?.blackout).toBe(true);
		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			pendingRequestIds: [],
			projection: { grandMaster: 1, blackout: false },
		});
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "invalid output" }),
		);
	});

	it("repairs the exact snapshot after a typed revision conflict", async () => {
		const { store, applyAction, repair, writer } = harness();
		applyAction.mockRejectedValueOnce(
			new OutputRuntimeTransportError(
				"revision conflict",
				"conflict",
				409,
				2,
				false,
			),
		);
		repair.mockImplementationOnce(async () => {
			store.installRepairSnapshot(
				outputSnapshot({
					cursor: 20,
					revision: 2,
					grandMaster: 0.6,
				}),
			);
		});

		await expect(
			writer.setOutput({ blackout: true, requestId: "conflict" }),
		).resolves.toBeNull();
		expect(repair).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			authorityRevision: 2,
			pendingRequestIds: [],
			projection: { grandMaster: 0.6, blackout: false },
		});
	});

	it("drops late responses after Show authority replacement", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<OutputRuntimeActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setOutput({ blackout: true, requestId: "late" });
		await Promise.resolve();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing request");

		store.reset(OTHER_SHOW_ID, DESK_ID, "session-b");
		response.resolve(changed(request));
		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			authorityKey: "session-b",
			projection: null,
		});
	});

	it("refuses malformed or empty mutations before transport", async () => {
		const { applyAction, writer } = harness();
		await expect(writer.setOutput({ requestId: "empty" })).resolves.toBeNull();
		await expect(
			writer.setOutput({ grandMaster: Number.NaN, requestId: "nan" }),
		).resolves.toBeNull();
		expect(applyAction).not.toHaveBeenCalled();
	});
});
