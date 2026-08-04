import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClientSummary, ScreenConfiguration } from "../../api/types";
import { ScreensProvider } from "../../features/screens/ScreensContext";
import type { ScreensContextValue } from "../../features/screens/types";
import {
	DefaultScreenPicker,
	ProgrammerControlSurfaceSettings,
	ScreenSettingsCard,
} from "./ScreensSetup";

const configuredScreen: ScreenConfiguration = {
	id: "screen-1",
	name: "Screen 1",
	layout: { desks: [], activeDeskId: "main" },
	content: { type: "desktop" },
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

describe("programmer control surface settings", () => {
	it("offers exactly main or named screen ownership and four or six encoders", () => {
		const updateProgrammerControlSurface = vi.fn();
		const source: ScreensContextValue = {
			screens: {
				screens: [configuredScreen],
				active_pages: {},
				programmer_control_surface: {
					owner_screen_id: null,
					visible_encoders: 6,
				},
			},
			bootstrap: null,
			session: null,
			saveScreen: vi.fn(),
			deleteScreen: vi.fn(),
			setScreenPage: vi.fn(),
			updateProgrammerControlSurface,
			updateControlDesk: vi.fn(),
			selectControlDesk: vi.fn(),
			removeClient: vi.fn(),
		};
		render(
			<ScreensProvider source={source}>
				<ProgrammerControlSurfaceSettings />
			</ScreensProvider>,
		);

		expect(
			screen.getByRole("heading", { name: "Programmer control surface" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Main screen" }));
		expect(screen.getByRole("option", { name: "Screen 1" })).toBeVisible();
		fireEvent.click(screen.getByRole("option", { name: "Screen 1" }));
		expect(updateProgrammerControlSurface).toHaveBeenCalledWith({
			owner_screen_id: "screen-1",
		});

		fireEvent.click(screen.getByRole("button", { name: "Six" }));
		expect(screen.getByRole("option", { name: "Four" })).toBeVisible();
		expect(screen.getByRole("option", { name: "Six" })).toBeVisible();
		fireEvent.click(screen.getByRole("option", { name: "Four" }));
		expect(updateProgrammerControlSurface).toHaveBeenCalledWith({
			visible_encoders: 4,
		});
	});

	it("reports a closed owner and explicitly recovers controls to main", () => {
		const updateProgrammerControlSurface = vi.fn();
		const source: ScreensContextValue = {
			screens: {
				screens: [{ ...configuredScreen, desired_open: false }],
				active_pages: {},
				programmer_control_surface: {
					owner_screen_id: configuredScreen.id,
					visible_encoders: 4,
				},
			},
			bootstrap: null,
			session: null,
			saveScreen: vi.fn(),
			deleteScreen: vi.fn(),
			setScreenPage: vi.fn(),
			updateProgrammerControlSurface,
			updateControlDesk: vi.fn(),
			selectControlDesk: vi.fn(),
			removeClient: vi.fn(),
		};
		render(
			<ScreensProvider source={source}>
				<ProgrammerControlSurfaceSettings />
			</ScreensProvider>,
		);

		expect(screen.getByRole("alert")).toHaveTextContent("Screen 1 is closed");
		fireEvent.click(
			screen.getByRole("button", {
				name: "Return controls to main screen",
			}),
		);
		expect(updateProgrammerControlSurface).toHaveBeenCalledWith({
			assign_to_main: true,
		});
	});
});

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
			screen.getByRole("heading", { name: "Settings" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Placement" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("heading", { name: "Playbacks" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Configure Playbacks" }),
		).toBeInTheDocument();
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
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={save}
				remove={vi.fn()}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", { name: "Configure Playbacks" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Configure Playbacks" }),
		).toBeInTheDocument();
		const addRow = screen.getByRole("button", { name: "Add Row" });
		const saveAction = screen.getByRole("button", { name: "Save" });
		expect(addRow.parentElement).toHaveClass("ui-modal-title-actions");
		expect(saveAction.parentElement).toHaveClass("ui-modal-title-actions");
		expect(
			screen.queryByRole("button", { name: "Cancel" }),
		).not.toBeInTheDocument();
		fireEvent.click(addRow);
		expect(
			screen.getByRole("button", { name: "Remove row 2" }),
		).toBeInTheDocument();
		const secondRowHandle = screen.getByRole("button", {
			name: "Reorder playback row 2",
		});
		const firstRowHandle = screen.getByRole("button", {
			name: "Reorder playback row 1",
		});
		const firstRow = firstRowHandle.closest(".playback-row-configuration");
		expect(firstRow).not.toBeNull();
		expect(secondRowHandle).toHaveTextContent("⠿");
		expect(secondRowHandle).not.toHaveTextContent("Row 2");
		const elementFromPoint = document.elementFromPoint;
		Object.defineProperty(secondRowHandle, "setPointerCapture", {
			configurable: true,
			value: vi.fn(),
		});
		Object.defineProperty(document, "elementFromPoint", {
			configurable: true,
			value: vi.fn(() => firstRow),
		});
		fireEvent.pointerDown(secondRowHandle, {
			pointerId: 1,
			pointerType: "mouse",
			clientX: 20,
			clientY: 120,
		});
		fireEvent.pointerMove(secondRowHandle, {
			pointerId: 1,
			pointerType: "mouse",
			clientX: 20,
			clientY: 60,
		});
		Object.defineProperty(document, "elementFromPoint", {
			configurable: true,
			value: elementFromPoint,
		});
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
				rows: [{ first_playback_slot: 9 }, { first_playback_slot: 1 }],
			},
		});
	});

	it("offers only the fixed-pane allowlist and enforces the Dock constraint", async () => {
		const saved: ScreenConfiguration[] = [];
		const save = vi.fn(async (value: ScreenConfiguration) => {
			saved.push(value);
		});
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				cueLists={[{ id: "cue-list-id", name: "7 · Walk-in" }]}
				textFiles={[
					{
						root: "shows",
						rootLabel: "Shows",
						path: "notes/walk-in.md",
						name: "walk-in.md",
					},
				]}
				save={save}
				remove={vi.fn()}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
		fireEvent.click(
			screen.getByRole("option", { name: "Fixed full-screen pane" }),
		);

		const dock = screen.getByRole("switch", { name: "Dock" });
		expect(dock).toBeDisabled();
		expect(dock).not.toBeChecked();
		expect(screen.getByRole("button", { name: "Fixture Sheet" })).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Fixture Sheet" }));
		for (const label of [
			"Fixture Sheet",
			"Stage - 2D",
			"Stage - 3D",
			"Cues - Cuelist",
			"Text",
		])
			expect(screen.getByRole("option", { name: label })).toBeVisible();
		expect(screen.queryByRole("option", { name: "DMX output" })).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Off" }));
		for (const mode of ["Off", "Icon only", "Text only"])
			expect(screen.getByRole("option", { name: mode })).toBeVisible();
		for (const column of ["Intensity", "Shapers", "Control", "Media"])
			expect(screen.getByRole("switch", { name: column })).toBeVisible();

		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		expect(saved[0]).toMatchObject({
			content: {
				type: "fixed_pane",
				pane: { type: "fixture_sheet" },
			},
			show_dock: false,
			show_playbacks: true,
			show_page_controls: true,
		});
	});

	it("configures left and right fixed panes with an explicit pixel width", async () => {
		const saved: ScreenConfiguration[] = [];
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={async (value) => {
					saved.push(value);
				}}
				remove={vi.fn()}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
		const browserLink = screen.getByRole("link", {
			name: "Open browser view",
		});
		expect(new URL(browserLink.getAttribute("href") ?? "").searchParams.get("screen")).toBe(
			"screen-1",
		);
		fireEvent.click(screen.getByRole("option", { name: "Fixed right pane" }));
		const width = screen.getByRole("textbox", { name: "Pane width (px)" });
		expect(width).toHaveValue("420");
		fireEvent.change(width, { target: { value: "480" } });
		expect(screen.getByRole("switch", { name: "Dock" })).not.toBeDisabled();

		await waitFor(() => expect(saved.length).toBeGreaterThan(0));
			expect(saved.at(-1)).toMatchObject({
				content: {
					type: "fixed_side_pane",
					side: "right",
					width_px: 480,
					base: "desktop",
				},
				show_dock: true,
			});
		});

		it("assigns Programmer ownership when a fixed side pane uses a Control surface base", async () => {
			const updateProgrammerOwner = vi.fn().mockResolvedValue(undefined);
			const save = vi.fn().mockResolvedValue(undefined);
			render(
				<ScreenSettingsCard
					screen={{
						...configuredScreen,
						content: {
							type: "fixed_side_pane",
							pane: { type: "cues", cue_list_id: "" },
							side: "left",
							width_px: 420,
							base: "desktop",
						},
					}}
					displays={[]}
					save={save}
					remove={vi.fn()}
					updateProgrammerOwner={updateProgrammerOwner}
				/>,
			);

			const desktopButtons = screen.getAllByRole("button", { name: "Desktop" });
			fireEvent.click(desktopButtons.at(-1) as HTMLButtonElement);
			fireEvent.click(screen.getByRole("option", { name: "Control surface" }));

			await waitFor(() => expect(save).toHaveBeenCalledOnce());
			await waitFor(() =>
				expect(updateProgrammerOwner).toHaveBeenCalledWith({
					owner_screen_id: "screen-1",
				}),
			);
			expect(save.mock.calls[0][0]).toMatchObject({
				content: {
					type: "fixed_side_pane",
					side: "left",
					width_px: 420,
					base: "control_surface",
				},
				show_dock: false,
			});
		});

	it("assigns the single Programmer owner when Control surface is selected", async () => {
		const updateProgrammerOwner = vi.fn().mockResolvedValue(undefined);
		const save = vi.fn().mockResolvedValue(undefined);
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={save}
				remove={vi.fn()}
				updateProgrammerOwner={updateProgrammerOwner}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
		fireEvent.click(screen.getByRole("option", { name: "Control surface" }));

		await waitFor(() => expect(save).toHaveBeenCalledOnce());
		await waitFor(() =>
			expect(updateProgrammerOwner).toHaveBeenCalledWith({
				owner_screen_id: "screen-1",
			}),
		);
		expect(save.mock.calls[0][0]).toMatchObject({
			content: { type: "control_surface" },
			show_dock: false,
		});
	});

	it("returns Programmer ownership to main when leaving Control surface", async () => {
		const updateProgrammerOwner = vi.fn().mockResolvedValue(undefined);
		render(
			<ScreenSettingsCard
				screen={{
					...configuredScreen,
					content: { type: "control_surface" },
					show_dock: false,
				}}
				displays={[]}
				save={vi.fn().mockResolvedValue(undefined)}
				remove={vi.fn()}
				programmerOwner
				updateProgrammerOwner={updateProgrammerOwner}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Control surface" }));
		fireEvent.click(screen.getByRole("option", { name: "Desktop" }));

		await waitFor(() =>
			expect(updateProgrammerOwner).toHaveBeenCalledWith({
				assign_to_main: true,
			}),
		);
	});

	it("keeps missing fixed objects configured and leaves Dock off after returning to Desktop", async () => {
		const fixed: ScreenConfiguration = {
			...configuredScreen,
			content: {
				type: "fixed_pane",
				pane: { type: "cues", cue_list_id: "missing-cuelist" },
			},
			show_dock: false,
		};
		const saved: ScreenConfiguration[] = [];
		render(
			<ScreenSettingsCard
				screen={fixed}
				displays={[]}
				save={async (value) => {
					saved.push(value);
				}}
				remove={vi.fn()}
			/>,
		);

		expect(
			screen.getByRole("button", {
				name: "Configured Cuelist is unavailable",
			}),
		).toBeVisible();
		fireEvent.click(
			screen.getByRole("button", { name: "Fixed full-screen pane" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Desktop" }));

		await waitFor(() => expect(saved).toHaveLength(1));
		expect(saved[0]).toMatchObject({
			content: { type: "desktop" },
			show_dock: false,
		});
	});
});

describe("default screen picker", () => {
	const client = (
		id: string,
		name: string,
		connected: boolean,
		last: string | null,
		canRemove = !connected,
	): ClientSummary => ({
		client_id: id,
		name,
		connected,
		last_connected_at: last,
		can_remove: canRemove,
		desk: {
			id: `desk-${id}`,
			name: `${name} screen`,
			osc_alias: id,
			columns: 8,
			rows: 2,
			buttons: 3,
		},
	});

	it("groups and sorts authoritative presence while identifying current client and default separately", () => {
		const select = vi.fn();
		const close = vi.fn();
		const remove = vi.fn(async () => true);
		render(
			<DefaultScreenPicker
				clients={[
					client(
						"connected-old",
						"Connected old",
						true,
						"2026-07-15T10:00:00Z",
						false,
					),
					client("unknown", "Unknown", false, null),
					client(
						"historical-new",
						"Historical new",
						false,
						"2026-07-17T10:00:00Z",
						false,
					),
					client(
						"connected-new",
						"Connected new",
						true,
						"2026-07-17T11:00:00Z",
						false,
					),
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
		expect(rows.map((row) => row.querySelector("b")?.textContent)).toEqual([
			"Connected new",
			"Connected old",
			"Historical new",
			"Unknown",
		]);
		expect(screen.getByText("Current client")).toBeInTheDocument();
		expect(screen.getAllByText("Current default screen")).toHaveLength(2);
		expect(screen.getByText(/Last connected unknown/)).toBeInTheDocument();
		expect(
			screen
				.getAllByRole("button", { name: "Remove client" })
				.filter((button) => !button.hasAttribute("disabled")),
		).toHaveLength(1);
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
		render(
			<DefaultScreenPicker
				clients={[client("old", "Old wing", false, "2026-07-01T10:00:00Z")]}
				currentClientId="current"
				currentDeskId="desk-current"
				onSelect={vi.fn()}
				onRemove={remove}
				onClose={vi.fn()}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Remove client" }));
		const confirmation = screen.getByRole("alertdialog", {
			name: "Remove client Old wing?",
		});
		expect(confirmation).toHaveTextContent(
			"per-show page and playback selection, desk lock, Update defaults",
		);
		expect(confirmation).toHaveTextContent(
			"Portable shows, Virtual Playback assignments and exclusion zones, users, optional screens, other clients, and installation-wide configuration will not change",
		);
		fireEvent.click(
			screen.getAllByRole("button", { name: "Remove client" }).at(-1)!,
		);
		await waitFor(() => expect(remove).toHaveBeenCalledWith("desk-old"));
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"may have reconnected",
		);
	});
});
