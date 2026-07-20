import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PreloadStoreModal } from "./PreloadStoreModal";

const CUE_LIST_ID = "11111111-1111-4111-8111-111111111111";

const mocks = vi.hoisted(() => ({
	state: { preloadStoreOpen: true },
	dispatch: vi.fn(),
	storePreload: vi.fn(),
	recordCue: vi.fn(),
	views: vi.fn(),
	presets: [
		{
			kind: "preset",
			id: "1",
			revision: 4,
			updated_at: "",
			body: { name: "Blue", number: 1, family: "Color", values: {} },
		},
	],
	cueLists: [
		{
			kind: "cue_list",
			id: "11111111-1111-4111-8111-111111111111",
			revision: 7,
			updated_at: "",
			body: { name: "Main", cues: [] },
		},
	],
}));

vi.mock("../../api/ServerContext", () => ({
	useServer: () => ({
		storePreload: mocks.storePreload,
		error: null,
	}),
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	usePresets: () => mocks.presets,
	useCueLists: () => mocks.cueLists,
}));
vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: mocks.views,
}));

beforeEach(() => {
	mocks.state.preloadStoreOpen = true;
	mocks.dispatch.mockClear();
	mocks.storePreload.mockReset();
	mocks.storePreload.mockResolvedValue(true);
	mocks.recordCue.mockReset();
	mocks.recordCue.mockResolvedValue({ status: "changed" });
	mocks.views.mockClear();
});

afterEach(cleanup);

describe("PreloadStoreModal", () => {
	it("keeps both scoped views dormant while closed", () => {
		mocks.state.preloadStoreOpen = false;
		render(<PreloadStoreModal />);

		expect(mocks.views).toHaveBeenCalledWith("preset", false);
		expect(mocks.views).toHaveBeenCalledWith("cue_list", false);
		expect(mocks.recordCue).not.toHaveBeenCalled();
		expect(mocks.storePreload).not.toHaveBeenCalled();
	});

	it("records pending Preload to a Cue through one typed action", async () => {
		render(<PreloadStoreModal />);
		fireEvent.click(screen.getByRole("button", { name: "Cue" }));
		await screen.findByRole("button", { name: "Main" });
		fireEvent.change(screen.getByLabelText("Cue number"), {
			target: { value: "2.5" },
		});
		fireEvent.change(screen.getByLabelText("Name"), {
			target: { value: "Look" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Record to Cue 2.5" }));

		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(mocks.recordCue).toHaveBeenCalledWith({
			target: { kind: "cue_list", cueListId: CUE_LIST_ID },
			operation: "overwrite",
			cueNumber: 2.5,
			timing: {},
			cueOnly: false,
			name: "Look",
			capturePolicy: "pending_or_active_preload",
			activationPolicy: "hold",
		});
		expect(mocks.storePreload).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "preloadStoreOpen",
			value: false,
		});
	});

	it("keeps the modal open when typed Cue recording fails", async () => {
		mocks.recordCue.mockResolvedValue(null);
		render(<PreloadStoreModal />);
		fireEvent.click(screen.getByRole("button", { name: "Cue" }));
		await screen.findByRole("button", { name: "Main" });
		fireEvent.click(screen.getByRole("button", { name: "Record to Cue 1" }));

		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(mocks.dispatch).not.toHaveBeenCalled();
	});

	it("preserves the existing Preset Preload path and object revision", async () => {
		render(<PreloadStoreModal />);
		fireEvent.click(screen.getByRole("button", { name: "Record to Preset 1" }));

		await waitFor(() => expect(mocks.storePreload).toHaveBeenCalledOnce());
		expect(mocks.storePreload).toHaveBeenCalledWith(
			{
				target: "preset",
				target_id: "1",
				name: undefined,
				mode: "merge",
			},
			4,
		);
		expect(mocks.recordCue).not.toHaveBeenCalled();
	});
});
