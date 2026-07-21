import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupsWindow } from "./GroupsWindow";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	executeCommand: vi.fn(),
	replaceCommand: vi.fn(),
	selectLive: vi.fn(),
	selectFrozen: vi.fn(),
	refresh: vi.fn(),
	recordGroup: vi.fn(),
	resetCommand: vi.fn(),
	updateGroup: vi.fn(),
	setGroupMaster: vi.fn(),
	commandLine: "",
	state: { storeArmed: false, groupsReturnToStage: false },
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
				master: 1,
				playback_fader: 4,
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
				master: 1,
				playback_fader: 5,
				derived_from: null,
				frozen_from: null,
			},
			runtime: { master: 1, flashLevel: 0, playbackNumber: 5 },
		},
	],
}));

vi.mock("../api/ServerContext", () => ({
	useServer: () => ({
		get playbacks() {
			throw new Error("Groups Window must not read broad playbacks");
		},
		bootstrap: { active_show: { id: "show" } },
		groups: mocks.groups,
		patch: { fixtures: [], revision: 0 },
		selectedFixtures: [],
		selectedGroupId: null,
		refresh: mocks.refresh,
		updateGroup: mocks.updateGroup,
		undoGroup: vi.fn(),
		refreshFrozenGroup: vi.fn(),
		detachDerivedGroup: vi.fn(),
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
	afterEach(() => cleanup());

	beforeEach(() => {
		mocks.dispatch.mockReset();
		mocks.executeCommand.mockReset().mockResolvedValue(true);
		mocks.replaceCommand.mockReset().mockResolvedValue(true);
		mocks.selectLive.mockReset().mockReturnValue(Promise.resolve(null));
		mocks.selectFrozen.mockReset().mockReturnValue(Promise.resolve(null));
		mocks.refresh.mockReset().mockResolvedValue(undefined);
		mocks.recordGroup.mockReset().mockResolvedValue({ status: "changed" });
		mocks.resetCommand.mockReset().mockResolvedValue(true);
		mocks.updateGroup.mockReset().mockResolvedValue(true);
		mocks.setGroupMaster.mockReset().mockResolvedValue(null);
		mocks.commandLine = "";
		mocks.state.storeArmed = false;
		mocks.state.groupsReturnToStage = false;
		mocks.runtimeReady = true;
		mocks.runtimeCanWrite = true;
		mocks.groups[0].body.color = undefined;
		mocks.groups[0].body.icon = undefined;
		mocks.groups[1].revision = 1;
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

	it("disables the Group master when the scoped writer is absent", () => {
		mocks.runtimeCanWrite = false;
		render(<GroupsWindow />);
		fireEvent.contextMenu(buttonForText("Stored Empty"));

		expect(screen.getByLabelText("Stored Empty master")).toBeDisabled();
		expect(mocks.setGroupMaster).not.toHaveBeenCalled();
	});

	it("does not reopen an old Group context after authority replacement", () => {
		const view = render(<GroupsWindow />);
		fireEvent.contextMenu(buttonForText("Stored Empty"));
		expect(screen.getByLabelText("Stored Empty master")).toBeInTheDocument();

		mocks.runtimeReady = false;
		view.rerender(<GroupsWindow />);
		expect(screen.queryByLabelText("Stored Empty master")).toBeNull();

		mocks.runtimeReady = true;
		view.rerender(<GroupsWindow />);
		expect(screen.queryByLabelText("Stored Empty master")).toBeNull();
	});

	it("writes the context master through exact scoped Group authority", async () => {
		render(<GroupsWindow />);
		fireEvent.contextMenu(buttonForText("Stored Empty"));
		fireEvent.change(screen.getByLabelText("Stored Empty master"), {
			target: { value: "35" },
		});

		expect(mocks.setGroupMaster).toHaveBeenCalledWith("4", 0.35);
	});

	it("selects a stored group through the scoped live-Group gesture", () => {
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));
		expect(mocks.selectLive).toHaveBeenCalledWith(mocks.groups[0]);
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

	it("opens and saves group properties when SET is armed before tapping the tile", async () => {
		mocks.commandLine = "SET ";
		render(<GroupsWindow />);
		fireEvent.click(buttonForText("Stored Empty"));
		expect(mocks.resetCommand).toHaveBeenCalledOnce();
		expect(
			screen.getByRole("dialog", { name: "Group properties" }),
		).toBeInTheDocument();
		fireEvent.change(screen.getByLabelText("Group name"), {
			target: { value: "Copy Center Spot" },
		});
		fireEvent.click(screen.getByRole("button", { name: /#718596/ }));
		fireEvent.click(screen.getByRole("option", { name: "Use color #1bd6ec" }));
		fireEvent.click(screen.getByRole("button", { name: /Choose icon/ }));
		fireEvent.click(await screen.findByRole("button", { name: "Use ★" }));
		fireEvent.click(screen.getByRole("button", { name: "Save group" }));
		await waitFor(() =>
			expect(mocks.updateGroup).toHaveBeenCalledWith("4", {
				name: "Copy Center Spot",
				color: "#1bd6ec",
				icon: "★",
			}),
		);
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
			screen.getByRole("dialog", { name: "Group properties" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Group name")).toHaveValue("Stored Empty");
		expect(screen.getByRole("button", { name: /#D76CFF/ })).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /Choose icon/ }),
		).toHaveTextContent("●");
	});
});
