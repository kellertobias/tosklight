import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackSectionSettingsModal } from "./PlaybackSectionSettingsModal";
const model = vi.hoisted(() => ({
	update: vi.fn(),
	solo: vi.fn(),
	surfaces: {
		playback_surfaces: {
			main: { name: "Main", page_source: { type: "own", page: 2 } },
			"screen:second": { name: "Screen1", page_source: { type: "own", page: 4 } },
		},
		resolved_pages: { main: 2 },
	},
}));
vi.mock("../../features/screens/ScreensContext", () => ({ useScreens: () => ({ surfaces: model.surfaces, updatePlaybackSurface: model.update }) }));
vi.mock("./PlaybackSoloSettings", () => ({ PlaybackSoloSettings: (props: unknown) => { model.solo(props); return <p>Solo manager</p>; } }));
afterEach(cleanup);
beforeEach(() => { model.update.mockReset().mockResolvedValue(undefined); model.solo.mockClear(); });
describe("Playback section settings", () => {
	it("places both settings controls in the standard modal title bar without a body tab row", () => {
		const showGrid = vi.fn();
		render(<PlaybackSectionSettingsModal surfaceId="main" onClose={vi.fn()} onShowGrid={showGrid} />);
		const page = screen.getByRole("tab", { name: "Page source" });
		const solo = screen.getByRole("tab", { name: "Manage Solo Regions" });
		expect(page.closest(".ui-modal-titlebar")).not.toBeNull();
		expect(solo.closest(".ui-modal-titlebar")).toBe(page.closest(".ui-modal-titlebar"));
		expect(page).toHaveAttribute("aria-selected", "true");
		fireEvent.click(solo);
		expect(solo).toHaveAttribute("aria-selected", "true");
		expect(page).toHaveAttribute("aria-selected", "false");
		expect(showGrid).toHaveBeenCalledOnce();
		expect(screen.getAllByRole("tablist")).toHaveLength(1);
		fireEvent.click(page);
		expect(screen.getByLabelText("Page")).toBeInTheDocument();
	});
	it("updates only local page source through the existing sparse intent", async () => {
		render(<PlaybackSectionSettingsModal surfaceId="main" onClose={vi.fn()} />);
		fireEvent.change(screen.getByLabelText("Page"), { target: { value: "7" } });
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(model.update).toHaveBeenCalledWith("main", { page_source: { type: "own", page: 7 } }));
		expect(model.update).toHaveBeenCalledOnce();
	});
	it("offers own/pinned/follow and saves an exact named surface without layout", async () => {
		render(<PlaybackSectionSettingsModal surfaceId="main" onClose={vi.fn()} />);
		fireEvent.click(screen.getByRole("button", { name: "Page source" }));
		fireEvent.click(screen.getByRole("option", { name: "Follow surface" }));
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() => expect(model.update).toHaveBeenCalledWith("main", { page_source: { type: "follow", surface_id: "screen:second" } }));
	});
	it("shows authoritative cycle rejection and keeps the modal open", async () => {
		model.update.mockRejectedValue(new Error("Playback page sources contain a cycle"));
		render(<PlaybackSectionSettingsModal surfaceId="main" onClose={vi.fn()} />);
		fireEvent.click(screen.getByRole("button", { name: "Save" }));
		expect(await screen.findByRole("alert")).toHaveTextContent("contain a cycle");
		expect(screen.getByRole("dialog", { name: "Playback section settings" })).toBeInTheDocument();
	});
	it("passes the exact local surface and close callback to the independent Solo manager", () => {
		const close = vi.fn();
		render(<PlaybackSectionSettingsModal surfaceId="screen:second" onClose={close} />);
		fireEvent.click(screen.getByRole("tab", { name: "Manage Solo Regions" }));
		expect(model.solo).toHaveBeenLastCalledWith({ surfaceId: "screen:second", onClose: close });
	});
});
