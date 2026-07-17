import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
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
		fireEvent.click(screen.getByRole("button", { name: "+ Add playback row" }));
		fireEvent.click(screen.getByRole("button", { name: "Follow Main" }));
		fireEvent.click(screen.getByRole("option", { name: "Dedicated Page" }));
		fireEvent.click(screen.getByRole("button", { name: "Save playback configuration" }));

		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		expect(saved[0]).toMatchObject({
			page_mode: "independent",
			playback_count: 16,
			playback_rows: 2,
			playback_layout: { playbacks_per_row: 8 },
		});
	});
});

describe("default screen picker", () => {
	it("marks the current client and selects another known client", () => {
		const select = vi.fn();
		const close = vi.fn();
		render(
			<DefaultScreenPicker
				desks={[
					{
						id: "desk-a",
						name: "Main client",
						osc_alias: "main",
						columns: 8,
						rows: 2,
						buttons: 3,
					},
					{
						id: "desk-b",
						name: "Backup client",
						osc_alias: "backup",
						columns: 6,
						rows: 1,
						buttons: 2,
					},
				]}
				currentDeskId="desk-a"
				onSelect={select}
				onClose={close}
			/>,
		);

		expect(
			screen.getByRole("heading", { name: "Choose default screen" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Current default screen" }),
		).toBeDisabled();
		fireEvent.click(
			screen.getByRole("button", { name: "Use as default screen" }),
		);
		expect(select).toHaveBeenCalledWith("desk-b");
		fireEvent.click(
			screen.getByRole("button", { name: "Close default screen chooser" }),
		);
		expect(close).toHaveBeenCalledOnce();
	});
});
