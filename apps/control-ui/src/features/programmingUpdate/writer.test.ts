import { describe, expect, it, vi } from "vitest";
import type { UpdateMode, UpdateTargetRequest } from "../../api/types";
import type { ShowObject, ShowObjectsChange } from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import {
	type ProgrammingUpdateActionOutcome,
	type ProgrammingUpdateActionRequest,
	type ProgrammingUpdatePreviewRequest,
	type ProgrammingUpdatePreviewResponse,
	type ProgrammingUpdateScope,
	type ProgrammingUpdateTarget,
	type ProgrammingUpdateTransport,
	ProgrammingUpdateTransportError,
	type UpdatePreviewAuthority,
} from "./contracts";
import { ProgrammingUpdateWriter } from "./writer";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const USER_ID = "33333333-3333-4333-8333-333333333333";
const CORRELATION_ID = "44444444-4444-4444-8444-444444444444";
const SCOPE_KEY = "session-a";
const CUE_LIST_ID = "66666666-6666-4666-8666-666666666666";
const CUE_ID = "77777777-7777-4777-8777-777777777777";
const MODE: UpdateMode = {
	target_type: "existing_content",
	mode: "update_existing",
};
const TARGET: UpdateTargetRequest = {
	family: { type: "group" },
	object_id: "front",
};
const WIRE_TARGET: ProgrammingUpdateTarget = {
	type: "group",
	object_id: "front",
};

function group(revision: number, name: string): ShowObject<"group"> {
	return {
		kind: "group",
		id: "front",
		revision,
		updated_at: "",
		body: { name, fixtures: ["fixture-1"] },
	};
}

