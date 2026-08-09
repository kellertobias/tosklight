import {
	act,
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupsWindow } from "./GroupsWindow";

const render = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: ModalProvider });

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	executeCommand: vi.fn(),
	replaceCommand: vi.fn(),
	selectLive: vi.fn(),
	selectFrozen: vi.fn(),
	refresh: vi.fn(),
	recordGroup: vi.fn(),
	resetCommand: vi.fn(),
	manageGroup: vi.fn(),
	loadGroupSettings: vi.fn(),
	setGroupMaster: vi.fn(),
	commandLine: "",
	state: {
		storeArmed: false,
		groupsReturnToStage: false,
		controlMode: "programmer" as "programmer" | "playbacks",
		playbackSetArmed: false,
	},
	runtimeReady: true,
	runtimeCanWrite: true,
	groups: [
		{
			id: "4",
			revision: 1,
			updated_at: "",
			body: {
				name: "Stored Empty",
				color: undefined as string | undefined,
				icon: undefined as string | undefined,
				fixtures: [],
				programming: {},
				derived_from: null,
				frozen_from: null,
			},
			runtime: { master: 1, flashLevel: 0, playbackNumber: 4 },
		},
		{
			id: "5",
			revision: 1,
			updated_at: "",
			body: {
				name: "Stored Populated",
				color: undefined as string | undefined,
				icon: undefined as string | undefined,
				fixtures: ["fixture-1"],
				programming: {},
				derived_from: null,
				frozen_from: null,
			},
			runtime: { master: 1, flashLevel: 0, playbackNumber: 5 },
		},
	],
}));

vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapReady: () => true,
	useActiveShowId: () => "show",
}));
vi.mock("../features/groupManagement/GroupManagementProvider", () => ({
	useGroupManagement: () => ({
		manage: mocks.manageGroup,
		settings: mocks.loadGroupSettings,
	}),
}));
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({
		ready: true,
		text: mocks.commandLine,
		target: "GROUP" as const,
		pristine: true,
		selected: [],
		selectedGroupId: null,
		read: () => ({
			text: mocks.commandLine,
			target: "GROUP",
			pristine: true,
			ready: true,
		}),
		replace: mocks.replaceCommand,
		reset: mocks.resetCommand,
		execute: mocks.executeCommand,
		cancelChoice: vi.fn(),
	}),
}));
vi.mock("../features/groupRecording/GroupRecordingProvider", () => ({
	useGroupRecording: () => ({ record: mocks.recordGroup }),
}));
vi.mock("../features/groupRuntime/groupRuntimeAuthority", () => ({
	useGroupRuntimeAuthority: () => ({
		ready: mocks.runtimeReady,
		serving: mocks.runtimeReady,
		loading: !mocks.runtimeReady,
		canWrite: mocks.runtimeCanWrite,
		groups: mocks.groups,
		setMaster: mocks.setGroupMaster,
		setFlash: vi.fn(),
	}),
}));
vi.mock("../features/groupSelection/useGroupSelectionActions", () => ({
	useGroupSelectionActions: () => ({
		selectLive: mocks.selectLive,
		selectFrozen: mocks.selectFrozen,
	}),
}));

vi.mock("../state/AppContext", () => ({
	useApp: () => ({
		state: mocks.state,
		dispatch: mocks.dispatch,
	}),
}));

function buttonForText(text: string, index = 0) {
	const button = screen.getAllByText(text)[index]?.closest("button");
	if (!button) throw new Error(`Missing button for ${text}`);
	return button;
}

