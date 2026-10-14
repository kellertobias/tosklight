import { describe, expect, it, vi } from "vitest";
import { ApiRequestError } from "../../api/ApiRequestError";
import {
	deferred,
	lifecycleWriterHarness,
	OTHER_ID,
	outcome,
	SHOW_ID,
} from "../programmerPreloadLifecycle/writerTestHarness";
import { createLayoutActions } from "./layouts";
import type { ServerController } from "./model";

type ShowObjectActionOutcome = Awaited<
	ReturnType<ServerController["api"]["showObjects"]["updateUserLayout"]>
>;

const layout = { desks: [], activeDeskId: "main" };
function receipt(revision = 5): ShowObjectActionOutcome {
	return {
		request_id: "save-layout",
		replayed: false,
		show_id: SHOW_ID,
		show_revision: revision,
		object: {
			kind: "user_layout",
			id: "desk",
			revision: 1,
			updated_at: "2026-10-14T00:00:00Z",
			body: layout,
		},
	};
}
function harness() {
	const preload = lifecycleWriterHarness(async (_scope, request) =>
		outcome(request),
	);
	const updateUserLayout = vi.fn().mockResolvedValue(receipt());
	const objectOrNull = vi
		.fn()
		.mockResolvedValue({ id: "desk", revision: 2, body: layout });
	const setDeskLayout = vi.fn();
	const setError = vi.fn();
	const model = {
		api: { showObjects: { updateUserLayout, objectOrNull } },
		bootstrap: { active_show: { id: SHOW_ID } },
		session: { token: "session-a" },
		deskLayout: null,
		showObjectsStore: preload.showStore,
		setDeskLayout,
		setError,
	} as unknown as ServerController;
	return {
		...preload,
		updateUserLayout,
		objectOrNull,
		setDeskLayout,
		setError,
		actions: createLayoutActions(model),
	};
}
describe("Desktop layout receipt authority", () => {
	it("actual saveDeskLayout advances shared authority before production GO without a mounted layout stream", async () => {
		const h = harness();
		await h.actions.saveDeskLayout(layout);
		await h.writer.go("real-save-go");
		expect(h.showStore.getSnapshot().showRevision).toBe(5);
		expect(h.runtimeStore.getSnapshot().showRevision).toBe(4);
		expect(h.apply.mock.calls[0][1].action).toMatchObject({
			expectedShowRevision: 5,
			expectedPlaybackEventSequence: 30,
		});
	});
	it.each([
		"show",
		"kind",
		"id",
	])("refuses a mismatched %s receipt", async (field) => {
		const h = harness();
		const wrong = receipt();
		if (field === "show") wrong.show_id = OTHER_ID;
		if (field === "kind") wrong.object.kind = "cue_list";
		if (field === "id") wrong.object.id = "other";
		h.updateUserLayout.mockResolvedValue(wrong);
		await h.actions.saveDeskLayout(layout);
		expect(h.showStore.getSnapshot().showRevision).toBe(4);
		expect(h.setDeskLayout).not.toHaveBeenCalled();
		expect(h.setError).toHaveBeenCalledWith(expect.any(String));
	});
	it.each([
		"A-B-A",
		"session reset",
	])("retires a late receipt after %s", async (change) => {
		const h = harness();
		const pending = deferred<ShowObjectActionOutcome>();
		h.updateUserLayout.mockReturnValue(pending.promise);
		const saved = h.actions.saveDeskLayout(layout);
		if (change === "A-B-A") h.showStore.reset(OTHER_ID, "session-b");
		h.showStore.reset(SHOW_ID, "session-new");
		h.showStore.installShowRevision(SHOW_ID, 8);
		pending.resolve(receipt());
		await saved;
		expect(h.showStore.getSnapshot().showRevision).toBe(8);
		expect(h.setDeskLayout).not.toHaveBeenCalled();
		expect(h.setError).not.toHaveBeenCalled();
	});
	it("keeps a newer event before a late receipt", async () => {
		const h = harness();
		const pending = deferred<ShowObjectActionOutcome>();
		h.updateUserLayout.mockReturnValue(pending.promise);
		const saved = h.actions.saveDeskLayout(layout);
		h.showStore.installShowRevision(SHOW_ID, 8);
		pending.resolve(receipt());
		await saved;
		expect(h.showStore.getSnapshot().showRevision).toBe(8);
		await h.writer.go("newer-event");
		expect(h.apply.mock.calls[0][1].action).toMatchObject({
			expectedShowRevision: 8,
		});
	});
	it("adopts only the successful final 409 retry receipt", async () => {
		const h = harness();
		h.updateUserLayout
			.mockRejectedValueOnce(new ApiRequestError("conflict", 409))
			.mockResolvedValueOnce(receipt(7));
		await h.actions.saveDeskLayout(layout);
		expect(h.updateUserLayout).toHaveBeenCalledTimes(2);
		expect(h.updateUserLayout.mock.calls[1][3]).toBe(2);
		expect(h.showStore.getSnapshot().showRevision).toBe(7);
	});
	it("does not retry a conflict after the read crosses an authority generation", async () => {
		const h = harness();
		const read = deferred<null>();
		h.updateUserLayout.mockRejectedValueOnce(
			new ApiRequestError("conflict", 409),
		);
		h.objectOrNull.mockReturnValue(read.promise);
		const saved = h.actions.saveDeskLayout(layout);
		await vi.waitFor(() => expect(h.objectOrNull).toHaveBeenCalledOnce());
		h.showStore.reset(SHOW_ID, "new-session");
		read.resolve(null);
		await saved;
		expect(h.updateUserLayout).toHaveBeenCalledOnce();
		expect(h.setDeskLayout).not.toHaveBeenCalled();
	});
	it("does not adopt authority when the final retry fails", async () => {
		const h = harness();
		h.updateUserLayout
			.mockRejectedValueOnce(new ApiRequestError("conflict", 409))
			.mockRejectedValueOnce(new Error("save failed"));
		await h.actions.saveDeskLayout(layout);
		expect(h.showStore.getSnapshot().showRevision).toBe(4);
		expect(h.setDeskLayout).not.toHaveBeenCalled();
		expect(h.setError).toHaveBeenCalledWith(
			expect.stringContaining("save failed"),
		);
	});
	it("a queued React update preserves newer layout data and retired authority", async () => {
		const h = harness();
		await h.actions.saveDeskLayout(layout);
		const setter = h.setDeskLayout.mock.calls[0][0];
		const newer = { ...receipt().object, revision: 3 };
		expect(setter(newer)).toBe(newer);
		h.showStore.reset(OTHER_ID, "other-session");
		const other = { ...receipt().object, id: "other" };
		expect(setter(other)).toBe(other);
	});
});
