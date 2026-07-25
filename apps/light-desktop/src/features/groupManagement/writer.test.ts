import { describe, expect, it, vi } from "vitest";
import { GroupManagementActionError } from "../../api/GroupManagementTransport";
import type { ShowObject } from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	GroupManagementOperation,
	GroupManagementOutcome,
	GroupManagementRequest,
	GroupManagementTransport,
} from "./contracts";
import { GroupManagementWriter } from "./writer";

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

function outcome(
	requestId: string,
	value = group(2, "Managed"),
	options: {
		status?: "changed" | "no_change";
		replayed?: boolean;
		persistenceWarning?: string | null;
	} = {},
): GroupManagementOutcome {
	const base = {
		requestId,
		correlationId: CORRELATION_ID,
		replayed: options.replayed ?? false,
		showId: SHOW_ID,
		showRevision: 8,
		group: { id: value.id, revision: value.revision, object: value },
		persistenceWarning: options.persistenceWarning ?? null,
	};
	if (options.status === "no_change") return { ...base, status: "no_change" };
	return { ...base, status: "changed", eventSequence: 12 };
}

const RENAME: GroupManagementOperation = {
	type: "update_properties",
	properties: { name: "Managed", color: null, icon: null },
};

function input(operation: GroupManagementOperation = RENAME) {
	return { objectId: "front", expectedObjectRevision: 1, operation };
}

