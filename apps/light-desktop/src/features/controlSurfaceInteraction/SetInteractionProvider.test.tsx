import { act, cleanup, render, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { registerControlSurfaceTarget } from "./registry";
import {
	type SetInteractionController,
	SetInteractionProvider,
	useSetInteraction,
} from "./SetInteractionProvider";

const mocks = vi.hoisted(() => ({
	replace: vi.fn(),
	reset: vi.fn(),
	assignGroupMaster: vi.fn(),
	assignVirtualGroupMaster: vi.fn(),
	command: { text: "" },
	groups: [
		{
			id: "4",
			revision: 12,
			kind: "group",
			updated_at: "",
			body: { name: "Front", fixtures: [] },
		},
	],
}));

vi.mock("../programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingCommandLineActions: () => ({
		replace: mocks.replace,
		reset: mocks.reset,
	}),
	useProgrammingCommandLineView: () => mocks.command,
}));
vi.mock("../playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => ({
		assignGroupMaster: mocks.assignGroupMaster,
		assignVirtualGroupMaster: mocks.assignVirtualGroupMaster,
	}),
}));
vi.mock("../showObjects/ShowObjectsState", () => ({
	usePortableGroups: () => mocks.groups,
	useShowObjectCollectionsReady: () => true,
}));
vi.mock("../showObjects/ShowObjectsView", () => ({
	useShowObjectView: vi.fn(),
}));

let controller: SetInteractionController | null = null;

function Probe() {
	controller = useSetInteraction();
	return null;
}

function View({ children }: PropsWithChildren) {
	return (
		<SetInteractionProvider deskId="desk-a" showId="show-a">
			<Probe />
			{children}
		</SetInteractionProvider>
	);
}

const playback = {
	addressing: "current_page" as const,
	pageNumber: 2,
	slot: 3,
	pageObjectId: "page-two",
	pageObjectRevision: 7,
	playbackObjectId: "playback-nine",
	playbackObjectRevision: 8,
};

describe("desk SET interaction owner", () => {
	afterEach(() => cleanup());
	beforeEach(() => {
		controller = null;
		mocks.replace.mockReset().mockResolvedValue(undefined);
		mocks.reset.mockReset().mockResolvedValue(undefined);
		mocks.assignGroupMaster
			.mockReset()
			.mockResolvedValue({ status: "changed" });
		mocks.assignVirtualGroupMaster
			.mockReset()
			.mockResolvedValue({ status: "changed" });
		mocks.command.text = "";
	});

	it("assigns the explicit Group to one revision-guarded Virtual Playback", async () => {
		render(<View />);
		await act(async () => {
			await controller?.arm("touch");
			await controller?.chooseGroup(
				{ objectId: "4", objectRevision: 12 },
				"touch",
			);
			await controller?.choosePlayback(
				{
					addressing: "virtual",
					pageNumber: 2,
					playbackNumber: 1301,
					pageObjectId: "page-two",
					pageObjectRevision: 7,
				},
				"touch",
			);
		});
		expect(mocks.assignVirtualGroupMaster).toHaveBeenCalledWith(
			"4",
			12,
			2,
			1301,
			{
				expectedPageRevision: 7,
				expectedPageObjectId: "page-two",
			},
		);
		expect(mocks.assignGroupMaster).not.toHaveBeenCalled();
		expect(controller?.state?.phase).toBe("idle");
	});

	it("assigns one explicit Group to one revision-guarded current-page Playback", async () => {
		render(<View />);
		await act(async () => {
			await controller?.arm("hardware");
		});
		expect(mocks.replace).toHaveBeenLastCalledWith("SET");

		await act(async () => {
			await controller?.chooseGroup(
				{ objectId: "4", objectRevision: 12 },
				"touch",
			);
		});
		expect(mocks.replace).toHaveBeenLastCalledWith("SET GROUP 4");

		await act(async () => {
			await controller?.choosePlayback(playback, "hardware");
		});
		expect(mocks.assignGroupMaster).toHaveBeenCalledWith("4", 12, 2, 3, {
			expectedPageRevision: 7,
			expectedPageObjectId: "page-two",
			expectedPlaybackRevision: 8,
			expectedPlaybackObjectId: "playback-nine",
		});
		expect(controller?.state?.phase).toBe("idle");
	});

	it("routes bare SET plus Playback as explicit settings without consulting selection", async () => {
		const opened = vi.fn();
		const release = registerControlSurfaceTarget({
			id: "playback-settings-test",
			priority: 1,
			accepts: ({ type }) => type === "open_playback_settings",
			handle: opened,
		});
		render(<View />);
		await act(async () => {
			await controller?.arm("keyboard");
			await controller?.choosePlayback(
				{ ...playback, addressing: "explicit_page" },
				"keyboard",
			);
		});
		expect(opened).toHaveBeenCalledWith({
			type: "open_playback_settings",
			source: "keyboard",
			scope: {
				deskId: "desk-a",
				showId: "show-a",
				surfaceId: "desk-control-surface",
			},
			playback: { ...playback, addressing: "explicit_page" },
		});
		expect(mocks.assignGroupMaster).not.toHaveBeenCalled();
		release();
	});

	it("promotes explicit command text to a pending Group and Clear cancels it", async () => {
		const view = render(<View />);
		await act(async () => {
			await controller?.arm("keyboard");
		});
		mocks.command.text = "SET GROUP 4";
		view.rerender(<View />);
		await waitFor(() =>
			expect(controller?.state).toMatchObject({
				phase: "group_source_pending",
				group: { objectId: "4", objectRevision: 12 },
			}),
		);
		await act(async () => {
			await controller?.clear();
		});
		expect(controller?.state?.phase).toBe("idle");
		expect(mocks.reset).toHaveBeenCalledOnce();
	});

	it("requires Enter after a Group source and routes that exact Group to settings", async () => {
		const opened = vi.fn();
		const release = registerControlSurfaceTarget({
			id: "group-settings-test",
			priority: 1,
			accepts: ({ type }) => type === "open_group_settings",
			handle: opened,
		});
		render(<View />);
		await act(async () => {
			await controller?.arm("touch");
			await controller?.chooseGroup(
				{ objectId: "4", objectRevision: 12 },
				"touch",
			);
		});
		expect(opened).not.toHaveBeenCalled();
		await act(async () => {
			await controller?.enter("touch");
		});
		expect(opened).toHaveBeenCalledWith(
			expect.objectContaining({
				type: "open_group_settings",
				group: { objectId: "4", objectRevision: 12 },
			}),
		);
		release();
	});
});
