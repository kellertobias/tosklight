import { act, renderHook } from "@testing-library/react";
import { save } from "@tauri-apps/plugin-dialog";
import { beforeEach, expect, it, vi } from "vitest";
import { registerPendingDocumentSave } from "../document/pendingDocumentSaves";
import { useCadPdfExport } from "./useCadPdfExport";
import { cadSession } from "./session";
import type { CadPrintDocumentInfo } from "./print";
import type { CadPrintPage, CadSceneSnapshot } from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));
vi.mock("./session", () => ({ cadSession: { exportPdf: vi.fn() } }));
vi.mock("./printFonts", async (original) => ({
	...await original<typeof import("./printFonts")>(), loadPrintFonts: vi.fn(async () => undefined),
}));
const info: CadPrintDocumentInfo = {
	showName: "Tour", project: "Earlier project", lightingDesigner: "Designer", venue: "Hall",
	contactEmail: "", contactPhone: "", showDate: "", showVersion: "", lastSavedAt: 0,
	fixtureCount: 0, universeCount: 0,
};
const page: CadPrintPage = {
	kind: "plan", id: "page", tileId: "tile", name: "Ground plan", view: "top_down",
	rotationQuarterTurns: 0, centreMillimetres: [0, 0], widthMillimetres: 5000,
	orientation: "landscape", included: true, showFixtureIds: false, showDmxAddresses: false,
	showMountingHardware: true,
};
function options() {
	return {
		scene: { showId: "show", sceneRevision: 1, selectionRevision: 1, selectedIds: [], entities: [], drawings: [], attachments: [] } as CadSceneSnapshot,
		pages: [page], loadInfo: vi.fn(async () => info), underlays: [], annotations: [], blocked: false,
		onNotice: vi.fn(), onError: vi.fn(),
	};
}
beforeEach(() => {
	vi.mocked(save).mockReset().mockResolvedValue("/rig.pdf");
	vi.mocked(cadSession.exportPdf).mockReset().mockResolvedValue(undefined);
});

it("waits for pending project saves before reading metadata and building the exported PDF", async () => {
	let finish!: () => void;
	const pending = new Promise<void>((resolve) => { finish = resolve; });
	let savedInfo = info;
	const unregister = registerPendingDocumentSave(async () => {
		await pending;
		savedInfo = { ...info, project: "Latest saved project" };
	});
	try {
		const settings = options();
		settings.loadInfo.mockImplementation(async () => savedInfo);
		const { result } = renderHook(() => useCadPdfExport(settings));
		let exported!: Promise<void>;
		act(() => { exported = result.current.exportPdf(); });
		expect(result.current.exporting).toBe(true);
		expect(save).not.toHaveBeenCalled();
		expect(settings.loadInfo).not.toHaveBeenCalled();
		expect(cadSession.exportPdf).not.toHaveBeenCalled();
		await act(async () => { finish(); await exported; });
		const pdf = new TextDecoder().decode(vi.mocked(cadSession.exportPdf).mock.calls[0][1] as Uint8Array);
		expect(pdf).toContain("Latest saved project");
		expect(pdf).not.toContain("Earlier project");
		expect(settings.loadInfo).toHaveBeenCalledOnce();
		expect(result.current.exporting).toBe(false);
	} finally { unregister(); }
});

it("blocks PDF export when the project save fails and exposes its error", async () => {
	const unregister = registerPendingDocumentSave(async () => { throw new Error("Disk full"); });
	try {
		const settings = options();
		const { result } = renderHook(() => useCadPdfExport(settings));
		await act(async () => result.current.exportPdf());
		expect(settings.onError).toHaveBeenCalledWith(expect.stringContaining("Disk full"));
		expect(settings.loadInfo).not.toHaveBeenCalled();
		expect(save).not.toHaveBeenCalled();
		expect(cadSession.exportPdf).not.toHaveBeenCalled();
		expect(result.current.exporting).toBe(false);
	} finally { unregister(); }
});

it("clears export progress after cancelling or failing the native save dialog", async () => {
	const settings = options();
	const { result } = renderHook(() => useCadPdfExport(settings));
	vi.mocked(save).mockResolvedValueOnce(null);
	await act(async () => result.current.exportPdf());
	expect(result.current.exporting).toBe(false);
	expect(settings.loadInfo).not.toHaveBeenCalled();
	vi.mocked(save).mockRejectedValueOnce(new Error("Dialog unavailable"));
	await act(async () => result.current.exportPdf());
	expect(result.current.exporting).toBe(false);
	expect(settings.onError).toHaveBeenCalledWith(expect.stringContaining("Dialog unavailable"));
	expect(cadSession.exportPdf).not.toHaveBeenCalled();
});
