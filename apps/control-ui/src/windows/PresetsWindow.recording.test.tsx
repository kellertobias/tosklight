import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PresetsWindow } from "./PresetsWindow";

const mocks = vi.hoisted(() => ({
	state: {
		preload: "idle",
		presetFamily: "Color",
		presetPoolColors: true,
		presetGroupsVisible: false,
		updateArmed: false,
		presetSetArmed: false,
		storeArmed: true,
	} as Record<string, unknown>,
	dispatch: vi.fn(),
	presets: [] as Array<Record<string, unknown>>,
	record: vi.fn(),
	commandReset: vi.fn(async () => true),
	storePreload: vi.fn(async () => true),
	applyPreset: vi.fn(async () => undefined),
}));

vi.mock("../api/ServerContext", () => ({
	useServer: () => ({
		bootstrap: { active_show: { id: "show-a" } },
		storePreload: mocks.storePreload,
		applyPreset: mocks.applyPreset,
	}),
}));
vi.mock("../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: () => undefined,
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	usePresets: () => mocks.presets,
}));
vi.mock("../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionView: () => ({ selected: ["fixture-a"] }),
}));
vi.mock("../features/presetRecording/PresetRecordingProvider", () => ({
	usePresetRecording: () => ({ record: mocks.record }),
}));
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({ reset: mocks.commandReset }),
}));
vi.mock("../components/shared/GroupStrip", () => ({ GroupStrip: () => null }));

function firstPresetCell() {
	const cell = document.querySelector<HTMLButtonElement>(".preset-card");
	if (!cell) throw new Error("Missing first Preset cell");
	return cell;
}

beforeEach(() => {
	mocks.state.preload = "idle";
	mocks.state.storeArmed = true;
	mocks.presets = [];
	mocks.dispatch.mockClear();
	mocks.record.mockReset();
	mocks.record.mockResolvedValue(null);
	mocks.commandReset.mockClear();
	mocks.storePreload.mockClear();
});

afterEach(cleanup);

describe("PresetsWindow normal recording boundary", () => {
	it("records an empty cell as one action-time overwrite at revision zero", () => {
		render(<PresetsWindow compact />);

		fireEvent.click(firstPresetCell());

		expect(mocks.record).toHaveBeenCalledOnce();
		expect(mocks.record).toHaveBeenCalledWith({
			objectId: "2.1",
			address: { family: "Color", number: 1 },
			name: "Preset 1",
			mode: "overwrite",
			expectedObjectRevision: 0,
		});
		expect(mocks.storePreload).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
	});

	it("resets the authoritative command only after a successful normal recording", async () => {
		mocks.record.mockResolvedValue({ status: "changed" });
		render(<PresetsWindow compact />);

		fireEvent.click(firstPresetCell());

		await waitFor(() => expect(mocks.commandReset).toHaveBeenCalledOnce());
	});

	it("retains the authoritative command when normal recording fails", async () => {
		render(<PresetsWindow compact />);

		fireEvent.click(firstPresetCell());

		await waitFor(() => expect(mocks.record).toHaveBeenCalledOnce());
		expect(mocks.commandReset).not.toHaveBeenCalled();
	});

	it("preserves the existing-target mode dialog and object revision", () => {
		mocks.presets = [
			{
				kind: "preset",
				id: "01",
				revision: 4,
				updated_at: "",
				body: {
					name: "Blue",
					number: 1,
					family: "Color",
					values: {},
				},
			},
		];
		render(<PresetsWindow compact />);

		fireEvent.click(firstPresetCell());
		fireEvent.click(screen.getByRole("button", { name: "Merge" }));

		expect(mocks.record).toHaveBeenCalledWith({
			objectId: "01",
			address: { family: "Color", number: 1 },
			name: "Blue",
			mode: "merge",
			expectedObjectRevision: 4,
		});
	});

	it("prefers the backend canonical identity over a legacy alias", () => {
		mocks.presets = [
			{
				kind: "preset",
				id: "01",
				revision: 8,
				updated_at: "",
				body: { name: "Legacy", number: 1, family: "Color", values: {} },
			},
			{
				kind: "preset",
				id: "2.1",
				revision: 4,
				updated_at: "",
				body: { name: "Canonical", number: 1, family: "Color", values: {} },
			},
		];
		render(<PresetsWindow compact />);

		fireEvent.click(firstPresetCell());
		fireEvent.click(screen.getByRole("button", { name: "Merge" }));

		expect(mocks.record).toHaveBeenCalledWith(
			expect.objectContaining({
				objectId: "2.1",
				name: "Canonical",
				expectedObjectRevision: 4,
			}),
		);
	});

	it("keeps Preload recording on its established path", () => {
		mocks.state.preload = "active";
		render(<PresetsWindow compact />);

		fireEvent.click(firstPresetCell());

		expect(mocks.record).not.toHaveBeenCalled();
		expect(mocks.storePreload).toHaveBeenCalledWith(
			{
				target: "preset",
				target_id: "2.1",
				name: "Preset 1",
				mode: "overwrite",
				family: "Color",
			},
			0,
		);
		expect(mocks.commandReset).not.toHaveBeenCalled();
	});

});
