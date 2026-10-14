import { describe, expect, it } from "vitest";
import { playbackSnapshot } from "../playbackRuntime/testFixtures";
import {
	lifecycleWriterHarness as harness,
	OTHER_ID,
	outcome,
	SHOW_ID,
} from "./writerTestHarness";

describe("Preload GO portable Show authority", () => {
	it("uses the current portable layout revision while retaining the runtime event cursor", async () => {
		const setup = harness(async (_scope, request) => outcome(request));
		expect(setup.runtimeStore.getSnapshot().showRevision).toBe(4);
		setup.showStore.installShowRevision(SHOW_ID, 5);
		await setup.writer.go("layout-then-go");
		expect(setup.apply).toHaveBeenCalledOnce();
		expect(setup.apply.mock.calls[0][1].action).toEqual({
			type: "go",
			showId: SHOW_ID,
			expectedShowRevision: 5,
			expectedPlaybackEventSequence: 30,
		});
		expect(setup.runtimeStore.getSnapshot().showRevision).toBe(4);
	});
	it("refuses GO before a portable Show revision is hydrated", async () => {
		const setup = harness(undefined, { showRevision: null });
		expect(setup.showStore.getSnapshot().showRevision).toBeNull();
		await expect(
			setup.writer.go("missing-portable-authority"),
		).resolves.toBeNull();
		expect(setup.apply).not.toHaveBeenCalled();
		expect(setup.queueStore.getSnapshot().projection?.actions).toHaveLength(1);
	});
	it("cannot adopt a foreign layout event's revision", async () => {
		const setup = harness(async (_scope, request) => outcome(request));
		setup.showStore.installShowRevision(SHOW_ID, 4);
		expect(setup.showStore.installShowRevision(OTHER_ID, 99)).toBe(false);
		await setup.writer.go("foreign-layout-event");
		expect(setup.apply.mock.calls[0][1].action).toMatchObject({
			expectedShowRevision: 4,
		});
	});

	it("refuses foreign current-show authority without changing the pending queue", async () => {
		const setup = harness();
		setup.showStore.reset(OTHER_ID, "session-b");
		await expect(setup.writer.go("foreign-show")).resolves.toBeNull();
		expect(setup.apply).not.toHaveBeenCalled();
		expect(setup.queueStore.getSnapshot().projection?.actions).toHaveLength(1);
	});
	it("keeps pending work and never retries an exact stale Show rejection", async () => {
		const setup = harness(
			async () => {
				throw new Error("active Show revision conflict: expected 4, actual 5");
			},
			{ blind: true },
		);
		const pending = setup.queueStore.getSnapshot().projection;
		await expect(setup.writer.go("stale-authority")).resolves.toBeNull();
		expect(setup.apply).toHaveBeenCalledOnce();
		expect(setup.apply.mock.calls[0][1].action).toMatchObject({
			expectedShowRevision: 4,
			expectedPlaybackEventSequence: 30,
		});
		expect(setup.queueStore.getSnapshot().projection).toEqual(pending);
		expect(setup.captureModeStore.getSnapshot().projection?.blind).toBe(true);
	});

	it("uses newer compiled runtime authority after an external edit outside scoped Show collections", async () => {
		const h = harness();
		const snapshot = playbackSnapshot([], 31);
		snapshot.desk.scope.show_revision = 6;
		h.runtimeStore.installSnapshot(snapshot, []);
		await h.writer.go("external-object-edit");
		expect(h.showStore.getSnapshot().showRevision).toBe(4);
		expect(h.apply.mock.calls[0][1].action).toMatchObject({
			expectedShowRevision: 6,
			expectedPlaybackEventSequence: 31,
		});
	});
});