function cueList(revision: number, name: string): ShowObject<"cue_list"> {
	return {
		kind: "cue_list",
		id: "legacy-main",
		revision,
		updated_at: "",
		body: {
			id: CUE_LIST_ID,
			name: "Main",
			priority: 0,
			mode: "sequence",
			looped: false,
			cues: [
				{
					id: CUE_ID,
					number: 1,
					name,
					fade_millis: 0,
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

function authority(
	objectRevision = 1,
	showRevision = 7,
): UpdatePreviewAuthority {
	return {
		scopeKey: SCOPE_KEY,
		requestId: "55555555-5555-4555-8555-555555555555",
		correlationId: CORRELATION_ID,
		showId: SHOW_ID,
		showRevision,
		requestTarget: WIRE_TARGET,
		object: {
			kind: "group",
			object_id: "front",
			object_revision: objectRevision,
		},
		programmerRevision: "programmer-revision",
		preview: {
			revision: objectRevision,
			show_revision: showRevision,
			programmer_revision: "programmer-revision",
			target: targetIdentity(),
			mode: MODE,
			items: [],
		},
	};
}

function outcome(
	requestId: string,
	options: {
		name?: string;
		objectRevision?: number;
		showRevision?: number;
		eventSequence?: number;
		replayed?: boolean;
	} = {},
): ProgrammingUpdateActionOutcome {
	const objectRevision = options.objectRevision ?? 2;
	return {
		status: "changed",
		request_id: requestId,
		correlation_id: CORRELATION_ID,
		replayed: options.replayed ?? false,
		show_id: SHOW_ID,
		show_revision: options.showRevision ?? 8,
		projection: {
			kind: "group",
			object_id: "front",
			object_revision: objectRevision,
			body: group(objectRevision, options.name ?? "Response").body,
		},
		event_sequence: options.eventSequence ?? 12,
		summary: {
			target: targetIdentity(),
			revision_before: objectRevision - 1,
			revision_after: objectRevision,
			eligible_count: 1,
			changed_count: 1,
			added_count: 0,
			ignored_count: 0,
			changed_cues: [],
			programmer_values_retained: false,
		},
	};
}

function previewResponse(
	request: ProgrammingUpdatePreviewRequest,
): ProgrammingUpdatePreviewResponse {
	return {
		request_id: request.request_id,
		correlation_id: CORRELATION_ID,
		show_id: SHOW_ID,
		show_revision: 7,
		object: {
			kind: "group",
			object_id: "front",
			object_revision: 1,
		},
		programmer_revision: "programmer-revision",
		preview: { target: targetIdentity(), mode: MODE, items: [] },
	};
}

function cuePreviewResponse(
	request: ProgrammingUpdatePreviewRequest,
): ProgrammingUpdatePreviewResponse {
	return {
		request_id: request.request_id,
		correlation_id: CORRELATION_ID,
		show_id: SHOW_ID,
		show_revision: 9,
		object: {
			kind: "cue_list",
			object_id: "legacy-main",
			object_revision: 3,
		},
		programmer_revision: "programmer-revision",
		preview: {
			target: {
				family: { type: "cue" },
				object_id: CUE_LIST_ID,
				name: "Main",
				playback_number: 7,
				cue: { id: CUE_ID, number: 1 },
			},
			mode: { target_type: "cue", mode: "existing_only" },
			items: [],
		},
	};
}

function targetIdentity() {
	return {
		family: { type: "group" as const },
		object_id: "front",
		name: "Front",
	};
}

function groupEvent(
	eventSequence: number,
	value: ShowObject<"group">,
	showRevision = 8,
): ShowObjectsChange {
	return {
		showId: SHOW_ID,
		showRevision,
		eventSequence,
		changes: [
			{
				kind: "group",
				objectId: value.id,
				objectRevision: value.revision,
				body: value.body,
				deleted: false,
			},
		],
	};
}

function fakeTransport(
	overrides: Partial<ProgrammingUpdateTransport>,
): ProgrammingUpdateTransport {
	const unexpected = async () => {
		throw new Error("Unexpected Programming Update transport call");
	};
	return {
		preview: unexpected,
		targets: unexpected,
		apply: unexpected,
		loadSettings: unexpected,
		saveSettings: unexpected,
		...overrides,
	};
}

function setup(options: {
	apply: ProgrammingUpdateTransport["apply"];
	preview?: ProgrammingUpdateTransport["preview"];
	loadObject?: (
		showId: string,
		kind: "group",
		objectId: string,
	) => Promise<ShowObject<"group"> | null>;
	showRevision?: number | null;
}) {
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "session-a");
	store.setCollection(
		SHOW_ID,
		"group",
		[group(1, "Original")],
		10,
		options.showRevision === null ? undefined : (options.showRevision ?? 7),
	);
	const loadObject = vi.fn(
		options.loadObject ?? (async () => null as ShowObject<"group"> | null),
	);
	const transport = fakeTransport({
		apply: options.apply,
		...(options.preview ? { preview: options.preview } : {}),
	});
	const scope: ProgrammingUpdateScope = {
		showId: SHOW_ID,
		deskId: DESK_ID,
		userId: USER_ID,
		initialShowRevision: options.showRevision ?? null,
	};
	const writer = new ProgrammingUpdateWriter({
		scopeKey: SCOPE_KEY,
		scope,
		store,
		transport,
		loadObject: loadObject as never,
	});
	return { store, writer, transport, loadObject };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("ProgrammingUpdateWriter", () => {
	it("rejects a same-show preview authority from a replaced scope", async () => {
		const apply = vi.fn();
		const { store, writer } = setup({ apply });
		const foreign = { ...authority(), scopeKey: "server-b|session-b" };
		const before = store.getSnapshot();

		await expect(writer.confirm(foreign)).resolves.toBeNull();
		expect(apply).not.toHaveBeenCalled();
		expect(store.getSnapshot()).toBe(before);
	});

	it("reconciles the response before its retained event", async () => {
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				request: ProgrammingUpdateActionRequest,
			) => outcome(request.request_id),
		);
		const { store, writer } = setup({ apply });

		await expect(writer.confirm(authority())).resolves.toMatchObject({
			replayed: false,
		});
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Response"));
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);

		store.applyChange(groupEvent(12, group(2, "Canonical event")));
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Canonical event"));
	});

	it("retains an event that arrives before its HTTP response", async () => {
		const pending = deferred<ProgrammingUpdateActionOutcome>();
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				_request: ProgrammingUpdateActionRequest,
			) => pending.promise,
		);
		const { store, writer } = setup({ apply });
		const writing = writer.confirm(authority());
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const request = apply.mock.calls[0][2];

		store.applyChange(groupEvent(12, group(2, "Event first")));
		const afterEvent = store.getSnapshot().groups;
		pending.resolve(outcome(request.request_id, { name: "Late response" }));

		await expect(writing).resolves.toMatchObject({ showRevision: 8 });
		expect(store.getSnapshot().groups).toBe(afterEvent);
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Event first"));
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("retries once with the identical request and accepts a replayed outcome", async () => {
		const retryable = new ProgrammingUpdateTransportError(
			"temporarily unavailable",
			503,
			null,
			true,
		);
		const apply = vi
			.fn()
			.mockRejectedValueOnce(retryable)
			.mockImplementation(
				async (
					_show: string,
					_revision: number,
					request: ProgrammingUpdateActionRequest,
				) => outcome(request.request_id, { replayed: true }),
			);
		const { store, writer } = setup({ apply });

		await expect(writer.confirm(authority())).resolves.toMatchObject({
			replayed: true,
		});
		expect(apply).toHaveBeenCalledTimes(2);
		expect(apply.mock.calls[1][1]).toBe(apply.mock.calls[0][1]);
		expect(apply.mock.calls[1][2]).toBe(apply.mock.calls[0][2]);
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Response"));
	});

	it("abandons pending state when the changed-only contract rejects a no-op", async () => {
		const noOp = new ProgrammingUpdateTransportError(
			"no values are eligible for Update",
			400,
			null,
			false,
		);
		const { store, writer, loadObject } = setup({
			apply: vi.fn(async () => {
				throw noOp;
			}),
		});
		const before = store.getSnapshot();

		await expect(writer.confirm(authority())).rejects.toBe(noOp);
		const after = store.getSnapshot();
		expect(after.pendingObjectKeys.size).toBe(0);
		expect(after.groups).toBe(before.groups);
		expect(loadObject).not.toHaveBeenCalled();
	});

	it("repairs only the conflicted object and current show revision", async () => {
		const conflict = new ProgrammingUpdateTransportError(
			"revision conflict",
			409,
			9,
			false,
		);
		const repaired = group(4, "Repaired");
		const { store, writer, loadObject } = setup({
			apply: vi.fn(async () => {
				throw conflict;
			}),
			loadObject: async () => repaired,
		});

		await expect(writer.confirm(authority())).rejects.toBe(conflict);
		expect(store.getSnapshot().showRevision).toBe(9);
		expect(loadObject).toHaveBeenCalledOnce();
		expect(loadObject).toHaveBeenCalledWith(SHOW_ID, "group", "front");
		expect(store.getSnapshot().groups).toEqual([repaired]);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("drops a late response after same-show session authority replacement", async () => {
		const pending = deferred<ProgrammingUpdateActionOutcome>();
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				_request: ProgrammingUpdateActionRequest,
			) => pending.promise,
		);
		const { store, writer } = setup({ apply });
		const writing = writer.confirm(authority());
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const request = apply.mock.calls[0][2];

		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "group", [group(9, "Replacement")], 20, 20);
		pending.resolve(outcome(request.request_id, { name: "Late" }));

		await expect(writing).resolves.toBeNull();
		expect(store.getSnapshot().groups).toEqual([group(9, "Replacement")]);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("sends one direct action without preview and seals its response event", async () => {
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				request: ProgrammingUpdateActionRequest,
			) => outcome(request.request_id),
		);
		const preview = vi.fn();
		const { store, writer } = setup({ apply, preview });

		await writer.applyDirect(TARGET, MODE);
		expect(preview).not.toHaveBeenCalled();
		expect(apply).toHaveBeenCalledOnce();
		expect(apply).toHaveBeenCalledWith(
			SHOW_ID,
			7,
			expect.objectContaining({
				action: { type: "apply_direct", target: WIRE_TARGET, mode: MODE },
			}),
		);
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Response"));

		store.applyChange(groupEvent(12, group(2, "Duplicate event")));
		expect(store.getSnapshot().groups[0]).toEqual(group(2, "Response"));
	});

	it("uses one narrow preview before one direct action when revision is absent", async () => {
		const preview = vi.fn(
			async (_show: string, request: ProgrammingUpdatePreviewRequest) =>
				previewResponse(request),
		);
		const apply = vi.fn(
			async (
				_show: string,
				_revision: number,
				request: ProgrammingUpdateActionRequest,
			) => outcome(request.request_id),
		);
		const { writer } = setup({ apply, preview, showRevision: null });

		await writer.applyDirect(TARGET, MODE);
		expect(preview).toHaveBeenCalledOnce();
		expect(preview).toHaveBeenCalledWith(
			SHOW_ID,
			expect.objectContaining({ target: WIRE_TARGET, mode: MODE }),
		);
		expect(apply).toHaveBeenCalledOnce();
		expect(apply.mock.calls[0][1]).toBe(7);
	});

	it("repairs an exact direct Cue conflict with one narrow preview and object read", async () => {
		const conflict = new ProgrammingUpdateTransportError(
			"revision conflict",
			409,
			9,
			false,
		);
		const apply = vi.fn(async () => {
			throw conflict;
		});
		const preview = vi.fn(
			async (_show: string, request: ProgrammingUpdatePreviewRequest) =>
				cuePreviewResponse(request),
		);
		const repaired = cueList(3, "Repaired");
		const { store, writer, loadObject } = setup({
			apply,
			preview,
			loadObject: (async () => repaired) as never,
		});
		const cueTarget: UpdateTargetRequest = {
			family: { type: "cue" },
			object_id: CUE_LIST_ID,
			playback_number: 7,
			cue_id: CUE_ID,
			cue_number: 1,
			validate_active_context: false,
		};
		const cueMode: UpdateMode = {
			target_type: "cue",
			mode: "existing_only",
		};

		await expect(writer.applyDirect(cueTarget, cueMode)).rejects.toBe(conflict);
		expect(apply).toHaveBeenCalledOnce();
		expect(preview).toHaveBeenCalledOnce();
		expect(apply.mock.invocationCallOrder[0]).toBeLessThan(
			preview.mock.invocationCallOrder[0],
		);
		expect(loadObject).toHaveBeenCalledOnce();
		expect(loadObject).toHaveBeenCalledWith(SHOW_ID, "cue_list", "legacy-main");
		expect(store.getSnapshot().showRevision).toBe(9);
		expect(store.getSnapshot().cueLists).toEqual([repaired]);
	});

	it("serializes actions so the second uses the first committed show revision", async () => {
		const first = deferred<ProgrammingUpdateActionOutcome>();
		const apply = vi
			.fn()
			.mockImplementationOnce(async () => first.promise)
			.mockImplementationOnce(
				async (
					_show: string,
					_revision: number,
					request: ProgrammingUpdateActionRequest,
				) =>
					outcome(request.request_id, {
						name: "Second",
						objectRevision: 3,
						showRevision: 9,
						eventSequence: 13,
					}),
			);
		const { store, writer } = setup({ apply });
		const firstWrite = writer.applyDirect(TARGET, MODE);
		const secondWrite = writer.applyDirect(TARGET, MODE);
		await vi.waitFor(() => expect(apply).toHaveBeenCalledOnce());
		const firstRequest = apply.mock.calls[0][2];
		expect(apply.mock.calls[0][1]).toBe(7);

		first.resolve(outcome(firstRequest.request_id));
		await firstWrite;
		await secondWrite;

		expect(apply).toHaveBeenCalledTimes(2);
		expect(apply.mock.calls[1][1]).toBe(8);
		expect(store.getSnapshot().showRevision).toBe(9);
		expect(store.getSnapshot().groups[0]).toEqual(group(3, "Second"));
	});
});