function setup(
	manage: GroupManagementTransport["manage"],
	loadGroup = vi.fn(async () => null as ShowObject<"group"> | null),
) {
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "session-a");
	store.setCollection(SHOW_ID, "group", [group(1, "Original")], 10);
	const onError = vi.fn();
	const writer = new GroupManagementWriter({
		showId: SHOW_ID,
		store,
		transport: { manage },
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

describe("GroupManagementWriter", () => {
	it("refuses to act before the Group collection is ready", async () => {
		const manage = vi.fn();
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		const onError = vi.fn();
		const writer = new GroupManagementWriter({
			showId: SHOW_ID,
			store,
			transport: { manage },
			loadGroup: vi.fn(),
			onError,
		});

		expect(await writer.manage(input())).toBeNull();
		expect(manage).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Authoritative Group collection is still loading",
			}),
		);
	});

	it("installs a response before its one authoritative event", async () => {
		const manage = vi.fn(
			async (_showId: string, request: GroupManagementRequest) =>
				outcome(request.requestId),
		);
		const { store, writer } = setup(manage);

		await writer.manage(input());
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Managed"));
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		store.applyChange(groupEvent(12, group(2, "Canonical event")));
		expect(store.getSnapshot().groups[0].body.name).toBe("Canonical event");
	});

	it("retains the event when it arrives before the response", async () => {
		const pending = deferred<GroupManagementOutcome>();
		const manage = vi.fn(
			async (_showId: string, _request: GroupManagementRequest) => pending.promise,
		);
		const { store, writer } = setup(manage);
		const writing = writer.manage(input());
		await vi.waitFor(() => expect(manage).toHaveBeenCalledOnce());
		const request = manage.mock.calls[0]![1];
		store.applyChange(groupEvent(12, group(2, "Event first")));
		const afterEvent = store.getSnapshot().groups;
		pending.resolve(outcome(request.requestId, group(2, "Late response")));

		await writing;
		expect(store.getSnapshot().groups).toBe(afterEvent);
		expect(store.getSnapshot().groups[0].body.name).toBe("Event first");
	});

	it("settles a replayed no-change outcome without an event", async () => {
		const manage = vi.fn(
			async (_showId: string, request: GroupManagementRequest) =>
				outcome(request.requestId, group(1, "Canonical"), {
					status: "no_change",
					replayed: true,
				}),
		);
		const { store, writer } = setup(manage);

		expect(await writer.manage(input())).toMatchObject({
			status: "no_change",
			replayed: true,
		});
		expect(store.getSnapshot().groups[0].body.name).toBe("Canonical");
	});

	it("rejects an undo outcome that reports no authoritative change", async () => {
		const { store, writer, onError } = setup(
			vi.fn(async (_showId: string, request: GroupManagementRequest) =>
				outcome(request.requestId, group(1, "Unchanged"), {
					status: "no_change",
				}),
			),
		);

		expect(await writer.manage(input({ type: "undo" }))).toBeNull();
		expect(store.getSnapshot().groups).toEqual([group(1, "Original")]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Group undo must report an authoritative change",
			}),
		);
	});

	it("rejects a projection whose revision does not follow the request", async () => {
		const { store, writer, onError } = setup(
			vi.fn(async (_showId: string, request: GroupManagementRequest) =>
				outcome(request.requestId, group(9, "Impossible")),
			),
		);

		expect(await writer.manage(input())).toBeNull();
		expect(store.getSnapshot().groups).toEqual([group(1, "Original")]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Group management response revision is inconsistent",
			}),
		);
	});

	it("retries one ambiguous request with the identical request ID", async () => {
		const manage = vi.fn(
			async (_showId: string, request: GroupManagementRequest) => {
				if (manage.mock.calls.length === 1)
					throw new GroupManagementActionError(
						"connection lost",
						"unavailable",
						0,
						null,
						true,
					);
				return outcome(request.requestId);
			},
		);
		const { writer } = setup(manage);

		await writer.manage(input());

		expect(manage).toHaveBeenCalledTimes(2);
		expect(manage.mock.calls[0]![1].requestId).toBe(
			manage.mock.calls[1]![1].requestId,
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

		expect(await writer.manage(input())).toBeNull();
		expect(loadGroup).toHaveBeenCalledWith(SHOW_ID, "front");
		expect(store.getSnapshot().groups).toEqual([group(7, "Repaired")]);
		expect(store.getSnapshot()).toMatchObject({ status: "ready", error: null });
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("serializes concurrent operations in request order", async () => {
		const gates = [
			deferred<GroupManagementOutcome>(),
			deferred<GroupManagementOutcome>(),
		];
		const manage = vi.fn(
			async (_showId: string, _request: GroupManagementRequest) =>
				gates[manage.mock.calls.length - 1]!.promise,
		);
		const { writer } = setup(manage);

		const first = writer.manage(input());
		const second = writer.manage({
			objectId: "front",
			expectedObjectRevision: 2,
			operation: { type: "undo" },
		});
		await vi.waitFor(() => expect(manage).toHaveBeenCalledOnce());
		expect(manage).toHaveBeenCalledTimes(1);

		gates[0].resolve(outcome(manage.mock.calls[0]![1].requestId));
		await first;
		await vi.waitFor(() => expect(manage).toHaveBeenCalledTimes(2));
		gates[1].resolve(
			outcome(manage.mock.calls[1]![1].requestId, group(3, "Undone")),
		);

		expect(await second).toMatchObject({ status: "changed" });
	});

	it("ignores late outcomes after same-show authority replacement", async () => {
		const pending = deferred<GroupManagementOutcome>();
		const manage = vi.fn(
			async (_showId: string, _request: GroupManagementRequest) => pending.promise,
		);
		const { store, writer, loadGroup, onError } = setup(manage);
		const writing = writer.manage(input());
		await vi.waitFor(() => expect(manage).toHaveBeenCalledOnce());
		const request = manage.mock.calls[0]![1];
		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "group", [group(9, "Replacement")]);
		pending.resolve(outcome(request.requestId, group(2, "Late")));

		expect(await writing).toBeNull();
		expect(loadGroup).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().groups).toEqual([group(9, "Replacement")]);
	});

	it("ignores a late error after same-show authority replacement", async () => {
		const pending = deferred<GroupManagementOutcome>();
		const manage = vi.fn(
			async (_showId: string, _request: GroupManagementRequest) => pending.promise,
		);
		const { store, writer, loadGroup, onError } = setup(manage);
		const writing = writer.manage(input());
		await vi.waitFor(() => expect(manage).toHaveBeenCalledOnce());
		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "group", [group(9, "Replacement")]);
		pending.reject(conflictError());

		expect(await writing).toBeNull();
		expect(loadGroup).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().groups).toEqual([group(9, "Replacement")]);
	});

	it("stops accepting work after the owning writer is replaced", async () => {
		const manage = vi.fn(
			async (_showId: string, request: GroupManagementRequest) =>
				outcome(request.requestId),
		);
		const { writer } = setup(manage);
		writer.stop();

		expect(await writer.manage(input())).toBeNull();
		expect(manage).not.toHaveBeenCalled();
	});

	it("surfaces a persistence warning without discarding the authoritative body", async () => {
		const manage = vi.fn(
			async (_showId: string, request: GroupManagementRequest) =>
				outcome(request.requestId, group(2, "Managed"), {
					persistenceWarning: "backup volume is read-only",
				}),
		);
		const { store, writer, onError } = setup(manage);

		expect(await writer.manage(input())).toMatchObject({ status: "changed" });
		expect(store.getSnapshot().groups).toEqual([group(2, "Managed")]);
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "backup volume is read-only" }),
		);
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

function conflictError() {
	return new GroupManagementActionError(
		"revision conflict",
		"conflict",
		409,
		7,
		false,
	);
}
