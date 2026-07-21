import { describe, expect, it } from "vitest";
import { ProgrammerPreloadLifecycleTransportError } from "./contracts";
import {
	captureMode,
	deferred,
	lifecycleWriterHarness as harness,
	OTHER_ID,
	outcome,
	SHOW_ID,
	USER_ID,
	values,
} from "./writerTestHarness";

function transportError(
	kind: "invalid" | "conflict" | "unavailable",
	status: number,
	retryable = false,
) {
	return new ProgrammerPreloadLifecycleTransportError(
		`${kind} lifecycle request`,
		kind,
		status,
		kind === "conflict" ? 2 : null,
		kind === "conflict" ? 3 : null,
		retryable,
	);
}

describe("ProgrammerPreloadLifecycleWriter recovery", () => {
	it("retries one ambiguous request with the identical request object", async () => {
		let attempt = 0;
		const setup = harness(async (_scope, request) => {
			attempt++;
			if (attempt === 1) throw transportError("unavailable", 0, true);
			return outcome(request, { replayed: true });
		});

		await expect(setup.writer.release("replay-request")).resolves.toMatchObject({
			replayed: true,
		});
		expect(setup.apply).toHaveBeenCalledTimes(2);
		expect(setup.apply.mock.calls[1][1]).toBe(setup.apply.mock.calls[0][1]);
	});

	it("rolls back a definitive rejection without retrying", async () => {
		const response = deferred<never>();
		const setup = harness(() => response.promise, { active: true });
		const released = setup.writer.release("rejected-request");
		await Promise.resolve();
		expect(setup.localStore.getSnapshot().pending).toMatchObject({
			action: "release",
			optimisticActive: false,
		});

		response.reject(transportError("invalid", 400));
		await expect(released).resolves.toBeNull();
		expect(setup.apply).toHaveBeenCalledOnce();
		expect(setup.localStore.getSnapshot()).toMatchObject({
			pending: null,
			error: { message: "invalid lifecycle request" },
		});
	});

	it("repairs exact non-GO authorities on conflict without Playback repair", async () => {
		const setup = harness(async () => {
			throw transportError("conflict", 409);
		});

		await expect(setup.writer.clearPending("conflict-request")).resolves.toBeNull();
		expect(setup.repair.captureMode).toHaveBeenCalledOnce();
		expect(setup.repair.values).toHaveBeenCalledOnce();
		expect(setup.repair.queue).toHaveBeenCalledOnce();
		expect(setup.repair.selection).toHaveBeenCalledOnce();
		expect(setup.repair.lifecycle).toHaveBeenCalledOnce();
		expect(setup.repair.runtime).not.toHaveBeenCalled();
	});

	it("includes the desk-only Playback repair for a GO conflict", async () => {
		const setup = harness(async () => {
			throw transportError("conflict", 409);
		}, { blind: true });

		await expect(setup.writer.go("go-conflict")).resolves.toBeNull();
		expect(setup.repair.runtime).toHaveBeenCalledOnce();
	});

	it("abandons late responses after Show or session authority replacement", async () => {
		const response = deferred<ReturnType<typeof outcome>>();
		const showReplaced = harness(() => response.promise);
		const entered = showReplaced.writer.enter("late-show");
		await Promise.resolve();
		const request = showReplaced.apply.mock.calls[0][1];
		showReplaced.showStore.reset(SHOW_ID, "session-b");
		response.resolve(
			outcome(request, {
				status: "changed",
				captureMode: captureMode(2, true),
				captureModeEventSequence: 20,
			}),
		);

		await expect(entered).resolves.toBeNull();
		expect(showReplaced.captureModeStore.getSnapshot().projection?.revision).toBe(1);
		expect(showReplaced.localStore.getSnapshot().pending).toBeNull();

		const lateSession = deferred<ReturnType<typeof outcome>>();
		const sessionReplaced = harness(() => lateSession.promise);
		const released = sessionReplaced.writer.release("late-session");
		await Promise.resolve();
		const sessionRequest = sessionReplaced.apply.mock.calls[0][1];
		sessionReplaced.localStore.reset(
			SHOW_ID,
			USER_ID,
			sessionReplaced.selectionStore.getSnapshot().deskId,
			"session-b",
		);
		lateSession.resolve(outcome(sessionRequest));
		await expect(released).resolves.toBeNull();
	});

	it("refuses foreign user, desk, and unavailable lifecycle authority", async () => {
		const foreignUser = harness();
		foreignUser.valuesStore.reset(SHOW_ID, OTHER_ID, "session-a");
		foreignUser.valuesStore.installSnapshot({
			cursor: 12,
			projection: { ...values(), userId: OTHER_ID },
		});
		await expect(foreignUser.writer.release("foreign-user")).resolves.toBeNull();
		expect(foreignUser.apply).not.toHaveBeenCalled();

		const foreignDesk = harness();
		foreignDesk.selectionStore.reset(SHOW_ID, OTHER_ID, "session-a");
		await expect(foreignDesk.writer.release("foreign-desk")).resolves.toBeNull();
		expect(foreignDesk.apply).not.toHaveBeenCalled();

		const lifecycleLoading = harness(undefined, { active: null });
		await expect(
			lifecycleLoading.writer.release("loading-lifecycle"),
		).resolves.toBeNull();
		expect(lifecycleLoading.apply).not.toHaveBeenCalled();
	});
});
