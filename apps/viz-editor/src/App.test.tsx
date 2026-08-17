import * as dialog from "@tauri-apps/plugin-dialog";
import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import "./styles.css";

/** One profile, shaped as the document core returns it. */
const PROFILE_ID = "11111111-1111-4111-8111-111111111111";
const MODE_ID = "22222222-2222-4222-8222-222222222222";
const SHOW_ID = "33333333-3333-4333-8333-333333333333";

const document = {
	showId: SHOW_ID,
	name: "Planning show",
	path: "/tmp/planning.show",
	fixtureCount: 0,
};

const snapshot = {
	showId: SHOW_ID,
	showRevision: 1,
	patchRevision: 1,
	cursor: 0,
	fixtures: [],
	profileRevisions: [],
};

const cadEntity = {
	id: PROFILE_ID,
	name: "Planning Wash 1",
	fixtureNumber: 1,
	fixtureDisplayId: "1",
	dmxAddress: "1.1",
	kind: "wash",
	fixtureType: "wash",
	drawingId: "wash:1",
	layerId: "house",
	selectable: true,
	positionMillimetres: [0, 0, 4000] as [number, number, number],
	rotationDegrees: [0, 0, 0] as [number, number, number],
	sizeMillimetres: [400, 500, 700] as [number, number, number],
	outputDirection: [0, 1, 0] as [number, number, number],
};

const liveInputs = { schemaVersion: 1 as const, mappings: [] };
const rendererSettings = {
	source: "lighting_desk" as const,
	host: "127.0.0.1",
	port: 5000,
	user: "Operator",
	quality: null,
	fog: 0.15,
	persistence: 0,
	persistenceFalloff: 3,
	ambient: 0.06,
	exposure: 1,
	laserBrightness: 1,
	lampFogCloudiness: 0.35,
	lampFogTurbulence: 0.35,
	laserFogCloudiness: 0.35,
	laserFogTurbulence: 0.35,
	crowdAmount: 1,
	theme: "light_on_dark" as const,
	background: null,
	showLabels: true,
	showSelection: true,
	floorGrid: null,
	blender: "",
	inputOverrides: [],
};

const invoke = vi.hoisted(() => vi.fn());
const eventHandlers = vi.hoisted(
	() => new Map<string, (event: { payload: unknown }) => void>(),
);
const nativeWindow = vi.hoisted(() => ({
	close: vi.fn().mockResolvedValue(undefined),
	isFullscreen: vi.fn().mockResolvedValue(false),
	setFullscreen: vi.fn().mockResolvedValue(undefined),
	startDragging: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn(
		(event: string, handler: (event: { payload: unknown }) => void) => {
			eventHandlers.set(event, handler);
			return Promise.resolve(() => eventHandlers.delete(event));
		},
	),
}));
vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: () => nativeWindow,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
	open: vi.fn(),
	save: vi.fn(),
}));

function libraryProfile() {
	return {
		id: PROFILE_ID,
		revision: 1,
		manufacturer: "Acme",
		name: "Planning Wash",
		// Shaped as `light_fixture::FixtureProfile` serializes, which is what the command returns.
		profile: {
			schema_version: 2,
			id: PROFILE_ID,
			revision: 1,
			manufacturer: "Acme",
			name: "Planning Wash",
			short_name: "Wash",
			fixture_type: "wash",
			patch_policy: "dmx",
			notes: "",
			physical: {},
			hazardous: false,
			model_asset: null,
			stage_icon_asset: null,
			direct_control_protocols: [],
			signal_loss_policy: "hold",
			modes: [
				{
					id: MODE_ID,
					name: "Default",
					notes: "",
					splits: [{ number: 1, footprint: 1 }],
					heads: [],
					channels: [],
					color_systems: [],
				},
			],
		},
	};
}

function typedProfile(
	manufacturer: string,
	name: string,
	fixtureType: string,
	suffix: string,
) {
	const base = libraryProfile();
	return {
		...base,
		id: `11111111-1111-4111-8111-1111111111${suffix}`,
		manufacturer,
		name,
		profile: {
			...base.profile,
			id: `11111111-1111-4111-8111-1111111111${suffix}`,
			manufacturer,
			name,
			fixture_type: fixtureType,
			modes: base.profile.modes.map((mode) => ({
				...mode,
				id: `22222222-2222-4222-8222-2222222222${suffix}`,
			})),
		},
	};
}

function renderApp(children: ReactNode = <App />) {
	return render(<ModalProvider>{children}</ModalProvider>);
}

