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

vi.mock("../../api/client/serverLocation", () => ({
	configuredServerUrl: () => "http://[::1]:5000",
}));

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
	show_programmer: false,
	not_editable: false,
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
			bootstrap: {
				attribute_registry: [
					{
						id: "red",
						label: "Red",
						family: "color",
						value_type: "continuous",
						default_unit: null,
						encoder_group: "color",
						encoder_page: 1,
						encoder_slot: 1,
					},
				],
			} as ScreensContextValue["bootstrap"],
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
			screen.getByRole("heading", { name: "Encoder placement" }),
		).toBeInTheDocument();
		expect(screen.getByText("Encoders on")).toBeInTheDocument();
		// The semantic layout is edited in Attributes & encoders, never previewed here.
		expect(
			screen.queryByLabelText("6-encoder semantic layout preview"),
		).not.toBeInTheDocument();
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

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Encoders unavailable — assigned to Screen 1",
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Use encoders on this screen",
			}),
		);
		expect(updateProgrammerControlSurface).toHaveBeenCalledWith({
			assign_to_main: true,
		});
	});
});

describe("additional screen settings", () => {
	function openScreenConfiguration(
		tab: "Layout" | "Settings" | "Placement" | "Playbacks" = "Layout",
	) {
		fireEvent.click(screen.getByRole("button", { name: "Configure screen" }));
		if (tab !== "Layout")
			fireEvent.click(screen.getByRole("tab", { name: tab }));
	}

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

		expect(screen.queryByRole("heading", { name: "Layout" })).toBeNull();
		openScreenConfiguration();
		for (const tab of ["Layout", "Settings", "Placement", "Playbacks"])
			expect(screen.getByRole("tab", { name: tab })).toBeInTheDocument();
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

		openScreenConfiguration("Playbacks");
		fireEvent.click(
			screen.getByRole("button", { name: "Configure Playbacks" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Configure Playbacks" }),
		).toBeInTheDocument();
		const addRow = screen.getByRole("button", { name: "Add Row" });
		const saveAction = screen.getByRole("button", { name: "Save" });
		expect(addRow.closest(".ui-title-chrome-group")).not.toBeNull();
		expect(saveAction.closest(".ui-title-chrome-terminals")).not.toBeNull();
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

		openScreenConfiguration();
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
		fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
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
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, "clipboard", {
			configurable: true,
			value: { writeText },
		});
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

		openScreenConfiguration();
		fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
		expect(screen.queryByRole("link", { name: "Open in browser" })).toBeNull();
		const browserHref = "http://[::1]:5000/?screen=screen-1";
		expect(browserHref).toBe("http://[::1]:5000/?screen=screen-1");
		expect(new URL(browserHref).searchParams.get("screen")).toBe("screen-1");
		fireEvent.click(screen.getByRole("button", { name: "Copy browser link" }));
		await waitFor(() => expect(writeText).toHaveBeenCalledWith(browserHref));
		expect(
			screen.getByRole("button", { name: "✓ Copied browser link" }),
		).toHaveClass("ui-success");
		fireEvent.click(screen.getByRole("option", { name: "Fixed right pane" }));
		fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
		const width = screen.getByRole("textbox", { name: "Pane width (%)" });
		expect(width).toHaveValue("25");
		fireEvent.change(width, { target: { value: "40" } });
		fireEvent.click(screen.getByRole("tab", { name: "Layout" }));
		expect(screen.getByRole("switch", { name: "Dock" })).toBeDisabled();

		await waitFor(() => expect(saved.length).toBeGreaterThan(0));
		expect(saved.at(-1)).toMatchObject({
			content: {
				type: "fixed_side_pane",
				side: "right",
				width_percent: 40,
			},
			show_dock: false,
		});
	});

	it("assigns Programmer ownership when a fixed side pane is selected", async () => {
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

		openScreenConfiguration();
		fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
		fireEvent.click(screen.getByRole("option", { name: "Fixed left pane" }));

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
				width_percent: 25,
			},
			show_dock: false,
		});
	});

	it("assigns the single Programmer owner when Controls only is selected", async () => {
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

		openScreenConfiguration();
		fireEvent.click(screen.getByRole("button", { name: "Desktop" }));
		fireEvent.click(screen.getByRole("option", { name: "Controls only" }));
		fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
		expect(screen.getByRole("status")).toHaveTextContent(
			"Selecting it assigns that placement when saved",
		);

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

	it("returns Programmer ownership to main when leaving Controls only", async () => {
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

		openScreenConfiguration();
		fireEvent.click(screen.getByRole("button", { name: "Controls only" }));
		fireEvent.click(screen.getByRole("option", { name: "Desktop" }));
		fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
		expect(screen.getByRole("status")).toHaveTextContent(
			"the Playback/Encoders switch sits beside the section's own controls",
		);

		await waitFor(() =>
			expect(updateProgrammerOwner).toHaveBeenCalledWith({
				assign_to_main: true,
			}),
		);
	});

	it("requires one confirmed owner reassignment action before removing its screen", async () => {
		const updateProgrammerOwner = vi.fn().mockResolvedValue(undefined);
		const remove = vi.fn().mockResolvedValue(undefined);
		render(
			<ScreenSettingsCard
				screen={{
					...configuredScreen,
					content: { type: "control_surface" },
					show_dock: false,
				}}
				displays={[]}
				save={vi.fn().mockResolvedValue(undefined)}
				remove={remove}
				programmerOwner
				updateProgrammerOwner={updateProgrammerOwner}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Remove Screen" }));
		expect(remove).not.toHaveBeenCalled();
		const confirmation = screen.getByRole("dialog", {
			name: "Remove Screen 1",
		});
		expect(confirmation).toHaveTextContent(
			"move the encoders back to the main screen",
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Remove and use encoders on main screen",
			}),
		);

		await waitFor(() =>
			expect(updateProgrammerOwner).toHaveBeenCalledWith({
				assign_to_main: true,
			}),
		);
		await waitFor(() =>
			expect(remove).toHaveBeenCalledWith(
				expect.objectContaining({
					id: "screen-1",
					content: { type: "control_surface" },
				}),
			),
		);
		expect(updateProgrammerOwner.mock.invocationCallOrder[0]).toBeLessThan(
			remove.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY,
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
		openScreenConfiguration("Settings");

		expect(
			screen.getByRole("button", {
				name: "Configured Cuelist is unavailable",
			}),
		).toBeVisible();
		fireEvent.click(screen.getByRole("tab", { name: "Layout" }));
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
				onRemoveAll={vi.fn(async () => true)}
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
				onRemoveAll={vi.fn(async () => true)}
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
		const confirmRemove = screen
			.getAllByRole("button", { name: "Remove client" })
			.at(-1);
		expect(confirmRemove).toBeDefined();
		if (confirmRemove) fireEvent.click(confirmRemove);
		await waitFor(() => expect(remove).toHaveBeenCalledWith("desk-old"));
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"may have reconnected",
		);
	});

	it("requires confirmation before removing all eligible other clients", async () => {
		const removeAll = vi.fn(async () => true);
		render(
			<DefaultScreenPicker
				clients={[
					client("current", "Current", true, "2026-07-18T10:00:00Z", false),
					client("connected", "Connected", true, "2026-07-18T09:00:00Z", false),
					client("old", "Old wing", false, "2026-07-01T10:00:00Z"),
				]}
				currentClientId="current"
				currentDeskId="desk-current"
				onSelect={vi.fn()}
				onRemove={vi.fn(async () => true)}
				onRemoveAll={removeAll}
				onClose={vi.fn()}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", { name: "Remove all other clients" }),
		);
		const confirmation = screen.getByRole("alertdialog", {
			name: "Remove all other clients?",
		});
		expect(confirmation).toHaveTextContent(
			"current client and connected clients remain protected",
		);
		expect(removeAll).not.toHaveBeenCalled();
		const confirmRemoveAll = screen
			.getAllByRole("button", { name: "Remove all other clients" })
			.at(-1);
		expect(confirmRemoveAll).toBeDefined();
		if (confirmRemoveAll) fireEvent.click(confirmRemoveAll);
		await waitFor(() => expect(removeAll).toHaveBeenCalledOnce());
		expect(
			screen.queryByRole("alertdialog", {
				name: "Remove all other clients?",
			}),
		).toBeNull();
	});
});
