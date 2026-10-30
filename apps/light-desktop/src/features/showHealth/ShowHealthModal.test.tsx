import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ShowHealthModal } from "./ShowHealthModal";
import {
	currentObjectEditorRequest,
	resetObjectEditorRequestsForTests,
} from "../controlSurfaceInteraction/objectEditorRequest";

const mocks = vi.hoisted(() => ({
	read: vi.fn(),
	manage: vi.fn(),
	dispatch: vi.fn(),
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: { showHealthOpen: true }, dispatch: mocks.dispatch }),
}));
vi.mock("../deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show",
}));
vi.mock("../dmxDiagnostics/DmxDiagnosticsContext", () => ({
	useDmxDiagnostics: () => ({ readShowHealth: mocks.read }),
}));
vi.mock("../groupManagement/GroupManagementProvider", () => ({
	useGroupManagement: () => ({ manage: mocks.manage }),
}));
vi.mock("../showObjects/ShowObjectsView", () => ({
	useShowObjectView: () => {},
}));
vi.mock("../showObjects/ShowObjectsState", () => ({useShowObjectCollectionsReady: () => true}));
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	resetObjectEditorRequestsForTests();
});
const broken = {
	id: "group:7:missing",
	severity: "error",
	message: "Group 7 refers to missing fixture missing.",
	object_kind: "group",
	object_id: "7",
	object_revision: 4,
	fixture_id: null,
	multipatch_instance_id: null,
	remove_missing_group_fixtures: true,
};
const snapshot = (revision: number, findings: unknown[] = [broken]) => ({
	show_id: "show",
	show_revision: revision,
	checked_at: "2026-10-30T00:00:00Z",
	checks: ["Group references"],
	findings,
});

describe("Show Health", () => {
	it("groups findings, applies one cleanup and undoes it against refreshed show authority", async () => {
		mocks.read
			.mockResolvedValueOnce(snapshot(5))
			.mockResolvedValueOnce(snapshot(9, []))
			.mockResolvedValueOnce(snapshot(10));
		mocks.manage
			.mockResolvedValueOnce({
				status: "changed",
				group: { revision: 5 },
				showRevision: 6,
			})
			.mockResolvedValueOnce({
				status: "changed",
				group: { revision: 6 },
				showRevision: 10,
			});
		render(<ShowHealthModal />);
		expect(await screen.findByText(broken.message)).toBeTruthy();
		expect(
			within(screen.getByRole("region", { name: "error findings" })).getByText(
				broken.message,
			),
		).toBeTruthy();
		fireEvent.click(
			screen.getByRole("button", { name: "Remove missing fixtures" }),
		);
		await waitFor(() =>
			expect(mocks.manage).toHaveBeenCalledWith({
				objectId: "7",
				expectedObjectRevision: 4,
				expectedShowRevision: 5,
				operation: { type: "remove_missing_fixtures" },
			}),
		);
		await waitFor(() => expect(mocks.read).toHaveBeenCalledTimes(2));
		await waitFor(() => expect(screen.queryByText(broken.message)).toBeNull());
		fireEvent.click(screen.getByRole("button", { name: "Undo last cleanup" }));
		await waitFor(() =>
			expect(mocks.manage).toHaveBeenLastCalledWith({
				objectId: "7",
				expectedObjectRevision: 5,
				expectedShowRevision: 9,
				operation: { type: "undo" },
			}),
		);
		expect(await screen.findByText("Undid cleanup of Group 7.")).toBeTruthy();
	});
	it("retains an exact object jump until its window mounts without recalling output", async () => {
		mocks.read.mockResolvedValue(snapshot(5));
		render(<ShowHealthModal />);
		fireEvent.click(await screen.findByRole("button", { name: "Go to group" }));
		expect(currentObjectEditorRequest()).toEqual({
			kind: "group",
			objectId: "7",
			showId: "show",
		});
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "groups",
		});
		expect(mocks.manage).not.toHaveBeenCalled();
	});
	it("keeps a failed check actionable", async () => {
		mocks.read
			.mockRejectedValueOnce(new Error("Disconnected"))
			.mockResolvedValueOnce(snapshot(5, []));
		render(<ShowHealthModal />);
		expect((await screen.findByRole("alert")).textContent).toContain(
			"Disconnected",
		);
		fireEvent.click(screen.getByRole("button", { name: "Run check" }));
		expect(await screen.findByText(/0 errors/)).toBeTruthy();
	});
});
