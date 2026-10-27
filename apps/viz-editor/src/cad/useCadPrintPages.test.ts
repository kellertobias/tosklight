import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { underlaysForPage } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";
import { restorePrintPages, useCadPrintPages } from "./useCadPrintPages";
import { act, cleanup, render, renderHook, waitFor } from "@testing-library/react";
import { printPageSession, type PrintPageSnapshot } from "./printPages";
import type { CadPrintPage, CadSceneSnapshot } from "./types";
import { createElement } from "react";
import { PrintFrame } from "./CadPrintFrame";
import { buildCadPdf } from "./print";
import { PT_MM, printPageScale, withFixedPrintScale } from "./printLayout";

const PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v2";
const LEGACY_PRINT_KEY = "tosklight:viz-editor:cad-print-pages:v1";

/** A page as an older version of the Architect wrote it: no placed drawings existed yet. */
const storedPage = {
	kind: "plan",
	id: "page-1",
	tileId: "tile-1",
	name: "Page 1",
	view: "top_down",
	rotationQuarterTurns: 0,
	centreMillimetres: [0, 0],
	widthMillimetres: 5000,
	included: true,
	orientation: "landscape",
	showFixtureIds: true,
	showDmxAddresses: false,
};

const underlay = {
	id: "venue",
	name: "Ground plan.dxf",
	view: "top_down",
	visible: true,
} as CadUnderlay;

const workspace = new Map<string, string>();

describe("print pages a previous session saved", () => {
	beforeEach(() => {
		workspace.clear();
		vi.stubGlobal("localStorage", {
			getItem: (key: string) => workspace.get(key) ?? null,
			setItem: (key: string, value: string) => workspace.set(key, value),
			removeItem: (key: string) => workspace.delete(key),
			clear: () => workspace.clear(),
		});
	});

	it("prints every drawing on its axis when the page predates them", () => {
		localStorage.setItem(PRINT_KEY, JSON.stringify([storedPage]));
		const [page] = restorePrintPages();
		expect(page.hiddenUnderlayIds).toBeUndefined();
		expect(underlaysForPage([underlay], page).map((one) => one.id)).toEqual([
			"venue",
		]);
	});

	it("keeps the drawings a page had switched off", () => {
		localStorage.setItem(
			PRINT_KEY,
			JSON.stringify([{ ...storedPage, hiddenUnderlayIds: ["venue"] }]),
		);
		const [page] = restorePrintPages();
		expect(page.hiddenUnderlayIds).toEqual(["venue"]);
		expect(underlaysForPage([underlay], page)).toEqual([]);
	});

	it("still reads a v1 page, mirroring its top-down centre once", () => {
		localStorage.setItem(
			LEGACY_PRINT_KEY,
			JSON.stringify([{ ...storedPage, centreMillimetres: [1000, 2000] }]),
		);
		const [page] = restorePrintPages();
		expect(page.centreMillimetres).not.toEqual([1000, 2000]);
		expect(page.kind).toBe("plan");
	});

	it("drops a stored value that is not a page at all", () => {
		localStorage.setItem(PRINT_KEY, JSON.stringify([{ id: "broken" }, "nope"]));
		expect(restorePrintPages()).toEqual([]);
	});
});

// The transport is mocked here; the native tests save and reopen actual .show files.
vi.mock("./printPages", () => ({
	printPageSession: { all: vi.fn(), save: vi.fn(), onDelta: vi.fn() },
}));

const native = new Map<string, CadPrintPage[]>();
const revisions = new Map<string, number>();
const listeners = new Set<(snapshot: PrintPageSnapshot) => void>();
function snapshot(showId: string): PrintPageSnapshot {
	return { showId, pages: native.get(showId) ?? null, revision: revisions.get(showId) ?? (native.has(showId) ? 1 : 0) };
}
const fullPage: CadPrintPage = {
	...storedPage,
	kind: "plan",
	view: "top_down",
	orientation: "landscape",
	centreMillimetres: [0, 0],
	name: "Ground Plan · Südbahnhof",
	rotationQuarterTurns: 3,
	showMountingHardware: false,
	cutPlanes: { nearMillimetres: -8000, farMillimetres: -4000 },
	hiddenUnderlayIds: ["section"],
};

