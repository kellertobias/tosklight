import { describe, expect, it, vi } from "vitest";
import { GroupRecordingActionError } from "../../api/GroupRecordingTransport";
import type { ShowObject } from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	GroupRecordingOutcome,
	GroupRecordingRequest,
	GroupRecordingTransport,
	RecordedGroupProjection,
} from "./contracts";
import { GroupRecordingWriter } from "./writer";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function group(
	revision: number,
	name: string,
	fixtures = ["fixture-1"],
	id = "front",
): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision,
		updated_at: "",
		body: { name, fixtures },
	};
}

function stored(value = group(2, "Recorded")): RecordedGroupProjection {
	return {
		state: "stored",
		id: value.id,
		revision: value.revision,
		object: value,
	};
}

function deleted(revision = 2): RecordedGroupProjection {
	return { state: "deleted", id: "front", revision, object: null };
}

function outcome(
	requestId: string,
	projection = stored(),
	options: { status?: "changed" | "no_change"; replayed?: boolean } = {},
): GroupRecordingOutcome {
	const base = {
		requestId,
		correlationId: CORRELATION_ID,
		replayed: options.replayed ?? false,
		showRevision: 8,
		group: projection,
	};
	if (options.status !== "no_change")
		return { ...base, status: "changed", eventSequence: 12 };
	if (projection.state !== "stored")
		throw new Error("Test no-change outcomes require a stored projection");
	return { ...base, group: projection, status: "no_change" };
}

function input(
	operation: "overwrite" | "merge" | "subtract" | "delete" = "overwrite",
) {
	return {
		objectId: "front",
		operation,
		expectedObjectRevision: 1,
	};
}

function setup(
	record: GroupRecordingTransport["record"],
	loadGroup = vi.fn(async () => null as ShowObject<"group"> | null),
) {
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "session-a");
	store.setCollection(SHOW_ID, "group", [group(1, "Original")], 10);
	const onError = vi.fn();
	const writer = new GroupRecordingWriter({
		showId: SHOW_ID,
		store,
		transport: { record },
		loadGroup,
		onError,
	});
	return { store, writer, loadGroup, onError };
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

