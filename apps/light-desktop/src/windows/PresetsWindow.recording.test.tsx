import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PresetsWindow } from "./PresetsWindow";

const mocks = vi.hoisted(() => ({
	state: {
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
	commandReplace: vi.fn(async () => true),
	commandExecute: vi.fn(async () => true),
	commandText: "",
	storePreload: vi.fn(async () => true),
	recall: vi.fn(async () => null),
	updateTarget: vi.fn(),
	preload: {
		ready: true,
		armed: false,
		active: false,
		pending: false,
		phase: "idle" as const,
		error: null,
		actions: null,
	},
}));

vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapReady: () => true,
	useActiveShowId: () => "show-a",
}));
vi.mock(
	"../features/programmerActions/ProgrammerActionsContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useProgrammerActions: () => ({ storePreload: mocks.storePreload }),
	}),
);
vi.mock("../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	usePresets: () => mocks.presets,
}));
vi.mock("../features/presetRecall/PresetRecallProvider", () => ({
	usePresetRecall: () => ({
		actions: { recall: mocks.recall },
		selection: { selected: ["fixture-a"] },
	}),
}));
vi.mock("../features/presetRecording/PresetRecordingProvider", () => ({
	usePresetRecording: () => ({ record: mocks.record }),
}));
vi.mock(
	"../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView",
	() => ({ useProgrammerPreloadLifecycleView: () => mocks.preload }),
);
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({
		text: mocks.commandText,
		reset: mocks.commandReset,
		replace: mocks.commandReplace,
		execute: mocks.commandExecute,
	}),
}));
vi.mock("../components/control/updateWorkflow", () => ({
	requestUpdateTarget: mocks.updateTarget,
}));
vi.mock("../components/shared/GroupStrip", () => ({ GroupStrip: () => null }));

function firstPresetCell() {
	const cell = document.querySelector<HTMLButtonElement>(".preset-card");
	if (!cell) throw new Error("Missing first Preset cell");
	return cell;
}

beforeEach(() => {
	mocks.preload.armed = false;
	mocks.preload.active = false;
	mocks.state.storeArmed = true;
	mocks.state.updateArmed = false;
	mocks.state.presetSetArmed = false;
	mocks.presets = [];
	mocks.dispatch.mockClear();
	mocks.record.mockReset();
	mocks.record.mockResolvedValue(null);
	mocks.commandReset.mockClear();
	mocks.commandReplace.mockClear();
	mocks.commandExecute.mockClear();
	mocks.commandText = "";
	mocks.storePreload.mockClear();
	mocks.recall.mockClear();
	mocks.updateTarget.mockClear();
});

afterEach(cleanup);

describe("PresetsWindow normal recording boundary", () => {
	it("keeps 200 family-numbered slots and their stable qualified identities", () => {
		mocks.state.storeArmed = false;
		mocks.presets = [
			{
				kind: "preset",
				id: "2.5",
				revision: 2,
				updated_at: "",
				body: {
					name: "Lavender",
					number: 5,
					family: "Color",
					values: {},
				},
			},
			{
				kind: "preset",
				id: "4.2",
				revision: 1,
				updated_at: "",
				body: { name: "Other family", number: 2, family: "Beam", values: {} },
			},
		];
		const { container } = render(<PresetsWindow compact />);
		const cards = container.querySelectorAll(".preset-card");

		expect(cards).toHaveLength(200);
		expect(cards[4]).toHaveTextContent("Lavender");
		expect(cards[4]).toHaveTextContent("5");
		expect(cards[4]).toHaveAttribute("data-pool-slot-id", "2.5");
		expect(cards[4].querySelector(".pool-card-icon")).toBeNull();
		expect(cards[199]).toHaveAttribute("data-pool-slot-id", "2.200");
	});

	it("preserves store, update, and Set targets across all shared-grid slots", () => {
		mocks.state.updateArmed = true;
		mocks.state.presetSetArmed = true;
		const { container } = render(<PresetsWindow compact />);
		const cards = container.querySelectorAll(".preset-card");

		expect(cards).toHaveLength(200);
		expect(cards[0]).toHaveClass("store-target", "update-target", "set-target");
		expect(cards[199]).toHaveClass(
			"store-target",
			"update-target",
			"set-target",
		);
	});

	it("right-click opens the SET customization flow and suppresses the native menu", async () => {
		mocks.state.storeArmed = false;
		render(<PresetsWindow compact />);
		const cell = firstPresetCell();
		const contextMenu = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});
		fireEvent(cell, contextMenu);

		expect(contextMenu.defaultPrevented).toBe(true);
		expect(
			await screen.findByRole("dialog", { name: "Configure preset button" }),
		).toBeInTheDocument();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PRESET_SET_ARMED",
			value: false,
		});
		expect(mocks.recall).not.toHaveBeenCalled();
	});

	it("targets only eligible whole Preset cards for Copy, Move, and Delete", () => {
		mocks.state.storeArmed = false;
		mocks.presets = [
			{
				kind: "preset",
				id: "2.1",
				revision: 1,
				updated_at: "",
				body: { name: "Source", number: 1, family: "Color", values: {} },
			},
		];
		mocks.commandText = "COPY";
		const { container, rerender } = render(<PresetsWindow compact />);
		let cards = container.querySelectorAll<HTMLButtonElement>(".preset-card");
		expect(cards[0]).toHaveClass("copy-target");
		expect(cards[0]).toHaveTextContent("Copy");
		expect(cards[1]).not.toHaveClass("copy-target");
		fireEvent.click(cards[0]);
		expect(mocks.commandReplace).toHaveBeenCalledWith("COPY 2.1 AT");

		mocks.commandText = "MOVE 2.1 AT";
		rerender(<PresetsWindow compact />);
		cards = container.querySelectorAll<HTMLButtonElement>(".preset-card");
		expect(cards[0]).not.toHaveClass("move-target");
		expect(cards[1]).toHaveClass("move-target");
		fireEvent.click(cards[1]);
		expect(mocks.commandExecute).toHaveBeenCalledWith("MOVE 2.1 AT 2");

		mocks.commandText = "DELETE";
		rerender(<PresetsWindow compact />);
		cards = container.querySelectorAll<HTMLButtonElement>(".preset-card");
		expect(cards[0]).toHaveClass("delete-target");
		expect(cards[1]).not.toHaveClass("delete-target");
		fireEvent.click(cards[0]);
		expect(mocks.commandExecute).toHaveBeenCalledWith("DELETE 2.1");
	});

	it("recalls an existing Preset through the scoped typed action", () => {
		mocks.state.storeArmed = false;
		mocks.presets = [
			{
				kind: "preset",
				id: "2.1",
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

		expect(mocks.recall).toHaveBeenCalledOnce();
		expect(mocks.recall).toHaveBeenCalledWith({
			objectId: "2.1",
			address: { family: "Color", number: 1 },
		});
	});

	it("keeps Update ahead of Set, Store, and ordinary recall", () => {
		mocks.state.updateArmed = true;
		mocks.state.presetSetArmed = true;
		mocks.state.storeArmed = true;
		mocks.presets = [
			{
				kind: "preset",
				id: "2.1",
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

		expect(mocks.updateTarget).toHaveBeenCalledWith({
			family: { type: "preset" },
			object_id: "2.1",
		});
		expect(mocks.recall).not.toHaveBeenCalled();
		expect(mocks.record).not.toHaveBeenCalled();
	});

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
		mocks.preload.active = true;
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
