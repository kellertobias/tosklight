import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { usePaperworkAutosave } from "./usePaperworkAutosave";
import { documentSession, type DocumentSummary } from "./session";

vi.mock("./session", () => ({ documentSession: { savePaperwork: vi.fn() } }));
const first = { showId: "first", project: "Original", name: "First" } as DocumentSummary;
const second = { showId: "second", project: "Second", name: "Second" } as DocumentSummary;
const save = vi.mocked(documentSession.savePaperwork);
function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
	return { promise, resolve, reject };
}
async function debounce() { await act(async () => { await vi.advanceTimersByTimeAsync(350); }); }
beforeEach(() => {
	vi.useFakeTimers();
	save.mockReset().mockImplementation(async (draft, showId) => ({ ...first, ...draft, showId }));
});
afterEach(() => vi.useRealTimers());

describe("native project information autosave", () => {
	it("debounces the latest fields and sends the active show identity", async () => {
		const onDocument = vi.fn();
		const { result } = renderHook(() => usePaperworkAutosave(first, onDocument));
		act(() => { result.current.change("project", "Tour"); result.current.change("venue", "Hall"); });
		expect(save).not.toHaveBeenCalled();
		expect(result.current.saving).toBe(true);
		await debounce();
		expect(save).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ project: "Tour", venue: "Hall" }), "first");
		expect(result.current.saving).toBe(false);
		expect(onDocument).toHaveBeenCalledOnce();
	});

	it("serializes saves and keeps edits typed while an older save is in flight", async () => {
		const older = deferred<DocumentSummary>();
		save.mockImplementationOnce(() => older.promise);
		const { result } = renderHook(() => usePaperworkAutosave(first, vi.fn()));
		act(() => result.current.change("project", "Earlier"));
		await debounce();
		act(() => result.current.change("project", "Latest"));
		await debounce();
		expect(save).toHaveBeenCalledTimes(1);
		await act(async () => older.resolve({ ...first, project: "Earlier" }));
		expect(save).toHaveBeenCalledTimes(2);
		expect(save.mock.calls[1][0].project).toBe("Latest");
		expect(result.current.draft.project).toBe("Latest");
	});

	it("keeps failed edits visible, then retries the latest draft", async () => {
		save.mockRejectedValueOnce(new Error("Disk full"));
		const { result } = renderHook(() => usePaperworkAutosave(first, vi.fn()));
		act(() => result.current.change("project", "Tour"));
		await debounce();
		expect(result.current.error).toContain("Disk full");
		act(() => result.current.change("contactEmail", "desk@example.com"));
		expect(result.current.error).toContain("Disk full");
		await act(async () => result.current.retry());
		expect(save.mock.calls[1][0]).toEqual(expect.objectContaining({ project: "Tour", contactEmail: "desk@example.com" }));
		expect(result.current.error).toBeNull();
	});

	it("guards a flushed old draft and ignores its late response after switching shows", async () => {
		const old = deferred<DocumentSummary>();
		save.mockImplementationOnce(() => old.promise);
		const onDocument = vi.fn();
		const { result, rerender } = renderHook(({ document }) => usePaperworkAutosave(document, onDocument), { initialProps: { document: first } });
		act(() => result.current.change("project", "Old pending"));
		rerender({ document: second });
		expect(save.mock.calls[0][1]).toBe("first");
		expect(result.current.draft.project).toBe("Second");
		await act(async () => old.resolve({ ...first, project: "Old pending" }));
		expect(onDocument).not.toHaveBeenCalled();
		expect(result.current.draft.project).toBe("Second");
	});

	it("flushes a final edit on unmount and preserves a dirty form across rig summary updates", () => {
		const { result, rerender, unmount } = renderHook(({ document }) => usePaperworkAutosave(document, vi.fn()), { initialProps: { document: first } });
		act(() => result.current.change("project", "Final edit"));
		rerender({ document: { ...first, fixtureCount: 200 } });
		expect(result.current.draft.project).toBe("Final edit");
		unmount();
		expect(save).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ project: "Final edit" }), "first");
	});
});