beforeEach(() => {
	invoke.mockReset();
	eventHandlers.clear();
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
	vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
	for (const action of Object.values(nativeWindow)) action.mockClear();
	invoke.mockImplementation((command: string) => {
		switch (command) {
			case "document_summary":
				return Promise.resolve(document);
			case "library_profiles":
				return Promise.resolve([libraryProfile()]);
			case "patch_snapshot":
				return Promise.resolve(snapshot);
			case "discovered_desks":
				return Promise.resolve([]);
			case "live_dmx_inputs":
				return Promise.resolve(liveInputs);
			case "visualizer_is_running":
				return Promise.resolve(false);
			case "patch_layers":
				return Promise.resolve([
					{ id: "house", name: "House", order: 0 },
					{ id: "floor", name: "Floor", order: 1 },
				]);
			case "cad_scene_snapshot":
				return Promise.resolve({
					showId: SHOW_ID,
					sceneRevision: 1,
					selectionRevision: 0,
					entities: [cadEntity],
					drawings: [],
					selectedIds: [],
					attachments: [],
				});
			default:
				return Promise.reject(new Error(`unexpected command ${command}`));
		}
	});
});

describe("the Viz editor window", () => {
	it("shows a live, read-only, left-rotated top plan with the show name", async () => {
		renderApp();
		const overview = await screen.findByRole("img", {
			name: "Read-only rig overview for Planning show",
		});
		expect(overview).toHaveAttribute("data-view", "top_down");
		expect(overview).toHaveAttribute("data-rotation-quarter-turns", "-1");
		expect(overview).toHaveAttribute("data-entity-count", "1");
		expect(
			screen.getByText("Planning show", { selector: "strong" }),
		).toBeVisible();

		eventHandlers.get("cad-scene-delta")?.({
			payload: {
				sceneRevision: 2,
				upserted: [cadEntity, { ...cadEntity, id: "fixture-two" }],
				drawings: [],
				removedIds: [],
				attachments: [],
			},
		});
		await waitFor(() =>
			expect(overview).toHaveAttribute("data-entity-count", "2"),
		);
	});

	it("shows the desk's patch sheet over the open document", async () => {
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Patch" }));
		await screen.findByRole("columnheader", { name: "Fixture ID" });
		expect(
			document_root()?.querySelector(".show-patch-layout .ui-window-title"),
		).toHaveTextContent("Patch");
		expect(
			document_root()?.querySelector(".viz-native-window-title"),
		).toBeNull();
		expect(
			await screen.findByText("0 fixtures · 2 layers"),
		).toBeInTheDocument();
		const patchTitle = document_root()?.querySelector<HTMLElement>(
			".show-patch-layout .ui-window-title",
		);
		if (!patchTitle) throw new Error("patch title was not rendered");
		nativeWindow.startDragging.mockClear();
		fireEvent.pointerDown(patchTitle, { button: 0 });
		await waitFor(() =>
			expect(nativeWindow.startDragging).toHaveBeenCalledOnce(),
		);
		fireEvent.pointerDown(screen.getByRole("button", { name: "+ Add layer" }), {
			button: 0,
		});
		expect(nativeWindow.startDragging).toHaveBeenCalledOnce();
	});

	it("uses borderless native chrome and the operator sidebar", async () => {
		renderApp();
		const icon = await screen.findByAltText("ToskLight PreViz");
		const identity = icon.parentElement;
		if (!identity) throw new Error("visualizer identity was not rendered");
		expect(identity).toHaveAttribute("data-tauri-drag-region");
		expect(screen.getByText("planning.show")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Close window" }),
		).not.toHaveAttribute("data-tauri-drag-region");
		expect(
			screen.getByRole("button", { name: "Enter fullscreen" }),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Move window" })).toHaveAttribute(
			"data-tauri-drag-region",
		);
		const title = document_root()?.querySelector<HTMLElement>(
			".viz-show-settings-workspace > .ui-window-header",
		);
		if (!title) throw new Error("window title was not rendered");
		expect(title).toHaveTextContent("Show");
		expect(title).toHaveAttribute("data-tauri-drag-region");
		fireEvent.pointerDown(title, { button: 0 });
		await waitFor(() =>
			expect(nativeWindow.startDragging).toHaveBeenCalledOnce(),
		);
		nativeWindow.startDragging.mockClear();
		fireEvent.pointerDown(identity, { button: 0 });
		await waitFor(() =>
			expect(nativeWindow.startDragging).toHaveBeenCalledOnce(),
		);
		fireEvent.click(screen.getByRole("button", { name: "Enter fullscreen" }));
		await waitFor(() =>
			expect(nativeWindow.setFullscreen).toHaveBeenCalledWith(true),
		);
		fireEvent.click(screen.getByRole("button", { name: "Close window" }));
		await waitFor(() => expect(nativeWindow.close).toHaveBeenCalledOnce());
		for (const label of ["Show", "Patch", "Venue", "Effects", "Media"])
			expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
		const openCad = screen.getByRole("button", { name: "Open CAD" });
		const openViz = screen.getByRole("button", { name: "Open Viz" });
		expect(openCad).toBeEnabled();
		expect(openCad.nextElementSibling).toBe(openViz);
		fireEvent.click(screen.getByRole("button", { name: "Patch" }));
		await screen.findByRole("columnheader", { name: "Fixture ID" });
		expect(
			document_root()?.querySelector(".viz-native-window-title"),
		).toBeNull();
		expect(document_root()?.querySelectorAll(".ui-window-header")).toHaveLength(
			1,
		);
	});

	it("carries no desk furniture", async () => {
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Patch" }));
		await screen.findByRole("columnheader", { name: "Fixture ID" });

		// The three things this window deliberately does not have.
		expect(
			screen.queryByRole("button", { name: /Preview Stage/i }),
			"no second renderer: the visualizer window is the picture",
		).not.toBeInTheDocument();
		expect(document_root()?.querySelector(".encoder-frame")).toBeNull();
		expect(document_root()?.querySelector(".command-line")).toBeNull();
	});

	it("offers the file actions the planning workflow needs", async () => {
		renderApp();
		for (const label of [
			"New Show",
			"Load Show from Disk",
			"Open Demo Show",
			"Save As",
			"Import MVR",
			"Export MVR",
		]) {
			expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
		}
	});

	it("shows the layers the document itself carries", async () => {
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Patch" }));
		// The sheet counts the layers it was given: a document written on a desk arrives with
		// its own, and its fixtures belong to them.
		expect(
			await screen.findByText("0 fixtures · 2 layers"),
		).toBeInTheDocument();
	});

	it("returns to All fixtures before revealing a CAD-selected entity", async () => {
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Patch" }));
		await screen.findByText("0 fixtures · 2 layers");
		const layers = screen
			.getByRole("heading", { name: "Layers" })
			.closest("aside");
		if (!layers) throw new Error("Layers sidebar was not rendered");
		fireEvent.click(within(layers).getByRole("button", { name: /^House/ }));
		expect(within(layers).getByRole("button", { name: /^House/ })).toHaveClass(
			"active",
		);

		await waitFor(() =>
			expect(eventHandlers.get("cad-selection-delta")).toBeDefined(),
		);
		await act(async () => {
			eventHandlers.get("cad-selection-delta")?.({
				payload: { revision: 1, selectedIds: [PROFILE_ID] },
			});
		});

		await waitFor(() =>
			expect(
				within(layers).getByRole("button", { name: /^All fixtures/ }),
			).toHaveClass("active"),
		);
	});

	it("offers the desk it finds on the network, and opens what that desk sends", async () => {
		const found = [
			{
				instance: "desk-foh",
				name: "front-of-house",
				show: "Summer Tour",
				address: "10.0.0.4:5000",
			},
		];
		const loaded = {
			...document,
			name: "Summer Tour",
			path: "/tmp/Summer Tour.show",
		};
		invoke.mockImplementation((command: string) => {
			switch (command) {
				case "discovered_desks":
					return Promise.resolve(found);
				case "load_from_desk":
					return Promise.resolve(loaded);
				case "document_summary":
					return Promise.resolve(document);
				case "patch_snapshot":
					return Promise.resolve(snapshot);
				case "live_dmx_inputs":
					return Promise.resolve(liveInputs);
				default:
					return Promise.resolve([]);
			}
		});
		renderApp();
		const load = await screen.findByRole("button", {
			name: "Load from Desk · front-of-house: Summer Tour",
		});
		expect(load).toHaveAttribute("title", "front-of-house at 10.0.0.4:5000");

		fireEvent.click(load);
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("load_from_desk", {
				instance: "desk-foh",
			}),
		);
		expect(
			await screen.findByText("Loaded Summer Tour from front-of-house"),
		).toBeInTheDocument();
	});

	it("opens the packaged demo without asking the operator to find a file", async () => {
		const demoShowId = "44444444-4444-4444-8444-444444444444";
		const copy = {
			...document,
			showId: demoShowId,
			name: "Demo Show 2",
			path: "/data/shows/demo-show-2.show",
			fixtureCount: 59,
		};
		let cadReads = 0;
		let demoOpened = false;
		invoke.mockImplementation((command: string) => {
			switch (command) {
				case "open_demo_show":
					demoOpened = true;
					return Promise.resolve(copy);
				case "document_summary":
					return Promise.resolve(demoOpened ? copy : document);
				case "patch_snapshot":
					return Promise.resolve(snapshot);
				case "cad_scene_snapshot":
					cadReads += 1;
					return Promise.resolve({
						showId: cadReads === 1 ? SHOW_ID : demoShowId,
						sceneRevision: cadReads,
						selectionRevision: 0,
						entities: [
							{
								...cadEntity,
								name: cadReads === 1 ? cadEntity.name : "Demo Wash 1",
							},
						],
						drawings: [],
						selectedIds: [],
						attachments: [],
					});
				case "live_dmx_inputs":
					return Promise.resolve(liveInputs);
				default:
					return Promise.resolve([]);
			}
		});
		renderApp();
		fireEvent.click(
			await screen.findByRole("button", { name: "Open Demo Show" }),
		);

		// No file dialog is involved: the command is the whole interaction.
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_demo_show"));
		expect(dialog.open).not.toHaveBeenCalled();
		// The operator is told which copy this is and where it went, because the packaged demo
		// itself is never what gets opened.
		expect(
			await screen.findByText(
				"Opened Demo Show 2, a copy of the packaged Demo Show, at /data/shows/demo-show-2.show",
			),
		).toBeInTheDocument();
		expect(await screen.findByText("Demo Show 2")).toBeInTheDocument();
		expect(cadReads).toBeGreaterThanOrEqual(2);
	});

	it("offers no desk when there is none on the network", async () => {
		renderApp();
		fireEvent.click(await screen.findByRole("tab", { name: "DMX" }));
		await screen.findByRole("heading", { name: "Live DMX Inputs" });
		expect(screen.queryByText(/Load from Desk/)).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /Take from Desk/ }),
		).not.toBeInTheDocument();
	});

	it("uses the shared DMX controls in the fixed operator column order", async () => {
		const configured = {
			schemaVersion: 1 as const,
			mappings: [
				{
					id: "show-u1",
					logicalUniverse: 1,
					protocol: "sacn",
					destinationUniverse: 1,
					port: 5568,
					enabled: false,
					delivery: "multicast",
				},
			],
		};
		invoke.mockImplementation((command: string) => {
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "live_dmx_inputs") return Promise.resolve(configured);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			return Promise.resolve([]);
		});

		renderApp();
		fireEvent.click(await screen.findByRole("tab", { name: "DMX" }));
		await screen.findByRole("switch", { name: "Enable universe 1" });
		const headers = screen
			.getAllByRole("columnheader")
			.map((header) => header.textContent?.trim() ?? "");
		expect(headers).toEqual([
			"Show universe",
			"Protocol",
			"Wire universe",
			"Delivery",
			"UDP port",
			"Actions",
			"Enabled",
		]);
		expect(screen.getAllByRole("row")[1]).toHaveTextContent(
			"Streaming ACN (sACN)",
		);
		expect(
			screen.getAllByRole("row")[1].querySelectorAll(".ui-select-trigger"),
		).toHaveLength(2);
		expect(screen.getAllByText("Protocol")).toHaveLength(1);
		expect(screen.getAllByText("Delivery")).toHaveLength(1);
		expect(screen.queryByText("On", { exact: true })).not.toBeInTheDocument();
		expect(screen.queryByText("Off", { exact: true })).not.toBeInTheDocument();
		const row = screen.getAllByRole("row")[1];
		expect(row).toHaveClass("is-disabled");
		const enabledCell = row.querySelector(".viz-live-enabled");
		expect(enabledCell?.querySelector(".ui-form-field")).toBeNull();
		expect(enabledCell?.querySelector(".ui-switch-field-bare")).toBeNull();
		expect(enabledCell).toHaveTextContent("");
		fireEvent.click(screen.getByRole("switch", { name: "Enable universe 1" }));
		expect(row).not.toHaveClass("is-disabled");
		expect(document_root()?.querySelector(".viz-editor-file-bar")).toHaveClass(
			"viz-editor-file-bar",
		);
	});

	it("requires an explicit source choice when several desks are detected", async () => {
		const found = [
			{
				instance: "desk-foh",
				name: "front-of-house",
				show: "Tour",
				address: "10.0.0.4:5000",
			},
			{
				instance: "desk-backup",
				name: "backup",
				show: "Tour",
				address: "10.0.0.5:5000",
			},
		];
		invoke.mockImplementation((command: string) => {
			switch (command) {
				case "document_summary":
					return Promise.resolve(document);
				case "library_profiles":
					return Promise.resolve([libraryProfile()]);
				case "discovered_desks":
					return Promise.resolve(found);
				case "live_dmx_inputs":
					return Promise.resolve(liveInputs);
				case "take_live_dmx_inputs_from_desk":
					return Promise.resolve(liveInputs);
				default:
					return Promise.resolve([]);
			}
		});
		renderApp();
		fireEvent.click(await screen.findByRole("tab", { name: "DMX" }));
		fireEvent.change(await screen.findByRole("combobox", { name: "Desk" }), {
			target: { value: "desk-backup" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Take from Desk · backup" }),
		);
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("take_live_dmx_inputs_from_desk", {
				instance: "desk-backup",
			}),
		);
	});

	it("previews a detected desk's routes and applies them explicitly", async () => {
		const found = [
			{
				instance: "desk-foh",
				name: "front-of-house",
				show: "Summer Tour",
				address: "10.0.0.4:5000",
			},
		];
		const preview = {
			schemaVersion: 1 as const,
			mappings: [
				{
					id: "desk-u1",
					logicalUniverse: 1,
					protocol: "artnet",
					destinationUniverse: 11,
					port: 6454,
					enabled: true,
					delivery: "unicast",
				},
			],
		};
		invoke.mockImplementation((command: string) => {
			switch (command) {
				case "document_summary":
					return Promise.resolve(document);
				case "library_profiles":
					return Promise.resolve([libraryProfile()]);
				case "discovered_desks":
					return Promise.resolve(found);
				case "live_dmx_inputs":
					return Promise.resolve(liveInputs);
				case "take_live_dmx_inputs_from_desk":
					return Promise.resolve(preview);
				case "save_live_dmx_inputs":
					return Promise.resolve(preview);
				default:
					return Promise.resolve([]);
			}
		});
		renderApp();
		fireEvent.click(await screen.findByRole("tab", { name: "DMX" }));
		fireEvent.click(
			await screen.findByRole("button", {
				name: "Take from Desk · front-of-house",
			}),
		);
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("take_live_dmx_inputs_from_desk", {
				instance: "desk-foh",
			}),
		);
		expect(await screen.findByDisplayValue("11")).toBeInTheDocument();
		expect(screen.getByText(/Review them, then Apply/)).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("save_live_dmx_inputs", {
				inputs: preview,
			}),
		);
	});

	it("waits for a document before mounting the sheet", async () => {
		invoke.mockImplementation((command: string) =>
			command === "document_summary"
				? Promise.resolve(null)
				: Promise.resolve([]),
		);
		renderApp();
		expect((await screen.findAllByText("No show open")).length).toBeGreaterThan(
			0,
		);
		expect(screen.queryByText("Show Patch")).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Patch" })).toBeDisabled();
	});

	it("opens the separate visualizer output from the bottom dock", async () => {
		let running = false;
		invoke.mockImplementation((command: string) => {
			if (command === "open_visualizer") {
				running = true;
				return Promise.resolve();
			}
			if (command === "visualizer_is_running") return Promise.resolve(running);
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			return Promise.resolve([]);
		});
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Patch" }));
		expect(screen.queryByLabelText("Preview controls")).not.toBeInTheDocument();
		fireEvent.click(await screen.findByRole("button", { name: "Open Viz" }));
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_visualizer"));
		expect(
			await screen.findByLabelText("Preview controls"),
		).toBeInTheDocument();
	});

	it("opens the separate CAD planner immediately above the visualizer action", async () => {
		invoke.mockImplementation((command: string) => {
			if (command === "open_cad") return Promise.resolve();
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			return Promise.resolve([]);
		});
		renderApp();
		const openCad = await screen.findByRole("button", { name: "Open CAD" });
		expect(openCad.nextElementSibling).toBe(
			screen.getByRole("button", { name: "Open Viz" }),
		);
		fireEvent.click(openCad);
		await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_cad"));
	});

	it("applies renderer Settings directly above Open Viz", async () => {
		invoke.mockImplementation((command: string, payload?: unknown) => {
			if (command === "renderer_settings")
				return Promise.resolve(rendererSettings);
			if (command === "save_renderer_settings")
				return Promise.resolve(
					(payload as { settings: typeof rendererSettings }).settings,
				);
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			return Promise.resolve([]);
		});
		renderApp();
		const settingsButton = await screen.findByRole("button", {
			name: "Settings",
		});
		const openButton = screen.getByRole("button", { name: "Open Viz" });
		expect(
			settingsButton.compareDocumentPosition(openButton) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		fireEvent.click(settingsButton);
		const sharedTitle = document_root()?.querySelector<HTMLElement>(
			".viz-show-settings-workspace > .ui-window-header",
		);
		if (!sharedTitle)
			throw new Error("Show and Settings title was not rendered");
		expect(sharedTitle).toHaveTextContent("Show");
		expect(
			within(sharedTitle)
				.getAllByRole("tab")
				.map((tab) => tab.textContent),
		).toEqual([
			"Show",
			"DMX",
			"Rendering",
			"Atmosphere",
			"Picture",
			"Features",
		]);
		expect(
			within(sharedTitle).getByRole("tab", { name: "Rendering" }),
		).toHaveClass("is-active");
		await waitFor(() =>
			expect(
				document_root()?.querySelectorAll(
					".viz-renderer-settings-grid > section",
				),
			).toHaveLength(1),
		);
		fireEvent.click(
			within(sharedTitle).getByRole("tab", { name: "Atmosphere" }),
		);
		fireEvent.input(await screen.findByRole("slider", { name: "Fog amount" }), {
			target: { value: "0.08" },
		});
		await waitFor(() =>
			expect(invoke).toHaveBeenCalledWith("save_renderer_settings", {
				settings: expect.objectContaining({ fog: 0.08 }),
			}),
		);
		expect(
			screen.queryByRole("button", { name: "Save settings" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Discard changes" }),
		).not.toBeInTheDocument();
		const fogAmount = screen.getByRole("slider", { name: "Fog amount" });
		expect(fogAmount).toHaveClass("ui-native-control");
		const fogFader = fogAmount.closest(".horizontal-touch-fader");
		if (!fogFader) throw new Error("Fog amount fader was not rendered");
		expect(getComputedStyle(fogFader)).toMatchObject({
			boxSizing: "border-box",
			maxWidth: "100%",
			width: "100%",
		});
		fireEvent.click(within(sharedTitle).getByRole("tab", { name: "Picture" }));
		expect(
			screen.getByRole("heading", { name: "Picture" }),
		).toBeInTheDocument();
		fireEvent.click(within(sharedTitle).getByRole("tab", { name: "Features" }));
		expect(
			screen.getByRole("heading", { name: "Features" }),
		).toBeInTheDocument();
		fireEvent.click(
			within(sharedTitle).getByRole("tab", { name: "Rendering" }),
		);
		expect(
			screen.getByRole("slider", { name: "Environment brightness" }),
		).toBeInTheDocument();
		fireEvent.click(within(sharedTitle).getByRole("tab", { name: "DMX" }));
		expect(
			screen.getByRole("heading", { name: "Live DMX Inputs" }),
		).toBeInTheDocument();
		fireEvent.click(within(sharedTitle).getByRole("tab", { name: "Show" }));
		expect(
			screen.getByRole("button", { name: "Open Demo Show" }),
		).toBeInTheDocument();
	});

	it("reflects settings changed in the running Visualizer", async () => {
		let reads = 0;
		invoke.mockImplementation((command: string) => {
			if (command === "renderer_settings") {
				reads += 1;
				return Promise.resolve(
					reads === 1
						? rendererSettings
						: { ...rendererSettings, quality: "draft", fog: 0.02 },
				);
			}
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			return Promise.resolve([]);
		});
		renderApp();
		fireEvent.click(
			(await screen.findAllByRole("button", { name: "Settings" }))[0],
		);
		fireEvent.click(await screen.findByRole("tab", { name: "Atmosphere" }));
		const fog = await screen.findByRole("slider", { name: "Fog amount" });
		expect(fog).toHaveValue("0.15");
		await waitFor(() => expect(fog).toHaveValue("0.02"), { timeout: 1500 });
		fireEvent.click(screen.getByRole("tab", { name: "Rendering" }));
		expect(screen.getByRole("button", { name: "Draft" })).toBeInTheDocument();
	});

	it("presents Patch, Venue, and Effects through the same patch surface", async () => {
		renderApp();
		for (const screenName of ["Patch", "Venue", "Effects"] as const) {
			fireEvent.click(await screen.findByRole("button", { name: screenName }));
			expect(
				document_root()?.querySelector(".show-patch-layout .ui-window-title"),
			).toHaveTextContent(screenName);
			expect(
				screen.getByRole("columnheader", { name: "Fixture ID" }),
			).toBeInTheDocument();
		}
	});

	it("keeps Media tabs and the contextual add action in one title row", async () => {
		const mediaLayout = {
			fallbackAssets: [],
			servers: [],
			sources: [],
			ledModuleTypes: [],
			surfaces: [],
			projectors: [],
		};
		invoke.mockImplementation((command: string) => {
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			if (command === "media_layout") return Promise.resolve(mediaLayout);
			return Promise.resolve([]);
		});

		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Media" }));
		const title = document_root()?.querySelector<HTMLElement>(
			".viz-media-workspace > .ui-window-header",
		);
		if (!title) throw new Error("Media title row was not rendered");
		expect(
			Array.from(
				title.querySelectorAll("button"),
				(button) => button.textContent,
			),
		).toEqual([
			"Discover servers",
			"Enumerate outputs",
			"Add output manually",
			"Add server",
			"Servers",
			"Surfaces",
			"LED module types",
			"Projectors",
		]);
		expect(
			document_root()?.querySelectorAll(
				".viz-media-workspace > .ui-window-header",
			),
		).toHaveLength(1);
		expect(screen.queryAllByRole("columnheader")).toHaveLength(0);
		expect(
			screen.getByRole("heading", { name: "No media servers patched" }),
		).toBeInTheDocument();
		expect(
			screen.getByText(/patched DMX fixtures that control layers/),
		).toBeInTheDocument();

		fireEvent.click(within(title).getByRole("tab", { name: "Surfaces" }));
		expect(
			within(title).getByRole("button", { name: "Add surface" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "No media surfaces" }),
		).toBeInTheDocument();
		expect(
			screen.getByText(/physical projection screens, TVs, and LED walls/),
		).toBeInTheDocument();
		fireEvent.click(
			within(title).getByRole("tab", { name: "LED module types" }),
		);
		expect(
			screen.getByRole("heading", { name: "No LED module types" }),
		).toBeInTheDocument();
		fireEvent.click(within(title).getByRole("tab", { name: "Projectors" }));
		expect(
			screen.getByRole("heading", { name: "No projectors" }),
		).toBeInTheDocument();
	});

	it("shows a linked media server's DMX patch and opens the shared address screen", async () => {
		const profile = typedProfile(
			"ToskLight",
			"Media Server",
			"media_server",
			"09",
		);
		const fixtureId = "99999999-9999-4999-8999-999999999999";
		const mediaSnapshot = {
			...snapshot,
			fixtures: [
				{
					fixtureId,
					fixtureNumber: 1,
					virtualFixtureNumber: null,
					name: "Media Server 1",
					profileId: profile.id,
					profileRevision: 1,
					modeId: profile.profile.modes[0].id,
					splitPatches: [{ split: 1, universe: 2, address: 101 }],
					layerId: "house",
					directControl: null,
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					moveInBlackEnabled: true,
					moveInBlackDelayMillis: 0,
					highlightOverrides: [],
					fixtureRevision: 1,
					logicalHeads: [],
				},
			],
			profileRevisions: [
				{
					profileId: profile.id,
					profileRevision: 1,
					contentDigest: "demo",
					manufacturer: "ToskLight",
					name: "Media Server",
					fixtureType: "media_server",
					patchPolicy: "dmx",
					referencedModes: [
						{
							modeId: profile.profile.modes[0].id,
							name: "Default",
							splits: [{ split: 1, footprint: 1 }],
						},
					],
					profileSnapshot: profile.profile,
				},
			],
		};
		const mediaLayout = {
			fallbackAssets: [],
			servers: [
				{
					revision: 1,
					object: {
						kind: "media_server",
						body: {
							id: "server-1",
							name: "Media Server 1",
							fixtureId,
							citp: { host: "127.0.0.1", port: 4809 },
						},
					},
				},
			],
			sources: [],
			ledModuleTypes: [],
			surfaces: [],
			projectors: [],
		};
		invoke.mockImplementation((command: string) => {
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(mediaSnapshot);
			if (command === "patch_layers") return Promise.resolve([]);
			if (command === "library_profiles") return Promise.resolve([profile]);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			if (command === "media_layout") return Promise.resolve(mediaLayout);
			return Promise.resolve([]);
		});

		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Media" }));
		const address = await screen.findByRole("button", {
			name: "DMX patch, 2.101",
		});
		expect(address.closest(".viz-media-server-editor")).not.toBeNull();
		fireEvent.click(address);
		expect(
			await screen.findByRole("dialog", { name: "Fixture Address" }),
		).toBeInTheDocument();
	});

	it("can patch a CITP-discovered media server from its own editor section", async () => {
		const profile = typedProfile(
			"ToskLight",
			"Media Server",
			"media_server",
			"08",
		);
		invoke.mockImplementation((command: string) => {
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			if (command === "library_profiles") return Promise.resolve([profile]);
			if (command === "media_layout")
				return Promise.resolve({
					fallbackAssets: [],
					servers: [
						{
							revision: 1,
							object: {
								kind: "media_server",
								body: {
									id: "discovered-server",
									name: "Discovered Server",
									citp: { host: "10.0.0.9", port: 4809 },
								},
							},
						},
					],
					sources: [],
					ledModuleTypes: [],
					surfaces: [],
					projectors: [],
				});
			return Promise.resolve([]);
		});
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Media" }));
		const patchServer = await screen.findByRole("button", {
			name: "Patch media server",
		});
		expect(patchServer.closest(".viz-media-server-editor")).not.toBeNull();
		fireEvent.click(patchServer);
		expect(
			await screen.findByRole("dialog", { name: "Add fixture" }),
		).toBeInTheDocument();
	});

	it("reports Media discovery failures as a bottom-right toast", async () => {
		invoke.mockImplementation((command: string) => {
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			if (command === "media_layout")
				return Promise.resolve({
					fallbackAssets: [],
					servers: [],
					sources: [],
					ledModuleTypes: [],
					surfaces: [],
					projectors: [],
				});
			if (command === "discover_citp_servers") return Promise.resolve([]);
			return Promise.resolve([]);
		});
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Media" }));
		const title = document_root()?.querySelector<HTMLElement>(
			".viz-media-workspace > .ui-window-header",
		);
		if (!title) throw new Error("Media title row was not rendered");
		fireEvent.click(
			within(title).getByRole("button", { name: "Discover servers" }),
		);
		const toast = await screen.findByRole("alert");
		expect(toast).toHaveClass("viz-editor-toast");
		expect(toast).toHaveTextContent("No running CITP Media Server");
		expect(title).toHaveTextContent("Media");
	});

	it("opens Add server as the shared fixture library filtered to media-server fixtures", async () => {
		const emptyMedia = {
			fallbackAssets: [],
			servers: [],
			sources: [],
			ledModuleTypes: [],
			surfaces: [],
			projectors: [],
		};
		invoke.mockImplementation((command: string) => {
			if (command === "document_summary") return Promise.resolve(document);
			if (command === "patch_snapshot") return Promise.resolve(snapshot);
			if (command === "live_dmx_inputs") return Promise.resolve(liveInputs);
			if (command === "media_layout") return Promise.resolve(emptyMedia);
			if (command === "library_profiles")
				return Promise.resolve([
					typedProfile("ToskLight", "Media Server", "media_server", "01"),
					typedProfile("Resolume", "Arena Server", "media_server", "02"),
					typedProfile("LightingCo", "Wash", "wash", "03"),
				]);
			return Promise.resolve([]);
		});
		renderApp();
		fireEvent.click(await screen.findByRole("button", { name: "Media" }));
		fireEvent.click(await screen.findByRole("button", { name: "Add server" }));
		const dialog = await screen.findByRole("dialog", { name: "Add fixture" });
		expect(
			within(dialog).getByRole("button", { name: "ToskLight" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: "Resolume" }),
		).toBeInTheDocument();
		expect(
			within(dialog).queryByRole("button", { name: "LightingCo" }),
		).not.toBeInTheDocument();
		expect(within(dialog).getByText("Media Server")).toBeInTheDocument();
		expect(within(dialog).getByText("Arena Server")).toBeInTheDocument();
		expect(within(dialog).queryByText("Wash")).not.toBeInTheDocument();
	});

	it("reports a document that will not open instead of failing silently", async () => {
		invoke.mockImplementation((command: string) =>
			command === "document_summary"
				? Promise.reject(new Error("show file is not readable"))
				: Promise.resolve([]),
		);
		renderApp();
		await waitFor(() =>
			expect(
				screen.getByText(/show file is not readable/i),
			).toBeInTheDocument(),
		);
	});
});

function document_root() {
	return globalThis.document.body;
}
