import { describe, expect, it, vi } from "vitest";
import type { CommandLineProjection } from "./contracts";
import { ProgrammingCommandLineWriter } from "./commandLineWriter";
import { ProgrammingInteractionStore } from "./store";
import {
	commandLine,
	DESK_ID,
	programmingSnapshot,
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

function readyStore(command = commandLine()) {
	const store = new ProgrammingInteractionStore();
	store.reset(SHOW_ID, DESK_ID);
	store.installSnapshot(programmingSnapshot({ command }));
	return store;
}

const OPTIMISTIC_RESET = {
	text: "FIXTURE",
	target: "FIXTURE" as const,
	pristine: true,
	pendingChoice: null,
};

describe("ProgrammingCommandLineWriter", () => {
	it("coalesces unsent edits and chooses each revision immediately before sending", async () => {
		const store = readyStore();
		const firstRequest = deferred<CommandLineProjection>();
		const lastRequest = deferred<CommandLineProjection>();
		const replace = vi
			.fn()
			.mockReturnValueOnce(firstRequest.promise)
			.mockReturnValueOnce(lastRequest.promise);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot: vi.fn(),
		});

		const first = writer.replace("FIXTURE 1");
		await vi.waitFor(() => expect(replace).toHaveBeenCalledTimes(1));
		const superseded = writer.replace("FIXTURE 12");
		const last = writer.replace("FIXTURE 123");

		expect(store.getSnapshot().commandLine?.text).toBe("FIXTURE 123");
		await expect(superseded).resolves.toBe(true);
		firstRequest.resolve(commandLine(2, "FIXTURE 1"));
		await expect(first).resolves.toBe(true);
		await vi.waitFor(() => expect(replace).toHaveBeenCalledTimes(2));
		expect(replace).toHaveBeenNthCalledWith(
			2,
			DESK_ID,
			"FIXTURE 123",
			2,
		);
		expect(store.getSnapshot().commandLine).toMatchObject({
			text: "FIXTURE 123",
			revision: 2,
		});

		lastRequest.resolve(commandLine(3, "FIXTURE 123"));
		await expect(last).resolves.toBe(true);
		await writer.flush();
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(3, "FIXTURE 123"),
		);
	});

	it("reconciles when the authoritative event arrives before the response", async () => {
		const store = readyStore();
		const request = deferred<CommandLineProjection>();
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace: vi.fn().mockReturnValue(request.promise),
			loadSnapshot: vi.fn(),
		});
		const write = writer.replace("FIXTURE 9");
		const authority = commandLine(2, "FIXTURE 9");
		store.applyChange({ deskId: DESK_ID, commandLine: authority }, 11);

		request.resolve(authority);
		await expect(write).resolves.toBe(true);
		expect(store.getSnapshot().commandLine).toEqual(authority);
		expect(store.getSnapshot().eventSequence).toBe(11);
	});

	it("repairs a conflict without overwriting the concurrent authoritative edit", async () => {
		const store = readyStore();
		const conflict = Object.assign(new Error("revision conflict"), {
			status: 409,
		});
		const replace = vi.fn().mockRejectedValueOnce(conflict);
		const loadSnapshot = vi.fn().mockResolvedValue(
			programmingSnapshot({
				sequence: 17,
				command: commandLine(4, "GROUP", "GROUP"),
			}),
		);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot,
		});

		await expect(writer.replace("FIXTURE 8")).resolves.toBe(false);

		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(replace).toHaveBeenNthCalledWith(1, DESK_ID, "FIXTURE 8", 1);
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(4, "GROUP", "GROUP"),
		);
	});

	it("rolls back only a failed write and preserves the later optimistic edit", async () => {
		const store = readyStore();
		const firstRequest = deferred<CommandLineProjection>();
		const replace = vi
			.fn()
			.mockReturnValueOnce(firstRequest.promise)
			.mockResolvedValueOnce(commandLine(2, "FIXTURE 12"));
		const onError = vi.fn();
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot: vi.fn(),
			onError,
		});

		const failed = writer.replace("FIXTURE 1");
		const later = writer.replace("FIXTURE 12");
		const flushed = writer.flush();
		firstRequest.reject(new Error("offline"));

		await expect(failed).resolves.toBe(false);
		expect(store.getSnapshot().commandLine?.text).toBe("FIXTURE 12");
		await expect(later).resolves.toBe(true);
		await expect(flushed).resolves.toBe(true);
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(2, "FIXTURE 12"),
		);
		expect(store.getSnapshot().status).toBe("ready");
		expect(onError.mock.calls[0][0]).toEqual(new Error("offline"));
		expect(onError).toHaveBeenLastCalledWith(null);
	});

	it("does not clear an active stream error when a mutation succeeds", async () => {
		const store = readyStore();
		const streamError = new Error("programming stream disconnected");
		store.setError(streamError);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace: vi.fn().mockResolvedValue(commandLine(2, "FIXTURE 7")),
			loadSnapshot: vi.fn(),
		});

		await expect(writer.replace("FIXTURE 7")).resolves.toBe(true);

		expect(store.getSnapshot()).toMatchObject({
			status: "error",
			error: streamError,
		});
	});

	it("gates edits typed after Enter until execution is reconciled", async () => {
		const store = readyStore();
		const beforeExecution = deferred<CommandLineProjection>();
		const afterExecution = deferred<CommandLineProjection>();
		const replace = vi
			.fn()
			.mockReturnValueOnce(beforeExecution.promise)
			.mockReturnValueOnce(afterExecution.promise);
		const loadSnapshot = vi.fn().mockResolvedValue(
			programmingSnapshot({
				sequence: 12,
				command: commandLine(3),
			}),
		);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot,
		});
		const execution = deferred<boolean>();
		const execute = vi.fn(() => execution.promise);

		const before = writer.replace("FIXTURE 1");
		const outcome = writer.executeAfterPendingWrites(
			execute,
			OPTIMISTIC_RESET,
		);
		expect(store.getSnapshot().commandLine?.text).toBe("FIXTURE");
		const after = writer.replace(
			`${store.getSnapshot().commandLine?.text} 2`,
		);
		expect(store.getSnapshot().commandLine?.text).toBe("FIXTURE 2");
		expect(replace).toHaveBeenCalledTimes(1);

		beforeExecution.resolve(commandLine(2, "FIXTURE 1"));
		await expect(before).resolves.toBe(true);
		await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce());
		expect(replace).toHaveBeenCalledTimes(1);

		execution.resolve(true);
		await expect(outcome).resolves.toBe("executed");
		await vi.waitFor(() => expect(replace).toHaveBeenCalledTimes(2));
		expect(replace).toHaveBeenLastCalledWith(DESK_ID, "FIXTURE 2", 3);
		afterExecution.resolve(commandLine(4, "FIXTURE 2"));
		await expect(after).resolves.toBe(true);
	});

	it("executes the latest queued command after an obsolete write fails", async () => {
		const store = readyStore();
		const obsolete = deferred<CommandLineProjection>();
		const replace = vi
			.fn()
			.mockReturnValueOnce(obsolete.promise)
			.mockResolvedValueOnce(commandLine(2, "FIXTURE 12"));
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot: vi.fn().mockResolvedValue(
				programmingSnapshot({ sequence: 12, command: commandLine(3) }),
			),
		});
		const execute = vi.fn().mockResolvedValue(true);

		const failed = writer.replace("FIXTURE 1");
		const latest = writer.replace("FIXTURE 12");
		const outcome = writer.executeAfterPendingWrites(
			execute,
			OPTIMISTIC_RESET,
		);
		obsolete.reject(new Error("offline"));

		await expect(failed).resolves.toBe(false);
		await expect(latest).resolves.toBe(true);
		await expect(outcome).resolves.toBe("executed");
		expect(replace).toHaveBeenLastCalledWith(DESK_ID, "FIXTURE 12", 1);
		expect(execute).toHaveBeenCalledOnce();
	});

	it("does not send post-Enter edits when reconciliation fails", async () => {
		const store = readyStore(commandLine(2, "FIXTURE 1"));
		const replace = vi.fn();
		const synchronizationError = new Error("snapshot unavailable");
		const onError = vi.fn();
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot: vi.fn().mockRejectedValue(synchronizationError),
			onError,
		});

		const outcome = writer.executeAfterPendingWrites(
			vi.fn().mockResolvedValue(true),
			OPTIMISTIC_RESET,
		);
		const after = writer.replace("FIXTURE 2");

		await expect(outcome).resolves.toBe("execution_unknown");
		await expect(after).resolves.toBe(false);
		expect(replace).not.toHaveBeenCalled();
		expect(onError).toHaveBeenLastCalledWith(synchronizationError);
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(2, "FIXTURE 1"),
		);
	});

	it("repairs authority after an uncertain pre-Enter write failure", async () => {
		const store = readyStore();
		const request = deferred<CommandLineProjection>();
		const loadSnapshot = vi.fn().mockResolvedValue(
			programmingSnapshot({
				sequence: 12,
				command: commandLine(2, "FIXTURE 1"),
			}),
		);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace: vi.fn().mockReturnValue(request.promise),
			loadSnapshot,
		});
		const execute = vi.fn().mockResolvedValue(true);

		const write = writer.replace("FIXTURE 1");
		const outcome = writer.executeAfterPendingWrites(
			execute,
			OPTIMISTIC_RESET,
		);
		const after = writer.replace("FIXTURE 2");
		request.reject(new Error("response lost"));

		await expect(write).resolves.toBe(false);
		await expect(outcome).resolves.toBe("write_failed");
		await expect(after).resolves.toBe(false);
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(execute).not.toHaveBeenCalled();
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(2, "FIXTURE 1"),
		);
	});

	it("shares the active execution result with a repeated Enter", async () => {
		const store = readyStore(commandLine(2, "FIXTURE 1"));
		const execution = deferred<boolean>();
		const execute = vi.fn(() => execution.promise);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace: vi.fn(),
			loadSnapshot: vi.fn().mockResolvedValue(
				programmingSnapshot({ sequence: 12, command: commandLine(3) }),
			),
		});

		const first = writer.executeAfterPendingWrites(execute, OPTIMISTIC_RESET);
		const repeated = writer.executeAfterPendingWrites(
			vi.fn().mockResolvedValue(false),
			OPTIMISTIC_RESET,
		);
		expect(repeated).toBe(first);
		await vi.waitFor(() => expect(execute).toHaveBeenCalledOnce());
		execution.resolve(true);

		await expect(first).resolves.toBe("executed");
		await expect(repeated).resolves.toBe("executed");
	});

	it("settles writes and barriers promptly when stopped during an active request", async () => {
		const store = readyStore();
		const request = deferred<CommandLineProjection>();
		const replace = vi.fn().mockReturnValue(request.promise);
		const loadSnapshot = vi.fn();
		const execute = vi.fn().mockResolvedValue(true);
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace,
			loadSnapshot,
		});

		const write = writer.replace("FIXTURE 7");
		await vi.waitFor(() => expect(replace).toHaveBeenCalledOnce());
		const queued = writer.replace("FIXTURE 78");
		const flush = writer.flush();
		const execution = writer.executeAfterPendingWrites(
			execute,
			OPTIMISTIC_RESET,
		);
		writer.stop();

		await expect(write).resolves.toBe(false);
		await expect(queued).resolves.toBe(false);
		await expect(flush).resolves.toBe(false);
		await expect(execution).resolves.toBe("write_failed");
		expect(execute).not.toHaveBeenCalled();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());
	});

	it("retains an execution error after successful authority reconciliation", async () => {
		const store = readyStore(commandLine(2, "FIXTURE 1"));
		const executionError = new Error("execution transport failed");
		const onError = vi.fn();
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace: vi.fn(),
			loadSnapshot: vi.fn().mockResolvedValue(
				programmingSnapshot({ sequence: 12, command: commandLine(3) }),
			),
			onError,
		});

		await expect(
			writer.executeAfterPendingWrites(
				vi.fn().mockRejectedValue(executionError),
				OPTIMISTIC_RESET,
			),
		).resolves.toBe("execution_failed");

		expect(onError).toHaveBeenLastCalledWith(executionError);
		expect(onError).not.toHaveBeenCalledWith(null);
	});

	it("does not repair or report a conflict after the writer is stopped", async () => {
		const store = readyStore();
		const request = deferred<CommandLineProjection>();
		const loadSnapshot = vi.fn();
		const onError = vi.fn();
		const writer = new ProgrammingCommandLineWriter({
			deskId: DESK_ID,
			store,
			replace: vi.fn().mockReturnValue(request.promise),
			loadSnapshot,
			onError,
		});
		const write = writer.replace("FIXTURE 8");
		await Promise.resolve();

		writer.stop();
		request.reject(Object.assign(new Error("revision conflict"), { status: 409 }));

		await expect(write).resolves.toBe(false);
		await Promise.resolve();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(onError).not.toHaveBeenCalled();
		expect(store.getSnapshot().commandLine).toEqual(commandLine());
	});
});