describe("portable show-owned print pages", () => {
	beforeEach(() => {
		workspace.clear();
		native.clear();
		revisions.clear();
		listeners.clear();
		vi.clearAllMocks();
		vi.stubGlobal("localStorage", {
			getItem: (key: string) => workspace.get(key) ?? null,
			setItem: (key: string, value: string) => workspace.set(key, value),
			removeItem: (key: string) => workspace.delete(key),
		});
		vi.mocked(printPageSession.onDelta).mockImplementation(async (handler) => {
			listeners.add(handler); return () => { listeners.delete(handler); };
		});
		vi.mocked(printPageSession.all).mockImplementation(async (id) => snapshot(id));
		vi.mocked(printPageSession.save).mockImplementation(async (id, pages, onlyIfAbsent, expectedRevision) => {
			if (onlyIfAbsent && native.has(id)) return snapshot(id);
			const current = snapshot(id);
			if (current.revision !== expectedRevision) throw new Error("Another window changed these print pages. Retry to merge your page edits with the latest show.");
			native.set(id, structuredClone(pages));
			revisions.set(id, current.revision + 1);
			const next = snapshot(id);
			for (const handler of listeners) handler(next);
			return next;
		});
	});
	afterEach(() => cleanup());

	it("adopts six workspace pages once and keeps them out of a second show", async () => {
		const legacy = Array.from({ length: 6 }, (_, index) => ({ ...fullPage, id: `page-${index}` }));
		workspace.set(PRINT_KEY, JSON.stringify(legacy));
		const { result, rerender } = renderHook(({ id }) => useCadPrintPages(id), { initialProps: { id: "first" } });
		await waitFor(() => expect(result.current.pages).toEqual(legacy));
		expect(workspace.has(PRINT_KEY)).toBe(false);
		rerender({ id: "second" });
		expect(result.current.pages).toEqual([]);
		await waitFor(() => expect(native.get("second")).toEqual([]));
		rerender({ id: "first" });
		await waitFor(() => expect(result.current.pages).toEqual(legacy));
		expect(printPageSession.save).toHaveBeenCalledTimes(2);
	});

	it("converts legacy v1 coordinates once before native save and reopen", async () => {
		workspace.set(LEGACY_PRINT_KEY, JSON.stringify([{ ...fullPage, centreMillimetres: [1000, 2000] }]));
		const expected = restorePrintPages();
		const { result, unmount } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.pages).toEqual(expected));
		expect(workspace.has(LEGACY_PRINT_KEY)).toBe(false);
		unmount();
		const reopened = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(reopened.result.current.pages).toEqual(expected));
		expect(printPageSession.save).toHaveBeenCalledTimes(1);
	});

	it("retains the latest unsaved page and retries a failed native write", async () => {
		native.set("first", [fullPage]);
		const { result } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.pages).toHaveLength(1));
		vi.mocked(printPageSession.save).mockRejectedValueOnce(new Error("disk full"));
		act(() => result.current.change(fullPage.id, { name: "New title" }));
		await waitFor(() => expect(result.current.error).toContain("disk full"));
		expect(result.current.pages[0].name).toBe("New title");
		expect(native.get("first")?.[0].name).toBe(fullPage.name);
		act(() => result.current.retry());
		await waitFor(() => expect(result.current.busy).toBe(false));
		expect(native.get("first")?.[0].name).toBe("New title");
		expect(result.current.error).toBeNull();
	});

	it("persists title, framing and page switches when reopened", async () => {
		native.set("first", [fullPage]);
		const { result, unmount } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.pages).toHaveLength(1));
		act(() => result.current.change(fullPage.id, { name: "Section B", centreMillimetres: [50, 90], included: false }));
		await waitFor(() => expect(result.current.busy).toBe(false));
		const expected = { ...fullPage, name: "Section B", centreMillimetres: [50, 90], included: false };
		expect(native.get("first")).toEqual([expected]);
		unmount();
		const reopened = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(reopened.result.current.pages).toEqual([expected]));
	});

	it("deletes pages with next, previous and empty selection, and does not resurrect the last page", async () => {
		const pages = ["a", "b", "c"].map((id) => ({ ...fullPage, id }));
		native.set("first", pages);
		const { result, unmount } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.ready).toBe(true));
		act(() => result.current.select("b"));
		act(() => result.current.remove("b"));
		expect(result.current.selectedId).toBe("c");
		act(() => result.current.remove("c"));
		expect(result.current.selectedId).toBe("a");
		act(() => result.current.remove("a"));
		expect(result.current.selectedId).toBeNull();
		await waitFor(() => expect(result.current.busy).toBe(false));
		expect(native.get("first")).toEqual([]);
		unmount();
		const reopened = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(reopened.result.current.ready).toBe(true));
		expect(reopened.result.current.pages).toEqual([]);
	});

	it("persists an exact fixed scale, retaining it through orientation and frame width edits", async () => {
		native.set("first", [fullPage]);
		const { result, unmount } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.ready).toBe(true));
		act(() => result.current.change(fullPage.id, withFixedPrintScale(fullPage, 75)));
		expect(printPageScale(result.current.pages[0])).toBeCloseTo(1 / 75, 12);
		act(() => result.current.rotate(fullPage.id));
		expect(result.current.pages[0].orientation).toBe("portrait");
		act(() => result.current.change(fullPage.id, { widthMillimetres: 999 }));
		expect(printPageScale(result.current.pages[0])).toBeCloseTo(1 / 75, 12);
		await waitFor(() => expect(result.current.busy).toBe(false));
		unmount();
		const reopened = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(reopened.result.current.ready).toBe(true));
		expect(reopened.result.current.pages[0].fixedScaleDenominator).toBe(75);
		expect(printPageScale(reopened.result.current.pages[0])).toBeCloseTo(1 / 75, 12);
	});

	it("normalizes inconsistent stored fixed-scale width for both frame and PDF geometry", async () => {
		const stored = { ...fullPage, rotationQuarterTurns: 0, fixedScaleDenominator: 75, widthMillimetres: 5000 };
		native.set("first", [stored]);
		const { result } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.ready).toBe(true));
		const loaded = result.current.pages[0];
		expect(loaded.widthMillimetres).toBe(withFixedPrintScale(stored, 75).widthMillimetres);
		expect(printPageScale(loaded)).toBeCloseTo(1 / 75, 12);
		render(createElement(PrintFrame, {
			page: loaded, entities: [], underlays: [], camera: { pan: [0, 0], zoom: 0.1 },
			selected: true, onSelect: vi.fn(), onChange: vi.fn(), onCamera: vi.fn(),
		}));
		expect((document.querySelector(".cad-print-frame") as HTMLElement).style.width).toBe(`${loaded.widthMillimetres * 0.1}px`);
		const scene: CadSceneSnapshot = { showId: "first", sceneRevision: 1, selectionRevision: 1, selectedIds: [], attachments: [], drawings: [], entities: [] };
		const measuringLine: CadUnderlay = {
			id: "measure", name: "7500 mm reference", sourceFormat: "dxf", view: "top_down", visible: true,
			originMillimetres: [0, 0], scale: 1, rotationDegrees: 0, units: "mm",
			geometry: { units: "mm", extentsMillimetres: [0, 0, 7500, 0], polylines: [{ points: [[0, 0], [7500, 0]], closed: false, layer: "reference" }] },
		};
		const pdf = new TextDecoder().decode(buildCadPdf(scene, [loaded], undefined, [measuringLine]));
		expect(pdf).toContain("1:75");
		// 7500 world millimetres at 1:75 must occupy 100 physical millimetres in the PDF.
		const line = pdf.split("\n").find((command) => command.match(/^-?[\d.]+ -?[\d.]+ m -?[\d.]+ -?[\d.]+ l S$/) && Math.abs(Number(command.split(" ")[3]) - Number(command.split(" ")[0]) - 100 * PT_MM) < 0.02);
		expect(line).toBeDefined();
	});

	it("coalesces pointer updates and preserves changes made in the same React batch", async () => {
		native.set("first", [fullPage]);
		const { result } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.pages).toHaveLength(1));
		act(() => {
			for (let x = 0; x < 100; x++) result.current.change(fullPage.id, { centreMillimetres: [x, 0] });
			result.current.change(fullPage.id, { name: "Last drag" });
		});
		await waitFor(() => expect(result.current.busy).toBe(false));
		expect(printPageSession.save).toHaveBeenCalledTimes(1);
		expect(native.get("first")?.[0]).toMatchObject({ name: "Last drag", centreMillimetres: [99, 0] });
	});

	it("serializes slow native writes and saves the newest queued frame", async () => {
		native.set("first", [fullPage]);
		let finish!: (pages: PrintPageSnapshot) => void;
		vi.mocked(printPageSession.save).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
		const { result } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.pages).toHaveLength(1));
		act(() => result.current.change(fullPage.id, { name: "First edit" }));
		await waitFor(() => expect(printPageSession.save).toHaveBeenCalledTimes(1));
		act(() => {
			result.current.change(fullPage.id, { name: "Intermediate" });
			result.current.change(fullPage.id, { name: "Newest edit" });
		});
		expect(printPageSession.save).toHaveBeenCalledTimes(1);
		await act(async () => finish({ showId: "first", revision: 1, pages: [{ ...fullPage, name: "First edit" }] }));
		await waitFor(() => expect(result.current.busy).toBe(false));
		expect(printPageSession.save).toHaveBeenCalledTimes(2);
		expect(native.get("first")?.[0].name).toBe("Newest edit");
	});

	it("keeps intentionally empty show pages instead of resurrecting workspace data", async () => {
		workspace.set(PRINT_KEY, JSON.stringify([fullPage]));
		native.set("first", []);
		const { result } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.busy).toBe(false));
		expect(result.current.pages).toEqual([]);
		expect(printPageSession.save).not.toHaveBeenCalled();
	});

	it("reports failed adoption and reserves legacy pages for the original show", async () => {
		workspace.set(PRINT_KEY, JSON.stringify([fullPage]));
		vi.mocked(printPageSession.save).mockRejectedValueOnce(new Error("disk full"));
		const { result, rerender } = renderHook(({ id }) => useCadPrintPages(id), { initialProps: { id: "first" } });
		await waitFor(() => expect(result.current.error).toContain("disk full"));
		expect(result.current.ready).toBe(false);
		expect(workspace.has(PRINT_KEY)).toBe(true);
		rerender({ id: "second" });
		await waitFor(() => expect(native.get("second")).toEqual([]));
		expect(workspace.has(PRINT_KEY)).toBe(true);
		rerender({ id: "first" });
		await waitFor(() => expect(result.current.pages).toEqual([fullPage]));
	});

	it("refreshes an idle second window from authoritative native page events", async () => {
		native.set("first", [fullPage]);
		const first = renderHook(() => useCadPrintPages("first"));
		const second = renderHook(() => useCadPrintPages("first"));
		expect(first.result.current.ready).toBe(false);
		await waitFor(() => expect(first.result.current.ready && second.result.current.ready).toBe(true));
		act(() => first.result.current.change(fullPage.id, { name: "Shared title" }));
		expect(first.result.current.ready).toBe(true);
		await waitFor(() => expect(second.result.current.pages[0].name).toBe("Shared title"));
		expect(second.result.current.busy).toBe(false);
		expect(printPageSession.save).toHaveBeenCalledTimes(1);
	});

	it("ignores page events from a different open show", async () => {
		native.set("first", [fullPage]);
		const { result } = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(result.current.ready).toBe(true));
		act(() => {
			for (const handler of listeners) handler({ showId: "second", revision: 99, pages: [{ ...fullPage, name: "Wrong show" }] });
		});
		expect(result.current.pages[0].name).toBe(fullPage.name);
	});

	it("rejects concurrent stale saves and Retry rebases only this window's fields", async () => {
		native.set("first", [fullPage]);
		const first = renderHook(() => useCadPrintPages("first"));
		const second = renderHook(() => useCadPrintPages("first"));
		await waitFor(() => expect(first.result.current.ready && second.result.current.ready).toBe(true));
		act(() => {
			first.result.current.change(fullPage.id, { name: "Other window title" });
			second.result.current.change(fullPage.id, { centreMillimetres: [400, 900] });
		});
		await waitFor(() => expect(second.result.current.error).toContain("Another window"));
		expect(native.get("first")?.[0].name).toBe("Other window title");
		expect(native.get("first")?.[0].centreMillimetres).toEqual(fullPage.centreMillimetres);
		expect(second.result.current.pages[0].centreMillimetres).toEqual([400, 900]);
		act(() => second.result.current.retry());
		await waitFor(() => expect(second.result.current.busy).toBe(false));
		expect(native.get("first")?.[0]).toMatchObject({ name: "Other window title", centreMillimetres: [400, 900] });
		expect(first.result.current.pages[0].centreMillimetres).toEqual([400, 900]);
		expect(second.result.current.error).toBeNull();
	});

	it("does not attach a late response from the previous show to the next show", async () => {
		let finish!: (pages: PrintPageSnapshot) => void;
		vi.mocked(printPageSession.all).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
		const { result, rerender } = renderHook(({ id }) => useCadPrintPages(id), { initialProps: { id: "first" } });
		await waitFor(() => expect(printPageSession.all).toHaveBeenCalledWith("first"));
		rerender({ id: "second" });
		await waitFor(() => expect(native.get("second")).toEqual([]));
		await act(async () => finish({ showId: "first", revision: 1, pages: [fullPage] }));
		expect(result.current.pages).toEqual([]);
		expect(native.has("first")).toBe(false);
	});
});
