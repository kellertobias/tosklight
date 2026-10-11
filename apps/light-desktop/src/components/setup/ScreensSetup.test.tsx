import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClientSummary, ScreenConfiguration } from "../../api/types";
import { DefaultScreenSettings } from "./screens/ScreenSettingsCards";
import { ScreensProvider } from "../../features/screens/ScreensContext";
import type { ScreensContextValue } from "../../features/screens/types";
import {
	KnownClientsModal,
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
	surface_id: "screen:screen-1",
	show_page_controls: true,
	show_programmer: false,
	not_editable: false,
	desired_open: true,
	display_id: null,
	bounds: null,
	fullscreen: false,
};

const surfaceSnapshot = {
	selected_surface_id: "main",
	command_page: 1,
	resolved_pages: { main: 1, "screen:screen-1": 1 },
	playback_surfaces: {
		main: {
			name: "Main",
			page_source: { type: "own" as const, page: 1 },
			layout: {
				rows: [
					{
						first_position: 1,
						count: 15,
						presentation: "fader" as const,
						button_count: 3,
					},
				],
			},
		},
		"screen:screen-1": {
			name: "Screen 1",
			page_source: { type: "follow" as const, surface_id: "main" },
			layout: {
				rows: [
					{
						first_position: 201,
						count: 10,
						presentation: "buttons" as const,
						button_count: 1,
					},
				],
			},
		},
	},
};
afterEach(cleanup);

