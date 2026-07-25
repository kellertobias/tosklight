import { describe, expect, it, vi } from "vitest";
import { PresetRecordingActionError } from "../../api/PresetRecordingTransport";
import type { ShowObject } from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import type {
	PresetRecordingOutcome,
	PresetRecordingRequest,
	PresetRecordingTransport,
} from "./contracts";
import { PresetRecordingWriter } from "./writer";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function preset(
	revision: number,
	name: string,
	id = "2.1",
): ShowObject<"preset"> {
	return {
		kind: "preset",
		id,
		revision,
		updated_at: "",
		body: { name, number: 1, family: "Color", values: {} },
	};
}

function outcome(
	requestId: string,
	presetValue = preset(2, "Recorded"),
	options: { status?: "changed" | "no_change"; replayed?: boolean } = {},
): PresetRecordingOutcome {
	const base = {
		requestId,
		correlationId: CORRELATION_ID,
		replayed: options.replayed ?? false,
		showRevision: 8,
		preset: presetValue,
	};
	return options.status === "no_change"
		? { ...base, status: "no_change" }
		: { ...base, status: "changed", eventSequence: 12 };
}

function input(objectId = "2.1") {
	return {
		objectId,
		address: { family: "Color" as const, number: 1 },
		name: "Recorded",
		mode: "overwrite" as const,
		expectedObjectRevision: 1,
	};
}

function setup(
	record: PresetRecordingTransport["record"],
	loadPreset = vi.fn(async () => null as ShowObject<"preset"> | null),
) {
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "session-a");
	store.setCollection(SHOW_ID, "preset", [preset(1, "Original")], 10);
	const onError = vi.fn();
	const writer = new PresetRecordingWriter({
		showId: SHOW_ID,
		store,
		transport: { record },
		loadPreset,
		onError,
	});
	return { store, writer, loadPreset, onError };
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

