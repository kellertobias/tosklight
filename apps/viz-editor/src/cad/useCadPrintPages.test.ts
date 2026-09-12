import { beforeEach, describe, expect, it, vi } from "vitest";
import { underlaysForPage } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";
import { restorePrintPages } from "./useCadPrintPages";

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
