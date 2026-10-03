import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncConflict, SyncStatus } from "../document/sync";
import { SyncStatusChip } from "./SyncStatusChip";

const invoke = vi.hoisted(() => vi.fn());
const handlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn((event: string, handler: (event: { payload: unknown }) => void) => {
		handlers.set(event, handler);
		return Promise.resolve(() => handlers.delete(event));
	}),
}));

function status(state: SyncStatus["state"], overrides: Partial<SyncStatus> = {}): SyncStatus {
	const words: Record<SyncStatus["state"], [string, string]> = {
		synced: ["Synced", "Saved to Control (FOH)."],
		pending: ["2 pending", "Saved on this computer; sending to FOH."],
		offline: ["Offline", "Show not active on Control. 2 changes saved on this computer."],
		conflict: ["1 conflict", "Someone on Control changed the same thing. Choose which version to keep."],
		error: ["Sync error", "The sync journal was damaged."],
	};
	return {
		state,
		label: words[state][0],
		detail: words[state][1],
		deskName: "FOH",
		pending: state === "pending" || state === "offline" ? 2 : 0,
		conflicts: state === "conflict" ? 1 : 0,
		savedToControl: state === "synced",
		savedOnThisComputer: true,
		...overrides,
	};
}

const conflict: SyncConflict = {
	entry: 7,
	kind: "patch_layer",
	id: "truss",
	path: "/name",
	base: "Truss",
	mine: "Back Truss",
	theirs: "Front Truss",
	reason: "field_changed",
	label: "patch layer truss · name",
};

function renderChip() {
	return render(
		<ModalProvider>
			<SyncStatusChip documentKey="show" />
		</ModalProvider>,
	);
}

describe("SyncStatusChip", () => {
	beforeEach(() => {
		invoke.mockReset();
		handlers.clear();
	});

	it("shows nothing for a document bound to no desk", async () => {
		invoke.mockResolvedValue(null);
		const { container } = renderChip();
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("sync_status"));
		expect(container.querySelector(".viz-sync-chip")).toBeNull();
	});

	it.each(["synced", "pending", "offline", "conflict", "error"] as const)(
		"says %s in words and only claims Saved to Control when confirmed",
		async (state) => {
			invoke.mockImplementation((command: string) =>
				Promise.resolve(command === "sync_status" ? status(state) : []),
			);
			renderChip();
			const chip = await screen.findByRole("button", { name: `Sync with FOH: ${status(state).label}` });
			expect(chip).toHaveClass(`is-${state}`);
			fireEvent.click(chip);
			const panel = await screen.findByRole("dialog", { name: "Sync with FOH" });
			expect(within(panel).getByText("Saved")).toBeInTheDocument();
			const claims = within(panel).queryAllByText(/Saved to Control/u);
			if (state === "synced") expect(claims.length).toBeGreaterThan(0);
			else expect(claims).toHaveLength(0);
		},
	);

	it("follows status changes the engine announces", async () => {
		invoke.mockImplementation((command: string) =>
			Promise.resolve(command === "sync_status" ? status("synced") : []),
		);
		renderChip();
		await screen.findByRole("button", { name: "Sync with FOH: Synced" });
		await waitFor(() => expect(handlers.has("sync-status-changed")).toBe(true));
		act(() => handlers.get("sync-status-changed")?.({ payload: status("pending") }));
		expect(await screen.findByRole("button", { name: "Sync with FOH: 2 pending" })).toBeInTheDocument();
	});

	it("keeps both drafts visible and resolves only on a deliberate choice", async () => {
		invoke.mockImplementation((command: string) => {
			if (command === "sync_status") return Promise.resolve(status("conflict"));
			if (command === "sync_conflicts") return Promise.resolve([conflict]);
			return Promise.resolve(undefined);
		});
		renderChip();
		fireEvent.click(await screen.findByRole("button", { name: "Sync with FOH: 1 conflict" }));
		const section = await screen.findByRole("region", { name: "Sync conflict" });
		expect(within(section).getByText("Control: Front Truss · Yours: Back Truss")).toBeInTheDocument();
		expect(invoke).not.toHaveBeenCalledWith("resolve_sync_conflict", expect.anything());
		fireEvent.click(within(section).getByRole("button", { name: "Use mine" }));
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("resolve_sync_conflict", { entry: 7, resolution: "use_mine" }),
		);
	});
});
