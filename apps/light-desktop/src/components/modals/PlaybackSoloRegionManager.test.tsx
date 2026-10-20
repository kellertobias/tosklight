import { act, cleanup, fireEvent, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackSoloEditor } from "../../features/playbackSolo/PlaybackSoloEditorContext";
import type { PlaybackSoloCapability, PlaybackSoloRegion } from "../../features/playbackSolo/contracts";
import { PlaybackSoloRegionManager } from "./PlaybackSoloRegionManager";
const region = { id: "stable", name: "Across pages", members: [
	{ page: 2, position: 101 }, { page: 4, position: 102 },
] };
afterEach(cleanup);
function setup(regions: readonly PlaybackSoloRegion[] | null = [region]) {
	const editor: PlaybackSoloEditor = {
		available: true, ready: true, selecting: false, selected: [], name: "",
		saving: false, error: null, setName: vi.fn(), begin: vi.fn(),
		beginEdit: vi.fn(), save: vi.fn(async () => {}), cancel: vi.fn(),
	};
	const capability: PlaybackSoloCapability = {
		authorityId: "show", authorityGeneration: 1, available: true, error: null,
		getRegions: () => [region], isSaving: () => false, subscribe: () => () => {},
		activate: () => () => {}, load: async () => [region], clearError: () => {},
		save: vi.fn(async (next) => next),
	};
	const close = vi.fn();
	const view = render(<PlaybackSoloRegionManager regions={regions}
		capability={capability} editor={editor} onClose={close} />);
	return { editor, capability, close, ...view };
}
describe("Manage Solo Regions", () => {
	it("lists exact cross-page members in an accessible table and selects without saving", () => {
		const s = setup();
		const table = within(s.getByRole("table", { name: "Solo Regions" }));
		expect(table.getByRole("columnheader", { name: "Name" })).toBeTruthy();
		expect(table.getByRole("columnheader", { name: "Playback members" })).toBeTruthy();
		expect(table.getByRole("cell", { name: "2.101, 4.102" })).toBeTruthy();
		const select = table.getByRole("button", { name: "Across pages" });
		expect(select.getAttribute("aria-pressed")).toBe("false");
		fireEvent.click(select);
		expect(select.getAttribute("aria-pressed")).toBe("true");
		expect(s.capability.save).not.toHaveBeenCalled();
		expect(s.editor.beginEdit).not.toHaveBeenCalled();
	});
	it("distinguishes an empty saved list from loading and keeps Create available", () => {
		const s = setup([]);
		expect(s.getByRole("status").textContent).toBe("No Solo Regions.");
		expect(s.queryByRole("table")).toBeNull();
		expect(s.getByRole("button", { name: "Create New Solo Region" }).hasAttribute("disabled")).toBe(false);
		s.rerender(<PlaybackSoloRegionManager regions={null}
			capability={s.capability} editor={s.editor} onClose={s.close} />);
		expect(s.getByRole("status").textContent).toBe("Loading Solo Regions…");
	});
	it("changes selection and deletes only that region while preserving its peers", async () => {
		const peer = { ...region, id: "peer", name: "Other region" };
		const s = setup([region, peer]);
		fireEvent.click(s.getByRole("button", { name: "Across pages" }));
		fireEvent.click(s.getByRole("button", { name: "Other region" }));
		expect((s.getByLabelText("Region name") as HTMLInputElement).value).toBe("Other region");
		await act(async () => fireEvent.click(s.getByRole("button", { name: "Delete" })));
		expect(s.capability.save).toHaveBeenCalledWith([region]);
		expect(s.queryByRole("button", { name: "Edit" })).toBeNull();
	});

	it("saves only the selected region color and preserves names and cross-page membership", async () => {
		const colored = { ...region, color: "#f97316" };
		const peer = { ...region, id: "peer", name: "Other region", color: "#22c55e" };
		const s = setup([colored, peer]);
		fireEvent.click(s.getByRole("button", { name: "Across pages" }));
		fireEvent.click(s.getByRole("button", { name: /#F97316/ }));
		await act(async () => fireEvent.click(s.getByRole("option", { name: "Use color #06b6d4" })));
		expect(s.capability.save).toHaveBeenCalledWith([{ ...colored, color: "#06b6d4" }, peer]);
		expect(s.editor.beginEdit).not.toHaveBeenCalled();
		fireEvent.click(s.getByRole("button", { name: "Other region" }));
		expect(s.getByRole("button", { name: /#22C55E/ })).toBeTruthy();
	});
	it("keeps a saved outline color through rename and membership editing", async () => {
		const colored = { ...region, color: "#06b6d4" };
		const s = setup([colored]);
		fireEvent.click(s.getByRole("button", { name: "Across pages" }));
		fireEvent.change(s.getByLabelText("Region name"), { target: { value: "Renamed" } });
		await act(async () => fireEvent.click(s.getByRole("button", { name: "Rename" })));
		expect(s.capability.save).toHaveBeenCalledWith([{ ...colored, name: "Renamed" }]);
		fireEvent.click(s.getByRole("button", { name: "Edit" }));
		expect(s.editor.beginEdit).toHaveBeenCalledWith(colored);
	});
	it("shows the amber default for a region saved before colors existed", () => {
		const s = setup();
		fireEvent.click(s.getByRole("button", { name: "Across pages" }));
		expect(s.getByRole("button", { name: /#FFB30F/ })).toBeTruthy();
		expect(s.capability.save).not.toHaveBeenCalled();
	});
	it("starts a new grid draft and closes without persisting", () => {
		const s = setup();
		fireEvent.click(s.getByRole("button", { name: "Create New Solo Region" }));
		expect(s.editor.begin).toHaveBeenCalledOnce();
		expect(s.close).toHaveBeenCalledOnce();
		expect(s.capability.save).not.toHaveBeenCalled();
	});
	it("selects an existing region then hands exact cross-page members to the grid", () => {
		const s = setup();
		expect(s.queryByRole("button", { name: "Edit" })).toBeNull();
		fireEvent.click(s.getByRole("button", { name: "Across pages" }));
		fireEvent.click(s.getByRole("button", { name: "Edit" }));
		expect(s.editor.beginEdit).toHaveBeenCalledWith(region);
		expect(s.close).toHaveBeenCalledOnce();
		expect(s.capability.save).not.toHaveBeenCalled();
	});
	it("renames without changing identity or membership then deletes only the selected region", async () => {
		const s = setup();
		fireEvent.click(s.getByRole("button", { name: "Across pages" }));
		fireEvent.change(s.getByLabelText("Region name"), { target: { value: "New name" } });
		await act(async () => fireEvent.click(s.getByRole("button", { name: "Rename" })));
		expect(s.capability.save).toHaveBeenCalledWith([{ ...region, name: "New name" }]);
		await act(async () => fireEvent.click(s.getByRole("button", { name: "Delete" })));
		expect(s.capability.save).toHaveBeenLastCalledWith([]);
	});
});
