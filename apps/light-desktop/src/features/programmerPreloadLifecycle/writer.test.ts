import { describe, expect, it } from "vitest";
import type { ProgrammerPreloadLifecycleOutcome } from "./contracts";
import {
	captureMode,
	deferred,
	goOutcome,
	lifecycleWriterHarness as harness,
	outcome,
} from "./writerTestHarness";

describe("ProgrammerPreloadLifecycleWriter reconciliation", () => {
	it("reconciles response-before-event and event-before-response", async () => {
		const responseFirst = harness(async (_scope, request) =>
			outcome(request, {
				status: "changed",
				captureMode: captureMode(2, true),
				captureModeEventSequence: 20,
			}),
		);
		await responseFirst.writer.enter("response-first");
		expect(() =>
			responseFirst.captureModeStore.applyProjection(
				captureMode(2, true),
				20,
			),
		).not.toThrow();
		expect(responseFirst.repair.captureMode).not.toHaveBeenCalled();

		const pending = deferred<ProgrammerPreloadLifecycleOutcome>();
		const eventFirst = harness(() => pending.promise);
		const entered = eventFirst.writer.enter("event-first");
		await Promise.resolve();
		eventFirst.captureModeStore.applyProjection(captureMode(2, true), 20);
		pending.resolve(
			outcome(eventFirst.apply.mock.calls[0][1], {
				status: "changed",
				captureMode: captureMode(2, true),
				captureModeEventSequence: 20,
			}),
		);
		await entered;
		expect(eventFirst.repair.captureMode).not.toHaveBeenCalled();
	});

	it("repairs response-first lifecycle state and accepts event-first state", async () => {
		let responseFirst!: ReturnType<typeof harness>;
		responseFirst = harness(async (_scope, request) =>
			outcome(request, {
				status: "changed",
				active: false,
				captureMode: captureMode(2),
				captureModeEventSequence: 20,
			}),
			{ active: true },
		);
		responseFirst.repair.lifecycle.mockImplementation(async () => {
			responseFirst.setActive(false);
		});

		await expect(
			responseFirst.writer.release("lifecycle-response-first"),
		).resolves.toMatchObject({ active: false });
		expect(responseFirst.repair.lifecycle).toHaveBeenCalledOnce();
		expect(responseFirst.readActive()).toBe(false);

		const pending = deferred<ProgrammerPreloadLifecycleOutcome>();
		const eventFirst = harness(() => pending.promise, { active: true });
		const released = eventFirst.writer.release("lifecycle-event-first");
		await Promise.resolve();
		eventFirst.setActive(false);
		pending.resolve(
			outcome(eventFirst.apply.mock.calls[0][1], {
				status: "changed",
				active: false,
				captureMode: captureMode(2),
				captureModeEventSequence: 20,
			}),
		);

		await expect(released).resolves.toMatchObject({ active: false });
		expect(eventFirst.repair.lifecycle).not.toHaveBeenCalled();
	});

	it("queues GO followed by Release and captures post-GO revisions", async () => {
		const pendingGo = deferred<ProgrammerPreloadLifecycleOutcome>();
		let setup!: ReturnType<typeof harness>;
		setup = harness(async (_scope, request) => {
			if (request.action.type === "go") return pendingGo.promise;
			setup.setActive(false);
			return outcome(request, {
				status: "changed",
				captureMode: captureMode(3),
				captureModeEventSequence: 34,
				valuesRevision: 2,
				queueRevision: 3,
			});
		}, { blind: true });
		const go = setup.writer.go("go-request");
		const release = setup.writer.release("release-request");
		await Promise.resolve();
		expect(setup.apply).toHaveBeenCalledOnce();
		setup.setActive(true);
		pendingGo.resolve(goOutcome(setup.apply.mock.calls[0][1]));

		await expect(go).resolves.toMatchObject({ active: true });
		await expect(release).resolves.toMatchObject({ active: false });
		expect(setup.apply.mock.calls.map(([, value]) => value.action.type)).toEqual([
			"go",
			"release",
		]);
		expect(setup.apply.mock.calls[1][1]).toMatchObject({
			expectedCaptureModeRevision: 2,
			expectedValuesRevision: 2,
			expectedQueueRevision: 3,
		});
	});

	it("keeps a sparse no-change outcome allocation-free", async () => {
		const setup = harness();
		const valuesProjection = setup.valuesStore.getSnapshot().projection;
		const queueProjection = setup.queueStore.getSnapshot().projection;

		await expect(setup.writer.release("same")).resolves.toMatchObject({
			status: "no_change",
		});
		expect(setup.valuesStore.getSnapshot().projection).toBe(valuesProjection);
		expect(setup.queueStore.getSnapshot().projection).toBe(queueProjection);
		expect(Object.values(setup.repair).every((mock) => mock.mock.calls.length === 0)).toBe(true);
	});
});
