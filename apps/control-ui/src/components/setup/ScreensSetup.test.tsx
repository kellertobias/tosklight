import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClientSummary, ScreenConfiguration } from "../../api/types";
import { DefaultScreenPicker, ScreenSettingsCard } from "./ScreensSetup";

const configuredScreen: ScreenConfiguration = {
	id: "screen-1",
	name: "Screen 1",
	layout: { desks: [], activeDeskId: "main" },
	show_dock: true,
	show_playbacks: true,
	playback_count: 8,
	playback_rows: 1,
	first_playback_slot: 1,
	page_mode: "follow_main",
	show_page_controls: true,
	desired_open: true,
	display_id: null,
	bounds: null,
	fullscreen: false,
};

afterEach(cleanup);

describe("additional screen settings", () => {
	it("updates fields immediately and serializes the saved configurations", async () => {
		const saved: ScreenConfiguration[] = [];
		const save = vi.fn(async (value: ScreenConfiguration) => {
			saved.push(value);
		});
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={save}
				remove={vi.fn()}
			/>,
		);

		expect(screen.getByRole("heading", { name: "Layout" })).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Placement" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Playbacks" }),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Configure Playbacks" })).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Remove Screen" }),
		).toBeInTheDocument();
		const name = screen.getByLabelText("Screen name");
		fireEvent.change(name, { target: { value: "Stage manager" } });
		expect(name).toHaveValue("Stage manager");
		fireEvent.click(screen.getByRole("button", { name: "Close Screen" }));

		await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
		expect(saved[0].name).toBe("Stage manager");
		expect(saved[1]).toMatchObject({
			name: "Stage manager",
			desired_open: false,
		});
	});

	it("configures playback rows and page mode in one save", async () => {
		const saved: ScreenConfiguration[] = [];
		const save = vi.fn(async (value: ScreenConfiguration) => {
			saved.push(value);
		});
		render(<ScreenSettingsCard screen={configuredScreen} displays={[]} save={save} remove={vi.fn()} />);

		fireEvent.click(screen.getByRole("button", { name: "Configure Playbacks" }));
		expect(screen.getByRole("dialog", { name: "Configure Playbacks" })).toBeInTheDocument();
		const addRow = screen.getByRole("button", { name: "Add Row" });
		const saveAction = screen.getByRole("button", { name: "Save" });
		expect(addRow.parentElement).toHaveClass("ui-modal-title-actions");
		expect(saveAction.parentElement).toHaveClass("ui-modal-title-actions");
		expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
		fireEvent.click(addRow);
		expect(screen.getByRole("button", { name: "Remove row 2" })).toBeInTheDocument();
		const secondRowHandle = screen.getByRole("button", { name: "Reorder playback row 2" });
		const firstRowHandle = screen.getByRole("button", { name: "Reorder playback row 1" });
		const firstRow = firstRowHandle.closest(".playback-row-configuration");
		expect(firstRow).not.toBeNull();
		expect(secondRowHandle).toHaveTextContent("⠿");
		expect(secondRowHandle).not.toHaveTextContent("Row 2");
		const elementFromPoint = document.elementFromPoint;
		Object.defineProperty(secondRowHandle, "setPointerCapture", { configurable: true, value: vi.fn() });
		Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => firstRow) });
		fireEvent.pointerDown(secondRowHandle, { pointerId: 1, pointerType: "mouse", clientX: 20, clientY: 120 });
		fireEvent.pointerMove(secondRowHandle, { pointerId: 1, pointerType: "mouse", clientX: 20, clientY: 60 });
		Object.defineProperty(document, "elementFromPoint", { configurable: true, value: elementFromPoint });
		fireEvent.click(screen.getByRole("button", { name: "Follow Main" }));
		fireEvent.click(screen.getByRole("option", { name: "Dedicated Page" }));
		fireEvent.click(saveAction);

		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		expect(saved[0]).toMatchObject({
			page_mode: "independent",
			playback_count: 16,
			playback_rows: 2,
			playback_layout: {
				playbacks_per_row: 8,
				rows: [
					{ first_playback_slot: 9 },
					{ first_playback_slot: 1 },
				],
			},
		});
	});
});

describe("default screen picker", () => {
	const client = (id: string, name: string, connected: boolean, last: string | null, canRemove = !connected): ClientSummary => ({
		client_id: id,
		name,
		connected,
		last_connected_at: last,
		can_remove: canRemove,
		desk: { id: `desk-${id}`, name: `${name} screen`, osc_alias: id, columns: 8, rows: 2, buttons: 3 },
	});

	it("groups and sorts authoritative presence while identifying current client and default separately", () => {
		const select = vi.fn();
		const close = vi.fn();
		const remove = vi.fn(async () => true);
		render(
			<DefaultScreenPicker
				clients={[
					client("connected-old", "Connected old", true, "2026-07-15T10:00:00Z", false),
					client("unknown", "Unknown", false, null),
					client("historical-new", "Historical new", false, "2026-07-17T10:00:00Z", false),
					client("connected-new", "Connected new", true, "2026-07-17T11:00:00Z", false),
				]}
				currentClientId="connected-old"
				currentDeskId="desk-historical-new"
				onSelect={select}
				onRemove={remove}
				onClose={close}
			/>,
		);

		expect(
			screen.getByRole("heading", { name: "Choose default screen" }),
		).toBeInTheDocument();
		const rows = screen.getAllByRole("article");
		expect(rows.map((row) => row.querySelector("b")?.textContent)).toEqual(["Connected new", "Connected old", "Historical new", "Unknown"]);
		expect(screen.getByText("Current client")).toBeInTheDocument();
		expect(screen.getAllByText("Current default screen")).toHaveLength(2);
		expect(screen.getByText(/Last connected unknown/)).toBeInTheDocument();
		expect(screen.getAllByRole("button", { name: "Remove client" }).filter((button) => !button.hasAttribute("disabled"))).toHaveLength(1);
		fireEvent.click(
			screen.getAllByRole("button", { name: "Use as default screen" })[0],
		);
		expect(select).toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Close default screen chooser" }),
		);
		expect(close).toHaveBeenCalledOnce();
	});

	it("requires named confirmation and reports a reconnect race without removing other state claims", async () => {
		const remove = vi.fn(async () => false);
		render(<DefaultScreenPicker clients={[client("old", "Old wing", false, "2026-07-01T10:00:00Z")]} currentClientId="current" currentDeskId="desk-current" onSelect={vi.fn()} onRemove={remove} onClose={vi.fn()}/>);
		fireEvent.click(screen.getByRole("button", { name: "Remove client" }));
		const confirmation = screen.getByRole("alertdialog", { name: "Remove client Old wing?" });
		expect(confirmation).toHaveTextContent("per-show page and playback selection, desk lock, Update defaults, and virtual-playback exclusion settings");
		expect(confirmation).toHaveTextContent("Portable shows, users, optional screens, other clients, and installation-wide configuration will not change");
		fireEvent.click(screen.getAllByRole("button", { name: "Remove client" }).at(-1)!);
		await waitFor(() => expect(remove).toHaveBeenCalledWith("desk-old"));
		expect(await screen.findByRole("alert")).toHaveTextContent("may have reconnected");
	});
});
