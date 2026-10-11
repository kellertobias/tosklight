import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackSoloEditor } from "../../features/playbackSolo/PlaybackSoloEditorContext";
import type { PlaybackSoloCapability } from "../../features/playbackSolo/contracts";
import { PlaybackSoloRegionManager } from "./PlaybackSoloRegionManager";
const region = { id: "stable", name: "Across pages", members: [
	{ page: 2, position: 101 }, { page: 4, position: 102 },
] };
afterEach(cleanup);
function setup() {
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
	const view = render(<PlaybackSoloRegionManager regions={[region]}
		capability={capability} editor={editor} onClose={close} />);
	return { editor, capability, close, ...view };
}
describe("Manage Solo Regions", () => {
	it("starts a new grid draft and closes without persisting", () => {
		const s = setup();
		fireEvent.click(s.getByRole("button", { name: "Create Solo Region" }));
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