describe("programmer control surface settings", () => {
	it("offers exactly main or named screen ownership and four, five or six encoders", () => {
		const updateProgrammerControlSurface = vi.fn();
		const source: ScreensContextValue = {
			screens: {
				screens: [configuredScreen],
				surfaces: surfaceSnapshot,
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
			surfaces: surfaceSnapshot,
			setSurfacePage: vi.fn(),
			selectSurface: vi.fn(),
			updatePlaybackSurface: vi.fn(),
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
		expect(screen.getByRole("option", { name: "Five" })).toBeVisible();
		expect(screen.getByRole("option", { name: "Six" })).toBeVisible();
		fireEvent.click(screen.getByRole("option", { name: "Four" }));
		expect(updateProgrammerControlSurface).toHaveBeenCalledWith({
			visible_encoders: 4,
		});
		fireEvent.click(screen.getByRole("button", { name: "Six" }));
		fireEvent.click(screen.getByRole("option", { name: "Five" }));
		expect(updateProgrammerControlSurface).toHaveBeenCalledWith({ visible_encoders: 5 });
	});

	it("reports a closed owner and explicitly recovers controls to main", () => {
		const updateProgrammerControlSurface = vi.fn();
		const source: ScreensContextValue = {
			screens: {
				screens: [{ ...configuredScreen, desired_open: false }],
				surfaces: surfaceSnapshot,
				programmer_control_surface: {
					owner_screen_id: configuredScreen.id,
					visible_encoders: 4,
				},
			},
			bootstrap: null,
			session: null,
			saveScreen: vi.fn(),
			deleteScreen: vi.fn(),
			surfaces: surfaceSnapshot,
			setSurfacePage: vi.fn(),
			selectSurface: vi.fn(),
			updatePlaybackSurface: vi.fn(),
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

	it("defaults compact encoder assignments off and persists the optional-screen toggle", async () => {
		const save = vi.fn(async (_value: ScreenConfiguration) => undefined);
		render(<ScreenSettingsCard screen={configuredScreen} displays={[]} save={save} remove={vi.fn()} />);
		openScreenConfiguration();
		const toggle = screen.getByLabelText("Compact encoder assignments");
		expect(toggle).not.toBeChecked();
		fireEvent.click(toggle);
		await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ show_compact_encoders: true })));
		expect(toggle).toBeChecked();
	});

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

	it("removes a screen only after the saves queued before Remove have landed", async () => {
		const order: string[] = [];
		let finishSave: () => void = () => undefined;
		const save = vi.fn(
			(value: ScreenConfiguration) =>
				new Promise<void>((resolve) => {
					finishSave = () => {
						order.push(`save open=${value.desired_open}`);
						resolve();
					};
				}),
		);
		const remove = vi.fn(async () => {
			order.push("remove");
		});
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={save}
				remove={remove}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Close Screen" }));
		fireEvent.click(screen.getByRole("button", { name: "Remove Screen" }));
		await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
		expect(remove).not.toHaveBeenCalled();
		finishSave();
		await waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
		expect(order).toEqual(["save open=false", "remove"]);
	});

	it("lays the Layout switches out as one form without framed sections", () => {
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={vi.fn(async () => {})}
				remove={vi.fn()}
			/>,
		);

		openScreenConfiguration();
		const dialog = screen.getByRole("dialog", { name: "Configure Screen 1" });
		const form = dialog.querySelector(".screen-configuration-modal-content");
		expect(form).toHaveAttribute("data-tab", "layout");
		expect(dialog.querySelector("section section, h3")).toBeNull();
		const names = ["Dock", "Playbacks", "Command line", "Programming"].map(
			(name) => screen.getByRole("switch", { name }),
		);
		// The switches remain direct children of the same screen configuration form.
		for (const control of names)
			expect(control.closest(".ui-form-field")?.parentElement).toBe(form);
		expect(
			names.map((control) =>
				[...(form?.children ?? [])].indexOf(
					control.closest(".ui-form-field") as Element,
				),
			),
		).toEqual([2, 3, 4, 6]);
		const pageControls = screen
			.getByRole("switch", { name: "Page controls" })
			.closest(".ui-form-field");
		expect(pageControls).toHaveClass("screen-configuration-wide");
		expect(
			screen.getByRole("button", { name: "Desktop" }).closest(".ui-form-field"),
		).toHaveClass("screen-configuration-wide");

		fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
		expect(form).toHaveAttribute("data-tab", "settings");
		expect(form?.firstElementChild).toHaveClass("screen-settings-note");
		expect(dialog.querySelector("section section, h3")).toBeNull();
	});

	it("reports a rejected save and keeps saving later changes", async () => {
		const save = vi
			.fn<(value: ScreenConfiguration) => Promise<void>>()
			.mockRejectedValueOnce(new Error("invalid screen configuration"))
			.mockResolvedValue(undefined);
		render(
			<ScreenSettingsCard
				screen={configuredScreen}
				displays={[]}
				save={save}
				remove={vi.fn()}
			/>,
		);

		openScreenConfiguration("Placement");
		fireEvent.change(screen.getByLabelText("Window X"), {
			target: { value: "20" },
		});
		expect(
			await screen.findByText(
				"Could not save this screen: invalid screen configuration",
			),
		).toHaveAttribute("role", "alert");
		fireEvent.change(screen.getByLabelText("Window X"), {
			target: { value: "30" },
		});
		await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
		await waitFor(() =>
			expect(screen.queryByText(/Could not save this screen/)).toBeNull(),
		);
		expect(save.mock.calls[1][0].bounds).toMatchObject({ x: 30 });
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

describe("known windows", () => {
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
			id: "the-one-desk",
			surface_id: "main",
			name: "Front of house",
			columns: 8,
			rows: 2,
			buttons: 3,
		},
	});

	it("groups and sorts authoritative presence and names this window", () => {
		const close = vi.fn();
		const remove = vi.fn(async () => true);
		render(
			<KnownClientsModal
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
				onRemove={remove}
				onRemoveAll={vi.fn(async () => true)}
				onClose={close}
			/>,
		);

		expect(
			screen.getByRole("heading", { name: "Known windows" }),
		).toBeInTheDocument();
		const rows = screen.getAllByRole("article");
		// Every window shares the one desk, so each is named by its own identity.
		expect(rows.map((row) => row.querySelector("b")?.textContent)).toEqual([
			"Window connecte",
			"Window connecte",
			"Window historic",
			"Window unknown",
		]);
		expect(screen.getByText("This window")).toBeInTheDocument();
		expect(screen.queryByText(/default screen/i)).toBeNull();
		expect(screen.getByText(/Last connected unknown/)).toBeInTheDocument();
		expect(
			screen
				.getAllByRole("button", { name: "Forget window" })
				.filter((button) => !button.hasAttribute("disabled")),
		).toHaveLength(2);
		fireEvent.click(
			screen.getByRole("button", { name: "Close known windows" }),
		);
		expect(close).toHaveBeenCalledOnce();
	});

	it("requires named confirmation and reports a reconnect race", async () => {
		const remove = vi.fn(async () => false);
		render(
			<KnownClientsModal
				clients={[client("old", "Old wing", false, "2026-07-01T10:00:00Z")]}
				currentClientId="current"
				onRemove={remove}
				onRemoveAll={vi.fn(async () => true)}
				onClose={vi.fn()}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Forget window" }));
		const confirmation = screen.getByRole("alertdialog", {
			name: "Forget Window old?",
		});
		expect(confirmation).toHaveTextContent(
			"only a record that the window has connected before",
		);
		expect(confirmation).toHaveTextContent("The desk itself is untouched");
		const confirmRemove = screen
			.getAllByRole("button", { name: "Forget window" })
			.at(-1);
		expect(confirmRemove).toBeDefined();
		if (confirmRemove) fireEvent.click(confirmRemove);
		await waitFor(() =>
			expect(remove).toHaveBeenCalledWith(
				expect.objectContaining({ client_id: "old" }),
			),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"may have reconnected",
		);
	});

	it("requires confirmation before forgetting every other window", async () => {
		const removeAll = vi.fn(async () => true);
		render(
			<KnownClientsModal
				clients={[
					client("current", "Current", true, "2026-07-18T10:00:00Z", false),
					client("connected", "Connected", true, "2026-07-18T09:00:00Z", false),
					client("old", "Old wing", false, "2026-07-01T10:00:00Z"),
				]}
				currentClientId="current"
				onRemove={vi.fn(async () => true)}
				onRemoveAll={removeAll}
				onClose={vi.fn()}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", { name: "Forget other windows" }),
		);
		const confirmation = screen.getByRole("alertdialog", {
			name: "Forget other windows?",
		});
		expect(confirmation).toHaveTextContent(
			"This window and connected windows are left alone",
		);
		expect(removeAll).not.toHaveBeenCalled();
		const confirmRemoveAll = screen
			.getAllByRole("button", { name: "Forget other windows" })
			.at(-1);
		expect(confirmRemoveAll).toBeDefined();
		if (confirmRemoveAll) fireEvent.click(confirmRemoveAll);
		await waitFor(() => expect(removeAll).toHaveBeenCalledOnce());
		expect(
			screen.queryByRole("alertdialog", { name: "Forget other windows?" }),
		).toBeNull();
	});
});


describe("default screen compact encoder settings", () => {
	it("shows a touchable option and delegates changing the main screen preference", () => {
		const change = vi.fn();
		render(<DefaultScreenSettings deskName="Main" onDeskName={vi.fn()} onTextFocus={vi.fn()} onTextBlur={vi.fn()} keyboardShortcuts={false} onKeyboardShortcuts={vi.fn()} onConfigurePlaybacks={vi.fn()} onChooseDefault={vi.fn()} singleClientMode={false} onSingleClientMode={vi.fn()} showCompactEncoders={false} onShowCompactEncoders={change} />);
		const toggle = screen.getByLabelText("Compact encoder assignments");
		expect(toggle).not.toBeChecked();
		fireEvent.click(toggle);
		expect(change).toHaveBeenCalledWith(true);
	});
});