describe("GroupRecordingWriter", () => {
	it("refuses recording before the Group collection is ready", async () => {
		const record = vi.fn();
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		const onError = vi.fn();
		const writer = new GroupRecordingWriter({
			showId: SHOW_ID,
			store,
			transport: { record },
			loadGroup: vi.fn(),
			onError,
		});

		expect(await writer.record(input())).toBeNull();
		expect(record).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Authoritative Group collection is still loading",
			}),
		);
	});

	it("installs a response before its one authoritative event", async () => {
		const record = vi.fn(
			async (_showId: string, request: GroupRecordingRequest) =>
				outcome(request.requestId),
		);
		const { store, writer } = setup(record);

		await writer.record(input("merge"));
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Recorded"));
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		store.applyChange(groupEvent(12, group(2, "Canonical event")));
		expect(store.getSnapshot().groups[0].body.name).toBe("Canonical event");
	});

	it("retains the event when it arrives before the response", async () => {
		const pending = deferred<GroupRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: GroupRecordingRequest) =>
				pending.promise,
		);
		const { store, writer } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][1];
		store.applyChange(groupEvent(12, group(2, "Event first")));
		const afterEvent = store.getSnapshot().groups;
		pending.resolve(
			outcome(request.requestId, stored(group(2, "Late response"))),
		);

		await writing;
		expect(store.getSnapshot().groups).toBe(afterEvent);
		expect(store.getSnapshot().groups[0].body.name).toBe("Event first");
	});

	it("settles a replayed stored no-change outcome without an event", async () => {
		const record = vi.fn(
			async (_showId: string, request: GroupRecordingRequest) =>
				outcome(request.requestId, stored(group(1, "Canonical")), {
					status: "no_change",
					replayed: true,
				}),
		);
		const { store, writer } = setup(record);
		expect(await writer.record(input())).toMatchObject({
			status: "no_change",
			replayed: true,
		});
		expect(store.getSnapshot().groups[0].body.name).toBe("Canonical");
	});

	it("rejects an impossible deleted no-change outcome", async () => {
		const { store, writer, onError } = setup(
			vi.fn(
				async (_showId: string, request: GroupRecordingRequest) =>
					({
						requestId: request.requestId,
						correlationId: CORRELATION_ID,
						replayed: false,
						showRevision: 8,
						status: "no_change",
						group: deleted(1),
					}) as unknown as GroupRecordingOutcome,
			),
		);

		expect(await writer.record(input("delete"))).toBeNull();
		expect(store.getSnapshot().groups).toEqual([group(1, "Original")]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Group no-change outcome must retain a stored projection",
			}),
		);
	});

	it("settles deletion response/event in either order", async () => {
		const responseFirst = setup(
			vi.fn(async (_showId: string, request: GroupRecordingRequest) =>
				outcome(request.requestId, deleted()),
			),
		);
		await responseFirst.writer.record(input("delete"));
		expect(responseFirst.store.getSnapshot().groups).toEqual([]);
		responseFirst.store.applyChange(groupDeletionEvent(12, 2));
		expect(responseFirst.store.getSnapshot().groups).toEqual([]);

		const pending = deferred<GroupRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: GroupRecordingRequest) =>
				pending.promise,
		);
		const eventFirst = setup(record);
		const writing = eventFirst.writer.record(input("delete"));
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][1];
		eventFirst.store.applyChange(groupDeletionEvent(12, 2));
		const afterEvent = eventFirst.store.getSnapshot().groups;
		pending.resolve(outcome(request.requestId, deleted()));

		await writing;
		expect(eventFirst.store.getSnapshot().groups).toBe(afterEvent);
	});

	it("retries one ambiguous request with the identical request ID", async () => {
		const record = vi.fn(
			async (_showId: string, request: GroupRecordingRequest) => {
				if (record.mock.calls.length === 1)
					throw new GroupRecordingActionError(
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
		expect(record.mock.calls[0][1].requestId).toBe(
			record.mock.calls[1][1].requestId,
		);
	});

	it("rolls back a conflict and repairs only the exact Group", async () => {
		const conflict = conflictError();
		const loadGroup = vi.fn(async () => group(7, "Repaired"));
		const { store, writer, onError } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadGroup,
		);

		expect(await writer.record(input())).toBeNull();
		expect(loadGroup).toHaveBeenCalledWith(SHOW_ID, "front");
		expect(store.getSnapshot().groups).toEqual([group(7, "Repaired")]);
		expect(store.getSnapshot()).toMatchObject({ status: "ready", error: null });
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("does not let a stale 404 repair delete a newer event", async () => {
		const repair = deferred<ShowObject<"group"> | null>();
		const { store, writer } = setup(
			vi.fn(async () => {
				throw conflictError();
			}),
			vi.fn(async () => repair.promise),
		);
		const writing = writer.record(input());
		await vi.waitFor(() =>
			expect(store.getSnapshot().pendingObjectKeys.size).toBe(0),
		);
		store.applyChange(groupEvent(13, group(8, "Newer event")));
		repair.resolve(null);

		expect(await writing).toBeNull();
		expect(store.getSnapshot().groups).toEqual([group(8, "Newer event")]);
	});

	it("ignores late outcomes after same-show authority replacement", async () => {
		const pending = deferred<GroupRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: GroupRecordingRequest) =>
				pending.promise,
		);
		const { store, writer, loadGroup, onError } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][1];
		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "group", [group(9, "Replacement")]);
		pending.resolve(outcome(request.requestId, stored(group(2, "Late"))));

		expect(await writing).toBeNull();
		expect(loadGroup).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().groups).toEqual([group(9, "Replacement")]);
	});

	it("ignores a late error after same-show authority replacement", async () => {
		const pending = deferred<GroupRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: GroupRecordingRequest) =>
				pending.promise,
		);
		const { store, writer, loadGroup, onError } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "group", [group(9, "Replacement")]);
		pending.reject(conflictError());

		expect(await writing).toBeNull();
		expect(loadGroup).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().groups).toEqual([group(9, "Replacement")]);
	});
});

function groupEvent(eventSequence: number, value: ShowObject<"group">) {
	return {
		showId: SHOW_ID,
		showRevision: 8,
		eventSequence,
		changes: [
			{
				kind: "group" as const,
				objectId: value.id,
				objectRevision: value.revision,
				body: value.body,
				deleted: false,
			},
		],
	};
}

function groupDeletionEvent(eventSequence: number, revision: number) {
	return {
		showId: SHOW_ID,
		showRevision: 8,
		eventSequence,
		changes: [
			{
				kind: "group" as const,
				objectId: "front",
				objectRevision: revision,
				body: null,
				deleted: true,
			},
		],
	};
}

function conflictError() {
	return new GroupRecordingActionError(
		"revision conflict",
		"conflict",
		409,
		7,
		false,
	);
}