describe("GroupsWindow action routing", () => {
	afterEach(() => {
		cleanup();
		vi.useRealTimers();
	});

	beforeEach(() => {
		mocks.dispatch.mockReset();
		mocks.executeCommand.mockReset().mockResolvedValue(true);
		mocks.replaceCommand.mockReset().mockResolvedValue(true);
		mocks.selectLive.mockReset().mockReturnValue(Promise.resolve(null));
		mocks.selectFrozen.mockReset().mockReturnValue(Promise.resolve(null));
		mocks.refresh.mockReset().mockResolvedValue(undefined);
		mocks.recordGroup.mockReset().mockResolvedValue({ status: "changed" });
		mocks.resetCommand.mockReset().mockResolvedValue(true);
		mocks.manageGroup.mockReset().mockResolvedValue({
			status: "changed",
			group: { revision: 2 },
			showRevision: 13,
			persistenceWarning: null,
		});
		mocks.loadGroupSettings
			.mockReset()
			.mockImplementation(async (id: string) => {
				const target = mocks.groups.find((group) => group.id === id);
				if (!target) return null;
				return {
					showId: "show",
					showRevision: 12,
					group: {
						id,
						revision: target.revision,
						object: { kind: "group", ...target },
					},
					resolvedSpatial: {
						source_order: target.body.fixtures,
						effective_mapping: null,
						mapping_provenance: { type: "none" },
						ordered_fixture_ids: target.body.fixtures,
						projected_positions: [],
						ranks: [],
						rank_count: target.body.fixtures.length,
						warnings: [],
					},
				};
			});
		mocks.setGroupMaster.mockReset().mockResolvedValue(null);
		mocks.commandLine = "";
		mocks.state.storeArmed = false;
		mocks.state.groupsReturnToStage = false;
		mocks.state.controlMode = "programmer";
		mocks.state.playbackSetArmed = false;
		mocks.runtimeReady = true;
		mocks.runtimeCanWrite = true;
		mocks.groups[0].body.color = undefined;
		mocks.groups[0].body.icon = undefined;
		mocks.groups[1].revision = 1;
	});

	it("keeps 200 ordered slots with stable stored and empty identities", () => {
		const { container } = render(<GroupsWindow />);
		const cards = container.querySelectorAll(".group-card");

		expect(cards).toHaveLength(200);
		expect(cards[3]).toHaveTextContent("Stored Empty");
		expect(cards[3]).toHaveAttribute("data-pool-slot-id", "4");
		expect(cards[4]).toHaveTextContent("Stored Populated");
		expect(cards[4]).toHaveAttribute("data-pool-slot-id", "5");
		expect(cards[199]).toHaveAttribute("data-pool-slot-id", "200");
	});

	it("outlines every Record target and routes bare Delete through the whole occupied card", async () => {
		mocks.state.storeArmed = true;
		const view = render(<GroupsWindow />);
		let cards = view.container.querySelectorAll<HTMLButtonElement>(".group-card");
		expect(cards[0]).toHaveClass("record-target");
		expect(cards[3]).toHaveClass("record-target");
		expect(cards[4]).toHaveClass("record-target");

		mocks.state.storeArmed = false;
		mocks.commandLine = "DELETE";
		view.rerender(<GroupsWindow />);
		cards = view.container.querySelectorAll<HTMLButtonElement>(".group-card");
		expect(cards[0]).not.toHaveClass("delete-target");
		expect(cards[3]).toHaveClass("delete-target");
		expect(cards[3]).toHaveTextContent("Delete");
		fireEvent.click(cards[3]);
		await waitFor(() =>
			expect(mocks.executeCommand).toHaveBeenCalledWith("DELETE GROUP 4"),
		);
		expect(mocks.selectLive).not.toHaveBeenCalled();
	});

	it("opens the three-tab Group settings modal on a touch hold", () => {
		vi.useFakeTimers();
		render(<GroupsWindow />);
		const card = buttonForText("Stored Empty");

		fireEvent.pointerDown(card);
		act(() => vi.advanceTimersByTime(600));
		fireEvent.pointerUp(card);

		const dialog = screen.getByRole("dialog", { name: "Group 4 settings" });
		expect(dialog).toBeInTheDocument();
		expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
			"General",
			"Projection",
			"Phase",
		]);
	});

	it("stays on the open settings tab when an edit bumps the Group revision", () => {
		vi.useFakeTimers();
		const view = render(<GroupsWindow />);
		const card = buttonForText("Stored Empty");
		fireEvent.pointerDown(card);
		act(() => vi.advanceTimersByTime(600));
		fireEvent.pointerUp(card);

		fireEvent.click(screen.getByRole("tab", { name: "Projection" }));
		expect(screen.getByRole("tab", { name: "Projection" })).toHaveAttribute(
			"aria-selected",
			"true",
		);

		// Every mapping edit bumps the revision. Remounting on that threw the operator
		// back to General mid-edit. Restored afterwards: these mocks are shared.
		const original = mocks.groups[0].revision;
		try {
			mocks.groups[0].revision = original + 11;
			view.rerender(<GroupsWindow />);
			expect(screen.getByRole("tab", { name: "Projection" })).toHaveAttribute(
				"aria-selected",
				"true",
			);
		} finally {
			mocks.groups[0].revision = original;
		}
	});

	it("refuses every apparent empty-slot interaction while runtime loads", () => {
		mocks.runtimeReady = false;
		mocks.state.storeArmed = true;
		render(<GroupsWindow />);

		expect(screen.getByRole("status")).toHaveTextContent(
			"Group runtime loading…",
		);
		expect(screen.queryByText("Tap to record empty group")).toBeNull();
		expect(mocks.recordGroup).not.toHaveBeenCalled();
		expect(mocks.selectLive).not.toHaveBeenCalled();
	});

	it("right-click chooses the Group SET source and suppresses the native menu", async () => {
		render(<GroupsWindow />);
		const button = buttonForText("Stored Empty");
		const contextMenu = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});
		button.dispatchEvent(contextMenu);

		expect(contextMenu.defaultPrevented).toBe(true);
		await waitFor(() =>
			expect(mocks.replaceCommand).toHaveBeenCalledWith("SET GROUP 4", false),
		);
		expect(
			screen.queryByRole("dialog", { name: "Group 4 settings" }),
		).toBeNull();
	});

	it("does not reopen long-held Group settings after authority replacement", () => {
		vi.useFakeTimers();
		const view = render(<GroupsWindow />);
		fireEvent.pointerDown(buttonForText("Stored Empty"));
		act(() => vi.advanceTimersByTime(650));
		expect(
			screen.getByRole("dialog", { name: "Group 4 settings" }),
		).toBeInTheDocument();

		mocks.runtimeReady = false;
		view.rerender(<GroupsWindow />);
		expect(
			screen.queryByRole("dialog", { name: "Group 4 settings" }),
		).toBeNull();

		mocks.runtimeReady = true;
		view.rerender(<GroupsWindow />);
		expect(
			screen.queryByRole("dialog", { name: "Group 4 settings" }),
		).toBeNull();
	});

	it("selects a stored group through the scoped live-Group gesture", async () => {
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));
		await waitFor(() =>
			expect(mocks.selectLive).toHaveBeenCalledWith(mocks.groups[0]),
		);
	});

	it("uses a double quick press only for frozen Group selection", () => {
		vi.useFakeTimers();
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));
		fireEvent.doubleClick(buttonForText("Stored Empty"));
		act(() => vi.advanceTimersByTime(300));
		expect(mocks.selectFrozen).toHaveBeenCalledWith(mocks.groups[0]);
		expect(mocks.selectLive).not.toHaveBeenCalled();
	});

	it("records directly into a stored empty Group through the typed action", async () => {
		mocks.state.storeArmed = true;
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));
		await waitFor(() =>
			expect(mocks.recordGroup).toHaveBeenCalledWith({
				objectId: "4",
				operation: "overwrite",
				expectedObjectRevision: 1,
			}),
		);
		expect(
			screen.queryByRole("dialog", { name: "Record to Stored Empty" }),
		).toBeNull();
		expect(mocks.executeCommand).not.toHaveBeenCalled();
		expect(mocks.resetCommand).toHaveBeenCalledOnce();
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
	});

	it("records empty pool cells through one typed action without a mode dialog", async () => {
		mocks.state.storeArmed = true;
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Tap to record empty group"));
		await waitFor(() =>
			expect(mocks.recordGroup).toHaveBeenCalledWith({
				objectId: "1",
				operation: "overwrite",
				expectedObjectRevision: 0,
			}),
		);
		expect(screen.queryByRole("dialog")).toBeNull();
		expect(mocks.executeCommand).not.toHaveBeenCalled();
		expect(mocks.resetCommand).toHaveBeenCalledOnce();
		expect(mocks.refresh).not.toHaveBeenCalled();
	});

	it("captures the Group revision when the dialog opens and records Merge", async () => {
		mocks.state.storeArmed = true;
		const view = render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Populated"));
		await screen.findByRole("button", { name: "Merge" });
		mocks.groups[1].revision = 9;
		view.rerender(<GroupsWindow />);
		fireEvent.click(screen.getByRole("button", { name: "Merge" }));
		await waitFor(() =>
			expect(mocks.recordGroup).toHaveBeenCalledWith({
				objectId: "5",
				operation: "merge",
				expectedObjectRevision: 1,
			}),
		);
		expect(mocks.executeCommand).not.toHaveBeenCalled();
		expect(mocks.resetCommand).toHaveBeenCalledOnce();
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
	});

	it("retains the authoritative RECORD command when the typed action fails", async () => {
		mocks.state.storeArmed = true;
		mocks.recordGroup.mockResolvedValue(null);
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));

		await waitFor(() => expect(mocks.recordGroup).toHaveBeenCalledOnce());
		expect(mocks.resetCommand).not.toHaveBeenCalled();
	});

	it("opens Group settings from SET and saves General fields without an Apply footer", async () => {
		mocks.commandLine = "SET ";
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));
		await waitFor(() => expect(mocks.resetCommand).toHaveBeenCalledOnce());
		expect(
			screen.getByRole("dialog", { name: "Group 4 settings" }),
		).toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("Group name"), {
			target: { value: "Copy Center Spot" },
		});
		fireEvent.blur(screen.getByLabelText("Group name"));
		await waitFor(() =>
			expect(mocks.manageGroup).toHaveBeenCalledWith({
				objectId: "4",
				expectedObjectRevision: 1,
				expectedShowRevision: 12,
				operation: {
					type: "update_properties",
					properties: {
						name: "Copy Center Spot",
						color: "#718596",
						icon: "◇",
					},
				},
			}),
		);
		expect(
			screen.queryByRole("button", { name: /Save|Apply|Cancel/ }),
		).toBeNull();
	});

	it("routes SET plus a group tile into playback assignment in Playback mode", async () => {
		mocks.state.controlMode = "playbacks";
		mocks.state.playbackSetArmed = true;
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));

		await waitFor(() =>
			expect(mocks.replaceCommand).toHaveBeenCalledWith("SET GROUP 4", false),
		);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PLAYBACK_SET_ARMED",
			value: false,
		});
		expect(
			screen.queryByRole("dialog", { name: "Group 4 settings" }),
		).not.toBeInTheDocument();
	});

	it("opens the same populated properties modal for a desk-routed SET command", () => {
		mocks.groups[0].body.color = "#d76cff";
		mocks.groups[0].body.icon = "●";
		render(<GroupsWindow />);
		act(() =>
			window.dispatchEvent(
				new CustomEvent("light:group-configuration", { detail: "4" }),
			),
		);
		expect(
			screen.getByRole("dialog", { name: "Group 4 settings" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Group name")).toHaveValue("Stored Empty");
		expect(screen.getByRole("button", { name: /#D76CFF/ })).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /Choose icon/ }),
		).toHaveTextContent("●");
	});
});
