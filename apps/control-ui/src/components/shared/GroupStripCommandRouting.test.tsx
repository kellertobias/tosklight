import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupStrip } from "./GroupStrip";
import { UPDATE_TARGET_EVENT } from "../control/updateWorkflow";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	executeCommand: vi.fn(),
	selectLive: vi.fn(),
	replaceCommand: vi.fn(),
	selectFrozen: vi.fn(),
	resetCommand: vi.fn(),
	refresh: vi.fn(),
	recordGroup: vi.fn(),
	state: { storeArmed: false, updateArmed: false },
	groups: [
		{
			id: "1",
			revision: 4,
			kind: "group",
			updated_at: "",
			body: {
				name: "Shortcut Group",
				fixtures: ["fixture-1"],
			},
		},
	],
}));

vi.mock("../../api/ServerContext", () => ({
	useServer: () => ({
		bootstrap: { active_show: { id: "show" } },
		refresh: mocks.refresh,
	}),
}));
vi.mock("../control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({
		ready: true,
		text: "",
		target: "GROUP" as const,
		pristine: true,
		selected: [],
		selectedGroupId: null,
		read: () => ({ text: "", target: "GROUP", pristine: true, ready: true }),
		replace: mocks.replaceCommand,
		reset: mocks.resetCommand,
		execute: mocks.executeCommand,
		cancelChoice: vi.fn(),
	}),
}));
vi.mock("../../features/groupRecording/GroupRecordingProvider", () => ({
	useGroupRecording: () => ({ record: mocks.recordGroup }),
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	usePortableGroups: () => mocks.groups,
}));
vi.mock("../../features/groupSelection/useGroupSelectionActions", () => ({
	useGroupSelectionActions: () => ({
		selectLive: mocks.selectLive,
		selectFrozen: mocks.selectFrozen,
	}),
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: mocks.state,
		dispatch: mocks.dispatch,
	}),
}));

describe("GroupStrip action routing", () => {
	afterEach(() => cleanup());

	beforeEach(() => {
		mocks.dispatch.mockReset();
		mocks.executeCommand.mockReset().mockResolvedValue(true);
		mocks.selectLive.mockReset().mockReturnValue(Promise.resolve(null));
		mocks.replaceCommand.mockReset().mockResolvedValue(true);
		mocks.selectFrozen.mockReset().mockReturnValue(Promise.resolve(null));
		mocks.refresh.mockReset().mockResolvedValue(undefined);
		mocks.recordGroup.mockReset().mockResolvedValue({ status: "changed" });
		mocks.resetCommand.mockReset().mockResolvedValue(true);
		mocks.state.storeArmed = false;
		mocks.state.updateArmed = false;
		mocks.groups = [
			{
				id: "1",
				revision: 4,
				kind: "group",
				updated_at: "",
				body: {
					name: "Shortcut Group",
					fixtures: ["fixture-1"],
				},
			},
		];
	});

	it("selects shortcut groups through the scoped live-Group gesture", () => {
		render(<GroupStrip />);
		fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
		expect(mocks.selectLive).toHaveBeenCalledWith(mocks.groups[0]);
		expect(mocks.replaceCommand).toHaveBeenCalledWith("GROUP 1");
	});

	it("freezes shortcut groups through the scoped selectGroup action", () => {
		render(<GroupStrip />);
		fireEvent.doubleClick(screen.getByText("Shortcut Group").closest("button")!);
		expect(mocks.selectFrozen).toHaveBeenCalledWith(mocks.groups[0]);
		expect(mocks.selectLive).not.toHaveBeenCalled();
	});

	it("leaves the command line alone when refused authority blocks selection", () => {
		mocks.selectLive.mockReturnValue(null);
		render(<GroupStrip />);
		fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
		expect(mocks.selectLive).toHaveBeenCalledOnce();
		expect(mocks.replaceCommand).not.toHaveBeenCalled();
	});

	it("routes an armed Update touch to the exact Group target without selecting it", () => {
		mocks.state.updateArmed = true;
		const selected = vi.fn();
		window.addEventListener(UPDATE_TARGET_EVENT, selected);
		render(<GroupStrip />);
		fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
		expect((selected.mock.calls[0][0] as CustomEvent).detail).toEqual({
			family: { type: "group" },
			object_id: "1",
		});
		expect(mocks.selectLive).not.toHaveBeenCalled();
		window.removeEventListener(UPDATE_TARGET_EVENT, selected);
	});

	it("records directly into stored empty shortcut groups", async () => {
		mocks.state.storeArmed = true;
		mocks.groups = [
			{
				id: "1",
				revision: 7,
				kind: "group",
				updated_at: "",
				body: {
					name: "Stored Empty Shortcut",
					fixtures: [],
				},
			},
		];
		render(<GroupStrip />);
		fireEvent.click(
			screen.getByText("Stored Empty Shortcut").closest("button")!,
		);
		await waitFor(() =>
			expect(mocks.recordGroup).toHaveBeenCalledWith({
				objectId: "1",
				operation: "overwrite",
				expectedObjectRevision: 7,
			}),
		);
		expect(
			screen.queryByRole("dialog", { name: "Record to Stored Empty Shortcut" }),
		).toBeNull();
		expect(mocks.executeCommand).not.toHaveBeenCalled();
		expect(mocks.resetCommand).toHaveBeenCalledOnce();
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
	});

	it("uses the typed Merge operation for populated shortcut Groups", async () => {
		mocks.state.storeArmed = true;
		const view = render(<GroupStrip />);
		fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
		mocks.groups[0].revision = 9;
		view.rerender(<GroupStrip />);
		fireEvent.click(screen.getByRole("button", { name: "Merge" }));
		await waitFor(() =>
			expect(mocks.recordGroup).toHaveBeenCalledWith({
				objectId: "1",
				operation: "merge",
				expectedObjectRevision: 4,
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

	it("retains the authoritative RECORD command when shortcut recording fails", async () => {
		mocks.state.storeArmed = true;
		mocks.recordGroup.mockResolvedValue(null);
		render(<GroupStrip />);
		fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
		fireEvent.click(screen.getByRole("button", { name: "Overwrite" }));

		await waitFor(() => expect(mocks.recordGroup).toHaveBeenCalledOnce());
		expect(mocks.resetCommand).not.toHaveBeenCalled();
	});
});
