import { describe, expect, it, vi } from "vitest";
import type {
	SpeedGroupActionOutcome,
	SpeedGroupActionRequest,
} from "./contracts";
import { SpeedGroupRuntimeStore } from "./store";
import {
	changedOutcome,
	DESK_ID,
	deferred,
	noChangeOutcome,
	speedGroup,
	speedSnapshot,
} from "./testFixtures";
import type { SpeedGroupRuntimeTransport } from "./transport";
import { SpeedGroupTransportError } from "./transport";
import { SpeedGroupRuntimeWriter } from "./writer";

function harness() {
	const store = new SpeedGroupRuntimeStore();
	store.reset(DESK_ID, "session-a");
	store.installSnapshot(speedSnapshot());
	const applyAction = vi.fn<SpeedGroupRuntimeTransport["applyAction"]>();
	const repair = vi.fn(async (_error: Error) => undefined);
	const onError = vi.fn();
	const transport: SpeedGroupRuntimeTransport = {
		loadSnapshot: vi.fn(),
		applyAction,
		subscribe: vi.fn(),
	};
	const writer = new SpeedGroupRuntimeWriter({
		scope: { deskId: DESK_ID },
		store,
		transport,
		repair,
		onError,
	});
	return { store, applyAction, repair, onError, writer };
}

function changed(request: SpeedGroupActionRequest, eventSequence = 11) {
	const action = request.action;
	const groups =
		action.type === "synchronize"
			? [
					speedGroup(action.source, {
						synchronizedWith: action.target,
					}),
					speedGroup(action.target, {
						manualBpm: 120,
						synchronizedWith: action.source,
					}),
				].sort((left, right) => left.group.localeCompare(right.group))
			: [
					speedGroup(action.group, {
						manualBpm:
							action.type === "set_bpm" ? action.bpm : 120 + action.deltaBpm,
						phaseOriginMillis: 200,
					}),
				];
	return changedOutcome(request.requestId, groups, {
		revision: request.expectedRevision + 1,
		eventSequence,
	});
}

describe("SpeedGroupRuntimeWriter", () => {
	it("optimistically sends typed set, adjust, and synchronize actions in FIFO order", async () => {
		const { store, applyAction, writer } = harness();
		const firstResponse = deferred<SpeedGroupActionOutcome>();
		applyAction
			.mockReturnValueOnce(firstResponse.promise)
			.mockImplementation(async (_scope, request) => changed(request, 12));

		const first = writer.setBpm("A", 128, "set");
		const second = writer.adjustBpm("B", 5, "adjust");
		const third = writer.synchronize("A", "C", "sync");
		expect(store.getSnapshot().pendingRequestIds).toEqual([
			"set",
			"adjust",
			"sync",
		]);
		expect(store.getSnapshot().projection?.groups[2]).toMatchObject({
			manualBpm: 128,
			synchronizedWith: "A",
		});
		await Promise.resolve();
		expect(applyAction).toHaveBeenCalledOnce();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing first request");
		firstResponse.resolve(changed(request));
		await Promise.all([first, second, third]);

		expect(applyAction.mock.calls.map(([, sent]) => sent)).toMatchObject([
			{
				requestId: "set",
				expectedRevision: 1,
				action: { type: "set_bpm", group: "A", bpm: 128 },
			},
			{
				requestId: "adjust",
				expectedRevision: 2,
				action: { type: "adjust_bpm", group: "B", deltaBpm: 5 },
			},
			{
				requestId: "sync",
				expectedRevision: 3,
				action: { type: "synchronize", source: "A", target: "C" },
			},
		]);
	});

	it("accepts the exact event before its HTTP response", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<SpeedGroupActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setBpm("A", 128, "event-first");
		await Promise.resolve();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing request");
		const outcome = changed(request);
		if (outcome.status !== "changed")
			throw new Error("missing changed outcome");
		store.applyChange(
			{
				authorityId: outcome.authorityId,
				revision: outcome.revision,
				appliedAtMillis: outcome.appliedAtMillis,
				groups: outcome.groups,
			},
			outcome.eventSequence,
		);
		response.resolve(outcome);
		await expect(pending).resolves.toEqual(outcome);
		expect(store.getSnapshot().pendingRequestIds).toEqual([]);
	});

	it("retries an ambiguous request with the identical request object", async () => {
		const { applyAction, writer } = harness();
		applyAction
			.mockRejectedValueOnce(
				new SpeedGroupTransportError(
					"connection reset",
					"unavailable",
					0,
					null,
					true,
				),
			)
			.mockImplementationOnce(async (_scope, request) =>
				noChangeOutcome(request.requestId, [speedGroup("A")], true),
			);

		await expect(writer.setBpm("A", 120, "replay")).resolves.toMatchObject({
			status: "no_change",
			replayed: true,
		});
		expect(applyAction).toHaveBeenCalledTimes(2);
		expect(applyAction.mock.calls[1]?.[1]).toBe(applyAction.mock.calls[0]?.[1]);
	});

	it("rolls back definitive failure and repairs a revision conflict", async () => {
		const { store, applyAction, repair, writer } = harness();
		applyAction.mockRejectedValueOnce(
			new SpeedGroupTransportError("invalid", "invalid", 400, null, false),
		);
		const invalid = writer.setBpm("A", 130, "invalid");
		expect(store.getSnapshot().projection?.groups[0]?.manualBpm).toBe(130);
		await expect(invalid).resolves.toBeNull();
		expect(store.getSnapshot().projection?.groups[0]?.manualBpm).toBe(120);

		applyAction.mockRejectedValueOnce(
			new SpeedGroupTransportError(
				"revision conflict",
				"conflict",
				409,
				2,
				false,
			),
		);
		repair.mockImplementationOnce(async () => {
			store.installRepairSnapshot(speedSnapshot({ cursor: 20, revision: 2 }));
		});
		await expect(writer.setBpm("B", 95, "conflict")).resolves.toBeNull();
		expect(repair).toHaveBeenCalledOnce();
		expect(store.getSnapshot()).toMatchObject({
			authorityRevision: 2,
			pendingRequestIds: [],
		});
	});

	it("drops a late response after desk/session replacement", async () => {
		const { store, applyAction, writer } = harness();
		const response = deferred<SpeedGroupActionOutcome>();
		applyAction.mockReturnValueOnce(response.promise);
		const pending = writer.setBpm("A", 130, "late");
		await Promise.resolve();
		const request = applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing request");
		store.reset(DESK_ID, "session-b");
		response.resolve(changed(request));
		await expect(pending).resolves.toBeNull();
		expect(store.getSnapshot()).toMatchObject({
			authorityKey: "session-b",
			projection: null,
		});
	});

	it("refuses invalid groups, BPM, delta, and same-group synchronization", async () => {
		const { applyAction, writer } = harness();
		await expect(writer.setBpm("A", 0, "zero")).resolves.toBeNull();
		await expect(writer.adjustBpm("A", 0, "zero-delta")).resolves.toBeNull();
		await expect(writer.synchronize("A", "A", "same")).resolves.toBeNull();
		expect(applyAction).not.toHaveBeenCalled();
	});
});