describe("PresetRecordingWriter", () => {
	it("reports and refuses recording while the Preset collection is loading", async () => {
		const record = vi.fn();
		const store = new ShowObjectsStore();
		store.reset(SHOW_ID, "session-a");
		const onError = vi.fn();
		const writer = new PresetRecordingWriter({
			showId: SHOW_ID,
			store,
			transport: { record },
			loadPreset: vi.fn(),
			onError,
		});

		expect(await writer.record(input())).toBeNull();
		expect(record).not.toHaveBeenCalled();
		expect(onError).toHaveBeenCalledWith(
			expect.objectContaining({
				message: "Authoritative Preset collection is still loading",
			}),
		);
	});

	it("installs a response before its one canonical event", async () => {
		const record = vi.fn(async (_showId: string, request: PresetRecordingRequest) =>
			outcome(request.requestId),
		);
		const { store, writer } = setup(record);

		await writer.record(input());
		expect(store.getSnapshot().presets[0]).toMatchObject({
			revision: 2,
			body: { name: "Recorded" },
		});
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);

		store.applyChange({
			showId: SHOW_ID,
			showRevision: 8,
			eventSequence: 12,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 2,
					body: preset(2, "Canonical event").body,
					deleted: false,
				},
			],
		});
		expect(store.getSnapshot().presets[0].body.name).toBe("Canonical event");
	});

	it("retains the event when it arrives before the response", async () => {
		const pending = deferred<PresetRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: PresetRecordingRequest) => pending.promise,
		);
		const { store, writer } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][1];
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 8,
			eventSequence: 12,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 2,
					body: preset(2, "Event first").body,
					deleted: false,
				},
			],
		});
		pending.resolve(outcome(request.requestId, preset(2, "Late response")));

		await writing;
		expect(store.getSnapshot().presets[0].body.name).toBe("Event first");
	});

	it("settles a replayed no-change outcome without an event", async () => {
		const record = vi.fn(async (_showId: string, request: PresetRecordingRequest) =>
			outcome(request.requestId, preset(1, "Canonical"), {
				status: "no_change",
				replayed: true,
			}),
		);
		const { store, writer } = setup(record);

		const result = await writer.record(input());

		expect(result).toMatchObject({ status: "no_change", replayed: true });
		expect(store.getSnapshot().presets[0].body.name).toBe("Canonical");
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("replays one ambiguous request with the identical request ID", async () => {
		const record = vi.fn(
			async (_showId: string, request: PresetRecordingRequest) => {
				if (record.mock.calls.length === 1)
					throw new PresetRecordingActionError(
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

	it("rolls back a conflict and repairs only the exact Preset", async () => {
		const conflict = new PresetRecordingActionError(
			"revision conflict",
			"conflict",
			409,
			7,
			false,
		);
		const record = vi.fn(async () => {
			throw conflict;
		});
		const loadPreset = vi.fn(async () => preset(7, "Repaired"));
		const { store, writer, onError } = setup(record, loadPreset);

		expect(await writer.record(input())).toBeNull();

		expect(loadPreset).toHaveBeenCalledOnce();
		expect(loadPreset).toHaveBeenCalledWith(SHOW_ID, "2.1");
		expect(store.getSnapshot().presets[0]).toEqual(preset(7, "Repaired"));
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		expect(store.getSnapshot().status).toBe("ready");
		expect(store.getSnapshot().error).toBeNull();
		expect(onError).toHaveBeenLastCalledWith(conflict);
	});

	it("keeps an action failure out of shared Show Objects status", async () => {
		const failure = new Error("recording failed");
		const { store, writer, onError } = setup(vi.fn(async () => {
			throw failure;
		}));

		expect(await writer.record(input())).toBeNull();
		expect(store.getSnapshot()).toMatchObject({ status: "ready", error: null });
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
		expect(onError).toHaveBeenLastCalledWith(failure);
	});

	it("does not let a stale conflict repair overwrite a newer object event", async () => {
		const conflict = new PresetRecordingActionError(
			"revision conflict",
			"conflict",
			409,
			7,
			false,
		);
		const repair = deferred<ShowObject<"preset"> | null>();
		const loadPreset = vi.fn(async () => repair.promise);
		const { store, writer } = setup(
			vi.fn(async () => {
				throw conflict;
			}),
			loadPreset,
		);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(loadPreset).toHaveBeenCalledOnce());
		store.applyChange({
			showId: SHOW_ID,
			showRevision: 9,
			eventSequence: 11,
			changes: [
				{
					kind: "preset",
					objectId: "2.1",
					objectRevision: 8,
					body: null,
					deleted: true,
				},
			],
		});
		repair.resolve(preset(7, "Stale repair"));

		expect(await writing).toBeNull();
		expect(store.getSnapshot().presets).toEqual([]);
	});

	it("ignores a late response after same-show server authority replacement", async () => {
		const pending = deferred<PresetRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: PresetRecordingRequest) => pending.promise,
		);
		const { store, writer } = setup(record);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());
		const request = record.mock.calls[0][1];

		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "preset", [preset(9, "Replacement")]);
		pending.resolve(outcome(request.requestId, preset(2, "Late response")));

		expect(await writing).toBeNull();
		expect(store.getSnapshot().presets).toEqual([preset(9, "Replacement")]);
	});

	it("ignores a late error after same-show server authority replacement", async () => {
		const pending = deferred<PresetRecordingOutcome>();
		const record = vi.fn(
			async (_showId: string, _request: PresetRecordingRequest) => pending.promise,
		);
		const loadPreset = vi.fn(async () => preset(2, "Stale repair"));
		const { store, writer, onError } = setup(record, loadPreset);
		const writing = writer.record(input());
		await vi.waitFor(() => expect(record).toHaveBeenCalledOnce());

		store.reset(SHOW_ID, "session-b");
		store.setCollection(SHOW_ID, "preset", [preset(9, "Replacement")]);
		pending.reject(
			new PresetRecordingActionError(
				"late conflict",
				"conflict",
				409,
				9,
				false,
			),
		);

		expect(await writing).toBeNull();
		expect(loadPreset).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().presets).toEqual([preset(9, "Replacement")]);
	});
});
