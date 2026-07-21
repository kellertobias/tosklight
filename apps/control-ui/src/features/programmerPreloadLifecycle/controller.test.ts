import { describe, expect, it, vi } from "vitest";
import { playbackSnapshot } from "../playbackRuntime/testFixtures";
import type { ProgrammerPreloadLifecycleOutcome } from "./contracts";
import { ProgrammerPreloadLifecycleController } from "./controller";
import {
	deferred,
	DESK_ID,
	goOutcome,
	lifecycleWriterHarness,
	outcome,
	SHOW_ID,
	USER_ID,
} from "./writerTestHarness";

function controllerHarness(
	setup = lifecycleWriterHarness(undefined, { blind: true }),
) {
	const releaseDesk = vi.fn();
	const activateDesk = vi.fn(() => releaseDesk);
	const onError = vi.fn();
	const controller = new ProgrammerPreloadLifecycleController({
		scope: { showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID },
		writer: setup.writer,
		runtime: { store: setup.runtimeStore, activateDesk },
		onError,
	});
	return { ...setup, controller, activateDesk, releaseDesk, onError };
}

async function settle() {
	for (let index = 0; index < 6; index++) await Promise.resolve();
}

describe("ProgrammerPreloadLifecycleController", () => {
	it("performs every non-GO action without activating Playback", async () => {
		const setup = controllerHarness(lifecycleWriterHarness());

		await setup.controller.enter("enter");
		await setup.controller.clearPending("clear");
		await setup.controller.release("release");

		expect(setup.apply.mock.calls.map(([, request]) => request.action.type)).toEqual([
			"enter",
			"clear_pending",
			"release",
		]);
		expect(setup.activateDesk).not.toHaveBeenCalled();
	});

	it("holds one desk-only authority through GO and queues Release behind it", async () => {
		const goResponse = deferred<ProgrammerPreloadLifecycleOutcome>();
		let setup!: ReturnType<typeof controllerHarness>;
		const writerSetup = lifecycleWriterHarness(async (_scope, request) => {
			if (request.action.type === "go") return goResponse.promise;
			setup.setActive(false);
			return outcome(request, {
				status: "changed",
				captureMode: {
					...setup.captureModeStore.getSnapshot().projection!,
					revision: 3,
				},
				captureModeEventSequence: 34,
				valuesRevision: 2,
				queueRevision: 3,
			});
		}, { blind: true });
		setup = controllerHarness(writerSetup);
		setup.activateDesk.mockImplementation(() => {
			setup.runtimeStore.setLoading();
			queueMicrotask(() =>
				setup.runtimeStore.installSnapshot(playbackSnapshot([], 30), []),
			);
			return setup.releaseDesk;
		});

		const go = setup.controller.go("go");
		const release = setup.controller.release("release");
		await settle();
		expect(setup.activateDesk).toHaveBeenCalledOnce();
		expect(setup.apply).toHaveBeenCalledOnce();
		expect(setup.releaseDesk).not.toHaveBeenCalled();

		setup.setActive(true);
		goResponse.resolve(goOutcome(setup.apply.mock.calls[0][1]));
		await expect(go).resolves.toMatchObject({ active: true });
		await expect(release).resolves.toMatchObject({ active: false });
		expect(setup.apply.mock.calls.map(([, request]) => request.action.type)).toEqual([
			"go",
			"release",
		]);
		expect(setup.releaseDesk).toHaveBeenCalledOnce();
	});

	it("fails closed without mutation when desk hydration errors", async () => {
		const setup = controllerHarness();
		setup.activateDesk.mockImplementation(() => {
			setup.runtimeStore.setLoading();
			queueMicrotask(() =>
				setup.runtimeStore.setError(new Error("desk snapshot failed")),
			);
			return setup.releaseDesk;
		});

		await expect(setup.controller.go("failed-go")).resolves.toBeNull();
		expect(setup.apply).not.toHaveBeenCalled();
		expect(setup.onError).toHaveBeenCalledWith(
			expect.objectContaining({ message: "desk snapshot failed" }),
		);
		expect(setup.releaseDesk).toHaveBeenCalledOnce();
	});

	it("does not hang or mutate after runtime authority replacement", async () => {
		const setup = controllerHarness();
		setup.activateDesk.mockImplementation(() => {
			setup.runtimeStore.setLoading();
			return setup.releaseDesk;
		});
		const go = setup.controller.go("replaced-go");
		await Promise.resolve();
		setup.runtimeStore.reset(SHOW_ID, DESK_ID, "session-b");

		await expect(go).resolves.toBeNull();
		expect(setup.apply).not.toHaveBeenCalled();
		expect(setup.releaseDesk).toHaveBeenCalledOnce();
	});
});
