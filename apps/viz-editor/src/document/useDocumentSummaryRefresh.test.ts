import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentSummary } from "./session";
import { useDocumentSummaryRefresh } from "./useDocumentSummaryRefresh";

const read = vi.hoisted(() => vi.fn());
vi.mock("./session", () => ({ documentSession: { current: read } }));

const summary = (showId = "show-a", fixtureCount = 313): DocumentSummary => ({
	showId, fixtureCount, universeCount: 10, lastSavedAt: 1000, name: showId,
	path: "/shows/test.show", fileName: "test.show", project: "", lightingDesigner: "",
	venue: "", contactEmail: "", contactPhone: "", showDate: "", showVersion: "",
});
function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
	return { promise, resolve, reject };
}
function mount(visible = true) {
	const onSummary = vi.fn();
	const onError = vi.fn();
	const props = { visible, document: summary(), sceneRevision: null as number | null, onSummary, onError };
	const view = renderHook((next) => useDocumentSummaryRefresh(next), { initialProps: props });
	return { ...view, props, onSummary, onError };
}

beforeEach(() => read.mockReset());
afterEach(() => vi.restoreAllMocks());

describe("focused authoritative Show summary reads", () => {
	it("does not poll or duplicate reads before a matching scene is available", async () => {
		const intervals = vi.spyOn(window, "setInterval");
		const view = mount();
		view.rerender({ ...view.props });
		await act(async () => {});
		expect(read).not.toHaveBeenCalled();
		expect(intervals).not.toHaveBeenCalled();
		expect(view.onSummary).not.toHaveBeenCalled();
	});

	it("validates the first matching scene once, including a new show with the same revision", async () => {
		read.mockResolvedValueOnce(summary("show-a", 314)).mockResolvedValueOnce(summary("show-b", 12));
		const view = mount();
		await act(async () => view.rerender({ ...view.props, sceneRevision: 2 }));
		view.rerender({ ...view.props, sceneRevision: 2 });
		expect(read).toHaveBeenCalledTimes(1);
		view.rerender({ ...view.props, document: summary("show-b", 11), sceneRevision: null });
		expect(read).toHaveBeenCalledTimes(1);
		await act(async () => view.rerender({ ...view.props, document: summary("show-b", 11), sceneRevision: 2 }));
		expect(read).toHaveBeenCalledTimes(2);
		expect(view.onSummary.mock.calls).toEqual([[summary("show-a", 314)], [summary("show-b", 12)]]);
	});

	it("validates an already matching initial scene without rereading on its own response", async () => {
		read.mockResolvedValue(summary("show-a", 314));
		const onSummary = vi.fn();
		const props = { visible: true, document: summary(), sceneRevision: 2, onSummary, onError: vi.fn() };
		const view = renderHook((next) => useDocumentSummaryRefresh(next), { initialProps: props });
		await act(async () => {});
		view.rerender({ ...props, document: summary("show-a", 314) });
		await act(async () => {});
		expect(read).toHaveBeenCalledTimes(1);
		expect(onSummary).toHaveBeenCalledWith(summary("show-a", 314));
	});

	it("defers hidden edits and reads fresh metadata on Show entry", async () => {
		read.mockResolvedValue(summary("show-a", 314));
		const view = mount(false);
		view.rerender({ ...view.props, sceneRevision: 2 });
		expect(read).not.toHaveBeenCalled();
		await act(async () => view.rerender({ ...view.props, visible: true, sceneRevision: 2 }));
		expect(read).toHaveBeenCalledTimes(1);
		expect(view.onSummary).toHaveBeenCalledWith(summary("show-a", 314));
	});

	it("coalesces edits during one read into one followup and discards its stale answer", async () => {
		const first = deferred<DocumentSummary>();
		read.mockReturnValueOnce(first.promise).mockResolvedValueOnce(summary("show-a", 316));
		const view = mount();
		view.rerender({ ...view.props, sceneRevision: 2 });
		view.rerender({ ...view.props, sceneRevision: 3 });
		view.rerender({ ...view.props, sceneRevision: 4 });
		expect(read).toHaveBeenCalledTimes(1);
		await act(async () => first.resolve(summary("show-a", 314)));
		expect(read).toHaveBeenCalledTimes(2);
		expect(view.onSummary.mock.calls).toEqual([[summary("show-a", 316)]]);
	});

	it("ignores a late old-show answer after a new show has become visible", async () => {
		const old = deferred<DocumentSummary>();
		read.mockReturnValueOnce(old.promise).mockResolvedValueOnce(summary("show-b", 12));
		const view = mount();
		view.rerender({ ...view.props, sceneRevision: 2 });
		view.rerender({ ...view.props, document: summary("show-b", 11) });
		await act(async () => view.rerender({ ...view.props, document: summary("show-b", 11), sceneRevision: 2 }));
		await act(async () => old.resolve(summary("show-a", 314)));
		expect(view.onSummary.mock.calls).toEqual([[summary("show-b", 12)]]);
		expect(view.onError).not.toHaveBeenCalled();
	});

	it("does not overwrite a newer same-show summary from another authoritative refresh", async () => {
		const pending = deferred<DocumentSummary>();
		read.mockReturnValue(pending.promise);
		const view = mount();
		view.rerender({ ...view.props, sceneRevision: 2 });
		view.rerender({ ...view.props, sceneRevision: 2, document: { ...summary("show-a", 315), project: "Saved in another window" } });
		await act(async () => pending.resolve(summary("show-a", 314)));
		expect(view.onSummary).not.toHaveBeenCalled();
	});

	it("does not replace the active document with a mismatched summary", async () => {
		read.mockResolvedValue(summary("show-b"));
		const view = mount();
		await act(async () => view.rerender({ ...view.props, sceneRevision: 2 }));
		expect(view.onSummary).not.toHaveBeenCalled();
	});

	it("reports a focused read failure once and retries only on another relevant event", async () => {
		const failure = new Error("summary unavailable");
		read.mockRejectedValueOnce(failure).mockResolvedValueOnce(summary("show-a", 314));
		const view = mount();
		await act(async () => view.rerender({ ...view.props, sceneRevision: 2 }));
		expect(view.onError.mock.calls).toEqual([[failure]]);
		expect(view.onSummary).not.toHaveBeenCalled();
		await act(async () => view.rerender({ ...view.props, sceneRevision: 3 }));
		expect(read).toHaveBeenCalledTimes(2);
		expect(view.onSummary).toHaveBeenCalledWith(summary("show-a", 314));
	});

	it("suppresses hidden/unmounted answers and does not drain obsolete pending edits", async () => {
		for (const unmount of [false, true]) {
			const pending = deferred<DocumentSummary>();
			read.mockReset().mockReturnValue(pending.promise);
			const view = mount();
			view.rerender({ ...view.props, sceneRevision: 2 });
			view.rerender({ ...view.props, sceneRevision: 3 });
			if (unmount) view.unmount();
			else view.rerender({ ...view.props, visible: false, sceneRevision: 3 });
			await act(async () => pending.resolve(summary("show-a", 314)));
			expect(read).toHaveBeenCalledTimes(1);
			expect(view.onSummary).not.toHaveBeenCalled();
			expect(view.onError).not.toHaveBeenCalled();
			view.unmount();
		}
	});
});
