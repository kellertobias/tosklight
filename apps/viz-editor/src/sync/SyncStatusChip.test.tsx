import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncConflict, SyncStatus } from "../document/sync";
import { SyncStatusChip } from "./SyncStatusChip";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
const handlers = vi.hoisted(
	() => new Map<string, (event: { payload: unknown }) => void>(),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

function status(
	state: SyncStatus["state"],
	overrides: Partial<SyncStatus> = {},
): SyncStatus {
	const words: Record<SyncStatus["state"], [string, string]> = {
		synced: ["Synced", "Saved to Control (FOH)."],
		pending: ["2 pending", "Saved on this computer; sending to FOH."],
		offline: [
			"Offline",
			"Show not active on Control. 2 changes saved on this computer.",
		],
		conflict: [
			"1 conflict",
			"Someone on Control changed the same thing. Choose which version to keep.",
		],
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

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function chip(documentKey = "show") {
	return (
		<ModalProvider>
			<SyncStatusChip documentKey={documentKey} />
		</ModalProvider>
	);
}

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
		listen.mockReset();
		listen.mockImplementation(
			(event: string, handler: (event: { payload: unknown }) => void) => {
				handlers.set(event, handler);
				return Promise.resolve(() => {
					if (handlers.get(event) === handler) handlers.delete(event);
				});
			},
		);
	});

	it("reads settled status only after asynchronous listener registration", async () => {
		const registration = deferred<() => void>();
		listen.mockReturnValue(registration.promise);
		let current = status("offline", {
			detail: "Connecting to FOH.",
			pending: 0,
		});
		invoke.mockImplementation(() => Promise.resolve(current));
		renderChip();
		await act(async () => {});
		expect(invoke).not.toHaveBeenCalledWith("sync_status");
		// The engine settles while no listener exists; its latest snapshot must recover that event.
		current = status("synced");
		await act(async () => registration.resolve(() => {}));
		expect(
			await screen.findByRole("button", { name: "Sync with FOH: Synced" }),
		).toBeInTheDocument();
	});

	it("never lets a late initial reply overwrite a newer announced status", async () => {
		const initial = deferred<SyncStatus>();
		invoke.mockReturnValue(initial.promise);
		renderChip();
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("sync_status"));
		act(() =>
			handlers.get("sync-status-changed")?.({ payload: status("synced") }),
		);
		await act(async () => initial.resolve(status("offline")));
		expect(
			screen.getByRole("button", { name: "Sync with FOH: Synced" }),
		).toBeInTheDocument();
	});

	it("retains newer honest failure when the initial read rejects late", async () => {
		let reject!: (reason: Error) => void;
		invoke.mockReturnValue(
			new Promise((_, fail) => {
				reject = fail;
			}),
		);
		renderChip();
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("sync_status"));
		act(() =>
			handlers.get("sync-status-changed")?.({ payload: status("error") }),
		);
		await act(async () => reject(new Error("initial read unavailable")));
		expect(
			screen.getByRole("button", { name: "Sync with FOH: Sync error" }),
		).toBeInTheDocument();
	});

	it("resubscribes for a changed document and ignores its predecessor's late read", async () => {
		const old = deferred<SyncStatus>();
		const next = deferred<SyncStatus | null>();
		invoke.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
		const view = render(chip("old"));
		await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
		act(() =>
			handlers.get("sync-status-changed")?.({ payload: status("synced") }),
		);
		view.rerender(chip("new"));
		await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));
		await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
		expect(
			screen.queryByRole("button", { name: "Sync with FOH: Synced" }),
		).not.toBeInTheDocument();
		await act(async () => {
			next.resolve(null);
			old.resolve(status("synced"));
		});
		expect(view.container.querySelector(".viz-sync-chip")).toBeNull();
	});

	it("unlistens a registration that completes after unmount without reading status", async () => {
		const registration = deferred<() => void>();
		const stop = vi.fn();
		invoke.mockResolvedValue(null);
		listen.mockReturnValue(registration.promise);
		const view = renderChip();
		view.unmount();
		await act(async () => registration.resolve(stop));
		expect(stop).toHaveBeenCalledOnce();
		expect(invoke).not.toHaveBeenCalledWith("sync_status");
	});

	it("disposes a previous document's late listener and ignores its events", async () => {
		const oldRegistration = deferred<() => void>();
		const stop = vi.fn();
		listen.mockReturnValueOnce(oldRegistration.promise);
		invoke.mockResolvedValue(null);
		const view = render(chip("old"));
		const oldHandler = listen.mock.calls[0][1];
		view.rerender(chip("new"));
		await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
		await act(async () => oldRegistration.resolve(stop));
		act(() => oldHandler({ payload: status("synced") }));
		expect(stop).toHaveBeenCalledOnce();
		expect(invoke).toHaveBeenCalledTimes(1);
		expect(view.container.querySelector(".viz-sync-chip")).toBeNull();
	});

	it("retains deliberate offline and reconnect actions without inventing confirmation", async () => {
		invoke.mockImplementation((command: string) =>
			Promise.resolve(command === "sync_status" ? status("synced") : []),
		);
		renderChip();
		fireEvent.click(
			await screen.findByRole("button", { name: "Sync with FOH: Synced" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Work offline" }));
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("set_sync_online", { online: false }),
		);
		act(() =>
			handlers.get("sync-status-changed")?.({
				payload: status("offline", { detail: "Working offline.", pending: 0 }),
			}),
		);
		const panel = screen.getByRole("dialog", { name: "Sync with FOH" });
		expect(
			within(panel).queryByText(/Saved to Control/u),
		).not.toBeInTheDocument();
		fireEvent.click(within(panel).getByRole("button", { name: "Reconnect" }));
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("set_sync_online", { online: true }),
		);
		expect(
			within(panel).queryByText(/Saved to Control/u),
		).not.toBeInTheDocument();
	});

	it("shows nothing for a document bound to no desk", async () => {
		invoke.mockResolvedValue(null);
		const { container } = renderChip();
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("sync_status"));
		expect(container.querySelector(".viz-sync-chip")).toBeNull();
	});

	it.each([
		"synced",
		"pending",
		"offline",
		"conflict",
		"error",
	] as const)("says %s in words and only claims Saved to Control when confirmed", async (state) => {
		invoke.mockImplementation((command: string) =>
			Promise.resolve(command === "sync_status" ? status(state) : []),
		);
		renderChip();
		const chip = await screen.findByRole("button", {
			name: `Sync with FOH: ${status(state).label}`,
		});
		expect(chip).toHaveClass(`is-${state}`);
		fireEvent.click(chip);
		const panel = await screen.findByRole("dialog", { name: "Sync with FOH" });
		expect(within(panel).getByText("Saved")).toBeInTheDocument();
		const claims = within(panel).queryAllByText(/Saved to Control/u);
		if (state === "synced") expect(claims.length).toBeGreaterThan(0);
		else expect(claims).toHaveLength(0);
	});

	it("follows status changes the engine announces", async () => {
		invoke.mockImplementation((command: string) =>
			Promise.resolve(command === "sync_status" ? status("synced") : []),
		);
		renderChip();
		await screen.findByRole("button", { name: "Sync with FOH: Synced" });
		await waitFor(() => expect(handlers.has("sync-status-changed")).toBe(true));
		act(() =>
			handlers.get("sync-status-changed")?.({ payload: status("pending") }),
		);
		expect(
			await screen.findByRole("button", { name: "Sync with FOH: 2 pending" }),
		).toBeInTheDocument();
	});

	it("keeps both drafts visible and resolves only on a deliberate choice", async () => {
		invoke.mockImplementation((command: string) => {
			if (command === "sync_status") return Promise.resolve(status("conflict"));
			if (command === "sync_conflicts") return Promise.resolve([conflict]);
			return Promise.resolve(undefined);
		});
		renderChip();
		fireEvent.click(
			await screen.findByRole("button", { name: "Sync with FOH: 1 conflict" }),
		);
		const section = await screen.findByRole("region", {
			name: "Sync conflict",
		});
		expect(
			within(section).getByText("Control: Front Truss · Yours: Back Truss"),
		).toBeInTheDocument();
		expect(invoke).not.toHaveBeenCalledWith(
			"resolve_sync_conflict",
			expect.anything(),
		);
		fireEvent.click(within(section).getByRole("button", { name: "Use mine" }));
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("resolve_sync_conflict", {
				entry: 7,
				resolution: "use_mine",
			}),
		);
	});
});
