import {
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadApp, keepNewerHalves } from "./CadApp";

const fixtureId = "11111111-1111-4111-8111-111111111111";
const secondFixtureId = "33333333-3333-4333-8333-333333333333";
const snapshot = {
	showId: "22222222-2222-4222-8222-222222222222",
	sceneRevision: 9,
	selectionRevision: 4,
	selectedIds: [fixtureId],
	attachments: [],
	drawings: [],
	entities: [
		{
			id: fixtureId,
			logicalFixtureId: fixtureId,
			name: "Profile Stage 1",
			fixtureNumber: 101,
			fixtureDisplayId: "101",
			dmxAddress: "1.1",
			fixtureProfile: "Robe Robin DLS Profile",
			mode: "Mode 3",
			note: "Use secondary safety",
			kind: "profile",
			fixtureType: "moving_head_profile",
			drawingId: "profile:1",
			layerId: "default",
			selectable: true,
			positionMillimetres: [0, 0, 4000],
			rotationDegrees: [0, 0, 0],
			sizeMillimetres: [400, 500, 700],
			outputDirection: [0, 1, 0],
		},
	],
};

const mocks = vi.hoisted(() => ({
	snapshot: vi.fn(),
	replaceSelection: vi.fn(),
	transform: vi.fn(),
	undo: vi.fn(),
	redo: vi.fn(),
	delete: vi.fn(),
	add: vi.fn(),
	setTransforms: vi.fn(),
	exportPdf: vi.fn(),
	onSceneDelta: vi.fn(),
	onSelectionDelta: vi.fn(),
}));
const documentMocks = vi.hoisted(() => ({
	current: vi.fn(),
	savePaperwork: vi.fn(),
	patchSnapshot: vi.fn(),
	fixtureProfiles: vi.fn(),
	fixtureProfileUpdate: vi.fn(),
	updateFixtureProfile: vi.fn(),
	fixtureNotes: vi.fn(),
	saveFixtureNote: vi.fn(),
}));
const transportMocks = vi.hoisted(() => ({ patchFixtures: vi.fn() }));
const nativeWindow = vi.hoisted(() => ({
	close: vi.fn().mockResolvedValue(undefined),
	isFullscreen: vi.fn().mockResolvedValue(false),
	setFullscreen: vi.fn().mockResolvedValue(undefined),
	startDragging: vi.fn().mockResolvedValue(undefined),
}));
const workspace = new Map<string, string>();

vi.mock("./session", () => ({ cadSession: mocks }));
// The Elements panel reads the drawing arrangement straight from the show.
vi.mock("@tauri-apps/api/core", () => ({
	invoke: vi.fn((command: string) =>
		command === "cad_drawing_tree"
			? Promise.resolve({ folders: [], items: {} })
			: Promise.reject(new Error(`unexpected command ${command}`)),
	),
}));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn(() => Promise.resolve(() => undefined)),
}));
vi.mock("../document/session", () => ({ documentSession: documentMocks }));
vi.mock("../document/transport", () => ({
	TauriPatchTransport: class {
		patchFixtures = transportMocks.patchFixtures;
	},
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
	save: vi.fn().mockResolvedValue("/tmp/rig-plan.pdf"),
}));
vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: () => nativeWindow,
}));
const renderCounts = vi.hoisted(() => ({ viewBar: 0 }));
vi.mock("./CadTileViewBar", async (importOriginal) => {
	const actual = await importOriginal<typeof import("./CadTileViewBar")>();
	return {
		CadTileViewBar: (props: Parameters<typeof actual.CadTileViewBar>[0]) => {
			renderCounts.viewBar += 1;
			return actual.CadTileViewBar(props);
		},
	};
});
/** Where each render of the mocked viewports drew the first fixture, preview included, along X. */
const drawnX = vi.hoisted(() => [] as number[]);
vi.mock("./CadViewport", () => ({
	CadViewport: ({
		view,
		rotationQuarterTurns,
		camera,
		entities,
		onSelection,
		preview,
		onPreview,
		onMove,
		showCoordinateOrigins,
	}: {
		view: string;
		rotationQuarterTurns: number;
		camera: { pan: [number, number]; zoom: number };
		entities: readonly { positionMillimetres: [number, number, number] }[];
		onSelection(change: unknown): void;
		preview: { deltaMillimetres: [number, number, number] } | null;
		onPreview(preview: unknown): void;
		onMove(
			delta: [number, number, number],
			ids: readonly string[],
			spread: boolean,
		): void;
		showCoordinateOrigins: boolean;
	}) => {
		if (entities[0])
			drawnX.push(entities[0].positionMillimetres[0] + (preview?.deltaMillimetres[0] ?? 0));
		return (
			<button
				type="button"
				data-testid="cad-canvas"
				data-rotation={rotationQuarterTurns}
				data-pan={camera.pan.join(",")}
				data-zoom={camera.zoom}
				data-preview={preview?.deltaMillimetres.join(",") ?? "none"}
				data-coordinate-origins={showCoordinateOrigins ? "visible" : "hidden"}
				onPointerMove={() =>
					onPreview({
						entityIds: [fixtureId],
						deltaMillimetres: [250, 0, 0],
						spread: false,
					})
				}
				// As the real viewport does, a release keeps the preview and hands the move on.
				onPointerUp={() => onMove([250, 0, 0], [fixtureId], false)}
				onClick={() =>
					onSelection({
						type: "replace",
						ids: [view === "top_down" ? fixtureId : secondFixtureId],
					})
				}
			>
				{view}
			</button>
		);
	},
}));

beforeEach(() => {
	workspace.clear();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => workspace.get(key) ?? null,
		setItem: (key: string, value: string) => workspace.set(key, value),
		removeItem: (key: string) => workspace.delete(key),
		clear: () => workspace.clear(),
	});
	mocks.snapshot.mockReset().mockResolvedValue(snapshot);
	mocks.replaceSelection.mockReset();
	mocks.transform.mockReset();
	mocks.undo.mockReset();
	mocks.redo.mockReset();
	mocks.delete.mockReset().mockResolvedValue({ sceneRevision: 10, deletedIds: [fixtureId] });
	mocks.add.mockReset().mockResolvedValue({ sceneRevision: 10, addedIds: [] });
	mocks.exportPdf.mockReset().mockResolvedValue(undefined);
	mocks.onSceneDelta.mockReset().mockResolvedValue(() => undefined);
	mocks.onSelectionDelta.mockReset().mockResolvedValue(() => undefined);
	documentMocks.current.mockReset().mockResolvedValue({
		showId: snapshot.showId,
		name: "Demo Show",
		path: "/tmp/demo.show",
		fixtureCount: 1,
		fileName: "demo.show",
		lightingDesigner: "",
		showVersion: "",
		venue: "",
		contactEmail: "",
		contactPhone: "",
		project: "",
		showDate: "",
		lastSavedAt: 1_787_000_000,
		universeCount: 1,
	});
	documentMocks.savePaperwork.mockReset().mockImplementation((paperwork) =>
		Promise.resolve({
			showId: snapshot.showId,
			name: "Demo Show",
			path: "/tmp/demo.show",
			fixtureCount: 1,
			fileName: "demo.show",
			...paperwork,
			lastSavedAt: 1_787_000_001,
			universeCount: 1,
		}),
	);
	documentMocks.patchSnapshot.mockReset().mockResolvedValue({
		fixtures: [
			{
				fixtureId,
				name: "Profile Stage 1",
				location: { x: 0, y: 0, z: 4000 },
				rotation: { x: 0, y: 0, z: 0 },
			},
		],
	});
	documentMocks.fixtureNotes
		.mockReset()
		.mockResolvedValue([{ fixtureId, note: "Use secondary safety" }]);
	documentMocks.saveFixtureNote.mockReset().mockResolvedValue(undefined);
	documentMocks.fixtureProfiles.mockReset().mockResolvedValue([]);
	documentMocks.fixtureProfileUpdate.mockReset().mockResolvedValue(null);
	documentMocks.updateFixtureProfile.mockReset().mockResolvedValue(undefined);
	transportMocks.patchFixtures.mockReset().mockResolvedValue(undefined);
	for (const action of Object.values(nativeWindow)) action.mockClear();
});

describe("a scene write that lost the race", () => {
	const base = {
		sceneRevision: 7,
		selectionRevision: 3,
		selectedIds: ["a"],
		entities: [{ id: "a" }],
		drawings: [],
		attachments: [],
	} as unknown as Parameters<typeof keepNewerHalves>[0];

	it("keeps the newer rig when a selection carries the revision the mutation replaced", () => {
		// What duplicate-then-move used to do: the delta lands at 8, then `select` writes back the
		// pre-duplicate snapshot it captured at 7 and the next drag commits against a stale rig.
		const afterDelta = { ...base, sceneRevision: 8, entities: [{ id: "a" }, { id: "copy" }] };
		const staleSelection = { ...base, selectedIds: ["copy"] };
		const merged = keepNewerHalves(
			afterDelta as typeof base,
			staleSelection as typeof base,
		);
		expect(merged.sceneRevision).toBe(8);
		expect(merged.entities).toHaveLength(2);
		// The selection half of the write is still applied.
		expect(merged.selectedIds).toEqual(["copy"]);
	});

	it("takes a newer rig, and a newer selection, when the write carries one", () => {
		const newer = {
			...base,
			sceneRevision: 9,
			selectionRevision: 4,
			selectedIds: ["b"],
		};
		const merged = keepNewerHalves(base, newer as typeof base);
		expect(merged.sceneRevision).toBe(9);
		expect(merged.selectionRevision).toBe(4);
		expect(merged.selectedIds).toEqual(["b"]);
	});

	it("keeps a newer selection when the write carries an older one", () => {
		const afterSelectionDelta = { ...base, selectionRevision: 5, selectedIds: ["b"] };
		const merged = keepNewerHalves(afterSelectionDelta as typeof base, base);
		expect(merged.selectionRevision).toBe(5);
		expect(merged.selectedIds).toEqual(["b"]);
	});
});

describe("the CAD planning screen", () => {
	it("draws no window chrome of its own and exposes every view from the viewport corner", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const title = screen.getByText("CAD");
		expect(title.closest(".ui-window-header")).toBeInTheDocument();
		expect(
			screen.queryByText(/First synchronized 2D planning slice/i),
		).not.toBeInTheDocument();
		// The editor window draws the frame around this screen, so drawing a second set of
		// window controls inside it would close or resize the window the operator is working in.
		for (const control of ["Close window", "Enter fullscreen", "Move window"])
			expect(
				screen.queryByRole("button", { name: control }),
			).not.toBeInTheDocument();
		const direction = await screen.findByRole("combobox", {
			name: "View direction",
		});
		expect(direction.parentElement).toHaveClass("cad-view-control");
		expect(
			within(direction.parentElement as HTMLElement).getByRole("button", {
				name: "Fit",
			}),
		).toBeInTheDocument();
		expect(
			within(direction)
				.getAllByRole("option")
				.map((option) => option.textContent),
		).toEqual([
			"Top down",
			"Left to right",
			"Right to left",
			"Front to back",
			"Back to front",
		]);
		expect(
			screen.getByLabelText("Orientation: right +X, up +Y, depth +Z"),
		).toBeInTheDocument();
		expect(
			screen.queryByText("Scene r9 · Selection r4"),
		).not.toBeInTheDocument();
		const header = title.closest(".ui-window-header");
		if (!header)
			throw new Error("CAD title was not rendered in a window header");
		const named = (button: Element) =>
			button.getAttribute("aria-label") ?? button.textContent ?? "";
		expect(
			within(header as HTMLElement).getAllByRole("button").map(named),
		).toEqual(["Undo", "Redo", "Plans", "Elements", "Settings"]);
		// Undo and Redo are icons that name themselves, like the other title tools.
		for (const name of ["Undo", "Redo"]) {
			const button = within(header as HTMLElement).getByRole("button", { name });
			expect(button.textContent?.trim()).toBe("");
			expect(button.querySelector("svg")).not.toBeNull();
		}
		// The group boundary — and so the divider — belongs right of Redo; Meta is gone.
		expect(
			[...(header as HTMLElement).querySelectorAll(".ui-window-action-group")]
				.map((group) => [...group.querySelectorAll("button")].map(named).join(" "))
				.filter((labels) => labels.length > 0),
		).toEqual(["Undo Redo", "Plans Elements", "Settings"]);
		expect(
			screen.getByRole("button", {
				name: "Rotate top-down view 90 degrees counterclockwise",
			}),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", {
				name: "Rotate top-down view 90 degrees clockwise",
			}),
		).toBeInTheDocument();
	});

	it("lays out multiple selected print pages and blocks rig transforms", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		expect(
			screen.queryByRole("button", { name: "Add New Page" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Plans" }));
		fireEvent.click(screen.getByRole("button", { name: "Add New Page" }));
		fireEvent.click(screen.getByRole("button", { name: "Add plan page" }));
		fireEvent.click(await screen.findByRole("menuitem", { name: "Fixture list" }));
		const planRows = within(screen.getByRole("complementary", { name: "Plans" }))
			.getAllByRole("button", { name: /^\d+\. / })
			.map((row) =>
				[...row.children].map((cell) => cell.textContent),
			);
		expect(planRows).toEqual([
			["1", "Page 1", "Top down", "A4 Landscape"],
			["2", "Fixture List", "Fixture table", "A4 Landscape"],
		]);
		// The page just added stays the selected one, and its row says so.
		expect(screen.getByRole("button", { name: "2. Fixture List" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);

		fireEvent.pointerMove(screen.getByTestId("cad-canvas"));
		fireEvent.pointerUp(screen.getByTestId("cad-canvas"));
		await waitFor(() => expect(mocks.transform).not.toHaveBeenCalled());

		fireEvent.click(screen.getByRole("button", { name: "Export to PDF" }));
		await waitFor(() => expect(mocks.exportPdf).toHaveBeenCalledTimes(1));
		const bytes = mocks.exportPdf.mock.calls[0][1] as Uint8Array;
		const pdf = new TextDecoder().decode(bytes);
		expect(pdf).toContain("/Count 2");
		expect(pdf).toContain("Fixture ID");
		expect(pdf).toContain("Robe Robin DLS Profile");
		expect(pdf).toContain("Use secondary safety");
		expect(workspace.get("tosklight:viz-editor:cad-print-pages:v2")).toContain(
			"Page 1",
		);
	});

	it("opens plans and elements from their own window title buttons", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		expect(screen.queryByRole("button", { name: "Meta" })).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Plans" }));
		expect(screen.getByRole("button", { name: "Export to PDF" })).toBeVisible();

		// Elements is its own title button; Drawings, Objects and + share the panel's title row.
		fireEvent.click(screen.getByRole("button", { name: "Elements" }));
		const panel = screen.getByRole("complementary", { name: "Elements" });
		expect(
			screen.queryByRole("button", { name: "Export to PDF" }),
		).not.toBeInTheDocument();
		const titleRow = panel.querySelector(".cad-sidebar-header") as HTMLElement;
		expect(within(titleRow).getByRole("heading", { name: "Elements" })).toBeVisible();
		expect(
			within(titleRow).getAllByRole("tab").map((tab) => tab.textContent),
		).toEqual(["Drawings", "Objects"]);
		// Adding lives only behind +, never as buttons in the panel.
		expect(within(panel).queryByRole("button", { name: "New folder" })).toBeNull();
		expect(within(panel).queryByRole("button", { name: "Add Drawing" })).toBeNull();
		fireEvent.click(within(titleRow).getByRole("button", { name: "Add drawing" }));
		expect(
			(await screen.findAllByRole("menuitem")).map((item) => item.textContent),
		).toEqual(["Import drawing (DXF, SVG)…", "New folder"]);
		fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

		fireEvent.click(within(titleRow).getByRole("tab", { name: "Objects" }));
		expect(screen.getByRole("region", { name: "Venue items" })).toBeVisible();
		expect(screen.getByRole("region", { name: "3D models" })).toBeVisible();
		expect(within(panel).queryByRole("button", { name: "Import 3D model" })).toBeNull();
		fireEvent.click(within(titleRow).getByRole("button", { name: "Add object" }));
		expect(
			await screen.findByRole("menuitem", { name: "Import 3D model…" }),
		).toBeInTheDocument();

		// Pressing the open panel's own button closes it; the selection keeps Info open alone.
		fireEvent.click(screen.getByRole("button", { name: "Elements" }));
		expect(
			screen.queryByRole("complementary", { name: "Elements" }),
		).not.toBeInTheDocument();
		expect(screen.getByRole("complementary", { name: "Info" })).toBeVisible();
	});

	it("edits the selected element in the Info panel and remembers the panel width", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		const info = await screen.findByRole("region", { name: "Info" });
		await waitFor(() =>
			expect(within(info).getByLabelText("Notes")).toHaveValue("Use secondary safety"),
		);
		// Generic holds what the element is; where it stands is under Placement.
		expect(within(info).queryByLabelText("Position X")).toBeNull();
		const notes = within(info).getByLabelText("Notes");
		fireEvent.change(notes, { target: { value: "Check clamp" } });
		fireEvent.blur(notes);
		expect(documentMocks.saveFixtureNote).toHaveBeenCalledWith({ fixtureId, note: "Check clamp" });

		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		expect(within(info).queryByLabelText("Notes")).toBeNull();
		expect(within(info).getByLabelText("Position Z")).toHaveValue("4");
		// A lamp cannot be drawn at another size.
		expect(within(info).queryByLabelText("Scale")).toBeNull();

		// Typing does not write; Enter does, in whole millimetres.
		const x = within(info).getByLabelText("Position X");
		fireEvent.change(x, { target: { value: "1.25" } });
		expect(transportMocks.patchFixtures).not.toHaveBeenCalled();
		fireEvent.keyDown(x, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(transportMocks.patchFixtures.mock.calls[0][2].fixtures[0]).toMatchObject({
			fixtureId,
			location: { x: 1250, y: 0, z: 4000 },
		});

		const aside = screen.getByRole("complementary", { name: "Info" });
		const handle = within(aside).getByRole("separator", { name: "Resize side panel" });
		fireEvent.keyDown(handle, { key: "ArrowLeft" });
		expect(aside.style.width).toBe("320px");
		expect(workspace.get("tosklight:viz-editor:cad-sidebar-width:v1")).toBe("320");
	});

	it("switches view, zooms and pans the viewport from the keyboard, but not while typing", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const canvas = await screen.findByTestId("cad-canvas");
		const zoom = () => Number(screen.getByTestId("cad-canvas").getAttribute("data-zoom"));
		const before = zoom();
		fireEvent.keyDown(window, { key: "+" });
		expect(zoom()).toBeCloseTo(before * 1.25);
		fireEvent.keyDown(window, { key: "-" });
		expect(zoom()).toBeCloseTo(before);

		const pan = () => screen.getByTestId("cad-canvas").getAttribute("data-pan");
		const panned = pan();
		fireEvent.keyDown(window, { key: "d" });
		expect(pan()).not.toBe(panned);

		fireEvent.keyDown(window, { key: "2" });
		expect(canvas).toHaveTextContent("left_to_right");
		expect(screen.getByRole("combobox", { name: "View direction" })).toHaveValue("left_to_right");

		// A key typed into a field belongs to the field.
		const info = await screen.findByRole("region", { name: "Info" });
		fireEvent.keyDown(within(info).getByLabelText("Name"), { key: "1" });
		expect(screen.getByTestId("cad-canvas")).toHaveTextContent("left_to_right");
	});

	it("deletes a single selected element at once, from Info or with Delete and Backspace", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		const aside = await screen.findByRole("complementary", { name: "Info" });
		fireEvent.click(within(aside).getByRole("button", { name: "Delete selected element" }));
		await waitFor(() => expect(mocks.delete).toHaveBeenCalledWith(9, [fixtureId]));
		expect(screen.queryByRole("dialog")).toBeNull();
		await waitFor(() => expect(mocks.replaceSelection).toHaveBeenCalledWith(4, []));

		for (const key of ["Delete", "Backspace"]) {
			mocks.delete.mockClear();
			fireEvent.keyDown(window, { key });
			await waitFor(() => expect(mocks.delete).toHaveBeenCalledWith(9, [fixtureId]));
			expect(screen.queryByRole("dialog")).toBeNull();
		}
		// A key typed into a field is the field's: Backspace there edits the text.
		mocks.delete.mockClear();
		fireEvent.keyDown(within(await screen.findByRole("region", { name: "Info" })).getByLabelText("Name"), {
			key: "Backspace",
		});
		expect(mocks.delete).not.toHaveBeenCalled();
	});

	it("undoes and redoes with Command-Z and Shift-Command-Z", async () => {
		mocks.undo.mockResolvedValue({ sceneRevision: 10, transforms: [], attachments: [] });
		mocks.redo.mockResolvedValue({ sceneRevision: 11, transforms: [], attachments: [] });
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		fireEvent.keyDown(window, { key: "z", metaKey: true });
		await waitFor(() => expect(mocks.undo).toHaveBeenCalledWith(9));
		fireEvent.keyDown(window, { key: "Z", metaKey: true, shiftKey: true });
		await waitFor(() => expect(mocks.redo).toHaveBeenCalledTimes(1));
		fireEvent.keyDown(window, { key: "z", ctrlKey: true });
		await waitFor(() => expect(mocks.undo).toHaveBeenCalledTimes(2));
		expect(mocks.delete).not.toHaveBeenCalled();
	});

	it("opens Duplicate and Delete for the selection from the Menu key", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		await screen.findByRole("complementary", { name: "Info" });
		fireEvent.keyDown(window, { key: "ContextMenu" });
		const menu = await screen.findByRole("menu", { name: "Actions for the selected element" });
		expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
			"Duplicate",
			"Delete",
		]);
		fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
		expect(screen.queryByRole("menu")).toBeNull();
		// One element goes at once, as it does from Info.
		await waitFor(() => expect(mocks.delete).toHaveBeenCalledWith(9, [fixtureId]));
		expect(screen.queryByRole("dialog")).toBeNull();
	});

	it("duplicates the selection from its menu as a new, unpatched element beside it, and selects it", async () => {
		documentMocks.patchSnapshot.mockResolvedValue({
			showId: snapshot.showId,
			patchRevision: 3,
			fixtures: [
				{
					fixtureId,
					fixtureNumber: 101,
					virtualFixtureNumber: null,
					name: "Profile Stage 1",
					splitPatches: [{ split: 1, universe: 1, address: 1 }],
					location: { x: 0, y: 0, z: 4000 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					logicalHeads: [],
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		await screen.findByRole("complementary", { name: "Info" });
		// A key typed into a field is the field's.
		fireEvent.keyDown(within(await screen.findByRole("region", { name: "Info" })).getByLabelText("Name"), {
			key: "ContextMenu",
		});
		expect(screen.queryByRole("menu")).toBeNull();
		fireEvent.keyDown(window, { key: "F10", shiftKey: true });
		fireEvent.click(await screen.findByRole("menuitem", { name: "Duplicate" }));
		// The copy goes to the show as one step Undo takes away, against the rig it was made from.
		await waitFor(() => expect(mocks.add).toHaveBeenCalledTimes(1));
		expect(mocks.add.mock.calls[0][0]).toBe(9);
		const [written] = mocks.add.mock.calls[0][1];
		expect(written.fixtureId).not.toBe(fixtureId);
		expect(written).toMatchObject({
			fixtureNumber: 102,
			splitPatches: [{ split: 1, universe: null, address: null }],
			location: { x: 400, y: 0, z: 4000 },
		});
		await waitFor(() => expect(mocks.replaceSelection).toHaveBeenCalledWith(4, [written.fixtureId]));
	});

	it("duplicates the selection with Cmd+D the way its menu does, but not from a field", async () => {
		documentMocks.patchSnapshot.mockResolvedValue({
			showId: snapshot.showId,
			patchRevision: 3,
			fixtures: [
				{
					fixtureId,
					fixtureNumber: 101,
					virtualFixtureNumber: null,
					name: "Profile Stage 1",
					splitPatches: [{ split: 1, universe: 1, address: 1 }],
					location: { x: 0, y: 0, z: 4000 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					logicalHeads: [],
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		fireEvent.keyDown(within(await screen.findByRole("region", { name: "Info" })).getByLabelText("Name"), {
			key: "d",
			metaKey: true,
		});
		expect(mocks.add).not.toHaveBeenCalled();
		const pressed = new KeyboardEvent("keydown", { key: "d", metaKey: true, cancelable: true });
		window.dispatchEvent(pressed);
		// The browser's own ⌘D (bookmark) never runs over the drawing.
		expect(pressed.defaultPrevented).toBe(true);
		await waitFor(() => expect(mocks.add).toHaveBeenCalledTimes(1));
		const [written] = mocks.add.mock.calls[0][1];
		expect(written).toMatchObject({
			fixtureNumber: 102,
			splitPatches: [{ split: 1, universe: null, address: null }],
			location: { x: 400, y: 0, z: 4000 },
		});
		await waitFor(() => expect(mocks.replaceSelection).toHaveBeenCalledWith(4, [written.fixtureId]));
	});

	it("lists several selected elements in Info and always confirms deleting them", async () => {
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			selectedIds: [fixtureId, secondFixtureId],
			entities: [
				...snapshot.entities,
				{
					...snapshot.entities[0],
					id: secondFixtureId,
					logicalFixtureId: secondFixtureId,
					name: "Wash Left",
					fixtureDisplayId: "102",
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const list = await screen.findByRole("list", { name: "Selected elements" });
		expect(
			within(list)
				.getAllByRole("button")
				.map((row) => [...row.children].map((cell) => cell.textContent)),
		).toEqual([
			["101", "Profile Stage 1", "Robe Robin DLS Profile", "1.1"],
			["102", "Wash Left", "Robe Robin DLS Profile", "1.1"],
		]);
		const aside = screen.getByRole("complementary", { name: "Info" });
		// The trash button is how a selection is deleted; the list offers no second way.
		expect(within(aside).queryByRole("button", { name: /Delete all/ })).toBeNull();
		fireEvent.click(within(aside).getByRole("button", { name: "Delete 2 selected elements" }));
		const confirm = await screen.findByRole("dialog", { name: "Delete 2 elements?" });
		expect(mocks.delete).not.toHaveBeenCalled();
		fireEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));
		await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
		expect(mocks.delete).not.toHaveBeenCalled();

		// Delete asks the same question, and only the confirmation deletes: all of them, in one step.
		fireEvent.keyDown(window, { key: "Delete" });
		fireEvent.click(
			within(await screen.findByRole("dialog", { name: "Delete 2 elements?" })).getByRole("button", {
				name: "Delete all 2",
			}),
		);
		await waitFor(() => expect(mocks.delete).toHaveBeenCalledTimes(1));
		expect(mocks.delete).toHaveBeenCalledWith(9, [fixtureId, secondFixtureId]);
	});

	it("selects only the element a row of the selection list names", async () => {
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			selectedIds: [fixtureId, secondFixtureId],
			entities: [
				...snapshot.entities,
				{
					...snapshot.entities[0],
					id: secondFixtureId,
					logicalFixtureId: secondFixtureId,
					name: "Wash Left",
					fixtureDisplayId: "102",
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const list = await screen.findByRole("list", { name: "Selected elements" });
		fireEvent.click(within(list).getByRole("button", { name: /Wash Left/ }));
		await waitFor(() => expect(mocks.replaceSelection).toHaveBeenCalledWith(4, [secondFixtureId]));
	});

	it("edits one multi-patch copy in Info without moving the fixture or its other copies", async () => {
		const copyId = "44444444-4444-4444-8444-444444444444";
		const [original] = snapshot.entities;
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			entities: [
				original,
				{ ...original, id: copyId, dmxAddress: "2.1", positionMillimetres: [2000, 0, 4000] },
			],
		});
		const copy = {
			id: copyId,
			name: "",
			splitPatches: [],
			location: { x: 2000, y: 0, z: 4000 },
			rotation: { x: 0, y: 0, z: 0 },
		};
		documentMocks.patchSnapshot.mockResolvedValue({
			fixtures: [
				{
					fixtureId,
					name: "Profile Stage 1",
					location: { x: 0, y: 0, z: 4000 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [copy],
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const info = await screen.findByRole("region", { name: "Info" });
		const chooser = within(info).getByRole("combobox", { name: "Copy" });
		expect(within(chooser).getAllByRole("option").map((option) => option.textContent)).toEqual([
			"Original · 1.1",
			"Copy 1 · 2.1",
		]);
		// Notes belong to the fixture, so they say they change every copy.
		expect(within(info).getByLabelText("Notes (all copies)")).toBeInTheDocument();

		fireEvent.change(chooser, { target: { value: copyId } });
		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		await waitFor(() =>
			expect(within(info).getByLabelText("Position X")).toHaveValue("2"),
		);
		const x = within(info).getByLabelText("Position X");
		fireEvent.change(x, { target: { value: "3.5" } });
		fireEvent.keyDown(x, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(1));
		const written = transportMocks.patchFixtures.mock.calls[0][2].fixtures[0];
		expect(written.location).toEqual({ x: 0, y: 0, z: 4000 });
		expect(written.multipatch).toEqual([{ ...copy, location: { x: 3500, y: 0, z: 4000 } }]);

		// A copy's name is its own; the fixture keeps its name.
		fireEvent.click(screen.getByRole("tab", { name: "Generic" }));
		const name = within(info).getByLabelText("Name");
		fireEvent.change(name, { target: { value: "Profile Stage 1 SR" } });
		fireEvent.blur(name);
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(2));
		const renamed = transportMocks.patchFixtures.mock.calls[1][2].fixtures[0];
		expect(renamed.name).toBe("Profile Stage 1");
		expect(renamed.multipatch[0].name).toBe("Profile Stage 1 SR");
	});

	it("sizes a generated Venue object in Info by its profile's measurements instead of a scale", async () => {
		const curtainId = "55555555-5555-4555-8555-555555555555";
		const [original] = snapshot.entities;
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			selectedIds: [curtainId],
			entities: [
				{
					...original,
					id: curtainId,
					logicalFixtureId: curtainId,
					name: "Curtain (parametric)",
					kind: "venue",
					scenery: { kind: "curtain", chords: 0, pattern: "standard" },
				},
			],
		});
		documentMocks.patchSnapshot.mockResolvedValue({
			fixtures: [
				{
					fixtureId: curtainId,
					name: "Curtain (parametric)",
					profileId: "curtain-profile",
					profileRevision: 1,
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					scenerySizeMetres: null,
				},
			],
			profileRevisions: [
				{
					profileId: "curtain-profile",
					profileRevision: 1,
					profileSnapshot: {
						scenery: {
							kind: "curtain",
							chords: 0,
							default_size_metres: { x: 4, y: 6, z: 0.06 },
							adjustable: { width: true, height: true, depth: false },
							minimum_size_metres: { x: 0.5, y: 1, z: 0.06 },
							maximum_size_metres: { x: 30, y: 20, z: 0.06 },
						},
					},
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const info = await screen.findByRole("region", { name: "Info" });
		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		const width = await within(info).findByLabelText("Width");
		expect(width).toHaveValue("4");
		expect(within(info).getByLabelText("Height")).toHaveValue("6");
		// Only the measurements the profile lets the operator set, and no scale beside them.
		expect(within(info).queryByLabelText("Depth")).toBeNull();
		expect(within(info).queryByLabelText("Scale")).toBeNull();
		// The unit is shown inside each field.
		expect(width.parentElement?.querySelector(".cad-field-unit")?.textContent).toBe("m");

		// A size outside the profile's range is put back rather than written.
		fireEvent.change(width, { target: { value: "40" } });
		fireEvent.keyDown(width, { key: "Enter" });
		expect(width).toHaveValue("4");
		expect(transportMocks.patchFixtures).not.toHaveBeenCalled();

		fireEvent.change(width, { target: { value: "8.5" } });
		fireEvent.keyDown(width, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(transportMocks.patchFixtures.mock.calls[0][2].fixtures[0].scenerySizeMetres).toEqual({
			x: 8500,
			y: 6000,
			z: 60,
		});
	});

	it("puts a PA speaker on a pole stand from Info and sets the pole's height", async () => {
		const paId = "57575757-5757-4575-8575-575757575757";
		const [original] = snapshot.entities;
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			selectedIds: [paId],
			entities: [{ ...original, id: paId, logicalFixtureId: paId, name: "PA Speaker", kind: "venue" }],
		});
		const fixture = {
			fixtureId: paId,
			name: "PA Speaker",
			profileId: "pa-profile",
			profileRevision: 1,
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
			scenerySizeMetres: null,
		};
		const revision = {
			profileId: "pa-profile",
			profileRevision: 1,
			profileSnapshot: {
				scenery: {
					kind: "pa_top",
					chords: 0,
					default_size_metres: { x: 0.35, y: 0.6, z: 0.4 },
					adjustable: { width: false, height: true, depth: false },
					minimum_size_metres: { x: 0.35, y: 0.6, z: 0.4 },
					maximum_size_metres: { x: 0.35, y: 2.6, z: 0.4 },
				},
			},
		};
		documentMocks.patchSnapshot.mockResolvedValue({ fixtures: [fixture], profileRevisions: [revision] });
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const info = await screen.findByRole("region", { name: "Info" });
		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		// On its own cabinet: the stand is off and there is no pole to set.
		const stand = await within(info).findByRole("checkbox", { name: "Pole stand" });
		expect(stand).not.toBeChecked();
		expect(within(info).queryByLabelText("Pole")).toBeNull();
		expect(within(info).queryByLabelText("Height")).toBeNull();
		fireEvent.click(stand);
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(1));
		// The stand goes up at 1.2 m of pole under the 0.6 m cabinet.
		expect(transportMocks.patchFixtures.mock.calls[0][2].fixtures[0].scenerySizeMetres).toEqual({
			x: 350,
			y: 1800,
			z: 400,
		});
	});

	it("offers a corrected profile to an element built from an older version of it", async () => {
		const trussId = "66666666-6666-4666-8666-666666666666";
		const [original] = snapshot.entities;
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			selectedIds: [trussId],
			entities: [
				{
					...original,
					id: trussId,
					logicalFixtureId: trussId,
					name: "Four-Point Truss",
					kind: "venue",
					scenery: { kind: "truss", chords: 4, pattern: "standard" },
				},
			],
		});
		documentMocks.patchSnapshot.mockResolvedValue({
			fixtures: [
				{
					fixtureId: trussId,
					name: "Four-Point Truss",
					profileId: "truss-profile",
					profileRevision: 4,
					modeId: "truss-mode",
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					scenerySizeMetres: { x: 8000, y: 340, z: 340 },
				},
			],
			profileRevisions: [],
		});
		// The show answers by content: this computer's library holds a different copy.
		documentMocks.fixtureProfileUpdate.mockResolvedValue({
			fromRevision: 4,
			toRevision: 5,
			name: "Four-Point Truss",
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const info = await screen.findByRole("region", { name: "Info" });
		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		const update = await within(info).findByRole("button", { name: /Update to the newest version/u });
		expect(within(info).getByText(/Built from version 4 of its profile/u)).toBeVisible();

		fireEvent.click(update);
		await waitFor(() => expect(documentMocks.updateFixtureProfile).toHaveBeenCalledWith(trussId));
		// The offer goes once it has been taken.
		await waitFor(() =>
			expect(within(info).queryByRole("button", { name: /Update to the newest version/u })).toBeNull(),
		);
	});

	it("patches a lamp under Generic and sets its bracket and barn doors under Placement", async () => {
		documentMocks.patchSnapshot.mockResolvedValue({
			fixtures: [
				{
					fixtureId,
					name: "Profile Stage 1",
					splitPatches: [{ split: 1, universe: 1, address: 1 }],
					location: { x: 0, y: 0, z: 4000 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					bracketAngle: 0,
					shaperAngle: null,
				},
			],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const info = await screen.findByRole("region", { name: "Info" });
		const patch = await within(info).findByLabelText("Patch");
		await waitFor(() => expect(patch).toHaveValue("1.1"));
		// An address outside a universe is put back rather than written.
		fireEvent.change(patch, { target: { value: "2.600" } });
		fireEvent.keyDown(patch, { key: "Enter" });
		expect(patch).toHaveValue("1.1");
		expect(transportMocks.patchFixtures).not.toHaveBeenCalled();
		fireEvent.change(patch, { target: { value: "2.17" } });
		fireEvent.keyDown(patch, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(transportMocks.patchFixtures.mock.calls[0][2].fixtures[0].splitPatches).toEqual([
			{ split: 1, universe: 2, address: 17 },
		]);

		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		const barndoors = within(info).getByLabelText("Barndoors");
		expect(barndoors).toHaveValue("");
		fireEvent.change(barndoors, { target: { value: "15" } });
		fireEvent.keyDown(barndoors, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(2));
		expect(transportMocks.patchFixtures.mock.calls[1][2].fixtures[0].shaperAngle).toBe(15);
		const bracket = within(info).getByLabelText("Bracket angle");
		fireEvent.change(bracket, { target: { value: "-30" } });
		fireEvent.keyDown(bracket, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(3));
		expect(transportMocks.patchFixtures.mock.calls[2][2].fixtures[0].bracketAngle).toBe(-30);
	});

	it("spreads a THRU range over several selected lamps and lays them out with the assistant", async () => {
		const second = {
			...snapshot.entities[0],
			id: secondFixtureId,
			logicalFixtureId: secondFixtureId,
			name: "Wash Left",
			fixtureDisplayId: "102",
		};
		mocks.snapshot.mockResolvedValue({
			...snapshot,
			selectedIds: [fixtureId, secondFixtureId],
			entities: [...snapshot.entities, second],
		});
		const lamp = (id: string, x: number) => ({
			fixtureId: id,
			name: id,
			splitPatches: [],
			location: { x, y: 0, z: 4000 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
			shaperAngle: null,
		});
		documentMocks.patchSnapshot.mockResolvedValue({
			fixtures: [lamp(secondFixtureId, 2000), lamp(fixtureId, 0)],
		});
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const info = await screen.findByRole("region", { name: "Info" });
		fireEvent.click(screen.getByRole("tab", { name: "Placement" }));
		const x = await within(info).findByLabelText("Position X");
		// The values are read in selection order, so an even spread shows as its two ends.
		await waitFor(() => expect(x).toHaveValue("0 THRU 2"));
		expect(within(info).getByLabelText("Position Z")).toHaveValue("4");
		expect(within(info).getByLabelText("Barndoors")).toHaveAttribute("placeholder", "None");

		fireEvent.change(x, { target: { value: "1 … 3" } });
		fireEvent.keyDown(x, { key: "Enter" });
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(1));
		expect(
			transportMocks.patchFixtures.mock.calls[0][2].fixtures.map(
				(fixture: { fixtureId: string; location: { x: number } }) => [fixture.fixtureId, fixture.location.x],
			),
		).toEqual([
			[fixtureId, 1000],
			[secondFixtureId, 3000],
		]);

		const rotation = within(info).getByLabelText("Rotation Z");
		fireEvent.change(rotation, { target: { value: "not a range" } });
		fireEvent.keyDown(rotation, { key: "Enter" });
		expect(rotation).toHaveValue("0");

		fireEvent.click(within(info).getByRole("button", { name: "Placement Assistant" }));
		const assistant = await screen.findByRole("dialog", { name: "Placement Assistant" });
		fireEvent.click(within(assistant).getByRole("button", { name: "Circle" }));
		const radius = within(assistant).getByLabelText("Radius");
		fireEvent.change(radius, { target: { value: "5" } });
		fireEvent.keyDown(radius, { key: "Enter" });
		fireEvent.click(within(assistant).getByRole("button", { name: "Apply" }));
		await waitFor(() => expect(transportMocks.patchFixtures).toHaveBeenCalledTimes(2));
		// A whole circle of two, around the middle of where they stood.
		expect(
			transportMocks.patchFixtures.mock.calls[1][2].fixtures.map(
				(fixture: { location: unknown }) => fixture.location,
			),
		).toEqual([
			{ x: 7000, y: 0, z: 4000 },
			{ x: -3000, y: 0, z: 4000 },
		]);
		expect(screen.queryByRole("dialog", { name: "Placement Assistant" })).toBeNull();
	});

	it("fits automatically when the view changes and rotates only top down", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const direction = await screen.findByRole("combobox", {
			name: "View direction",
		});
		const canvas = screen.getByTestId("cad-canvas");
		expect(canvas).toHaveAttribute("data-zoom", "0.08");
		fireEvent.change(direction, { target: { value: "left_to_right" } });
		expect(canvas).toHaveAttribute("data-zoom", "0.18");
		expect(
			screen.queryByRole("button", { name: /Rotate top-down view/ }),
		).not.toBeInTheDocument();

		fireEvent.change(direction, { target: { value: "top_down" } });
		const orientation = screen.getByRole("img", {
			name: "Orientation: right +X, up +Y, depth +Z",
		}).parentElement as HTMLElement;
		expect(
			within(orientation).getAllByRole("button", {
				name: /Rotate top-down view/,
			}),
		).toHaveLength(2);
		expect(orientation.querySelectorAll("svg")).toHaveLength(2);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Rotate top-down view 90 degrees clockwise",
			}),
		);
		expect(canvas).toHaveAttribute("data-rotation", "1");
		expect(
			screen.getByLabelText("Orientation: right +Y, up −X, depth +Z"),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", {
				name: "Rotate top-down view 90 degrees counterclockwise",
			}),
		);
		expect(canvas).toHaveAttribute("data-rotation", "0");
	});

	it("keeps snapping inside the Settings modal", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", { name: "Architect Settings" });
		const snapping = within(settings).getByRole("switch", {
			name: "Enable snapping",
		});
		expect(snapping).toBeChecked();
		fireEvent.click(snapping);
		expect(snapping).not.toBeChecked();
		expect(
			within(settings).getByRole("switch", { name: "Show fixture IDs" }),
		).not.toBeChecked();
		expect(
			within(settings).getByRole("switch", { name: "Show DMX addresses" }),
		).not.toBeChecked();
		const origins = within(settings).getByRole("switch", {
			name: "Show coordinate origins",
		});
		expect(origins).not.toBeChecked();
		fireEvent.click(origins);
		expect(origins).toBeChecked();
		expect(screen.getByTestId("cad-canvas")).toHaveAttribute(
			"data-coordinate-origins",
			"visible",
		);
		expect(
			JSON.parse(
				localStorage.getItem("tosklight:viz-editor:cad-settings:v1") ?? "{}",
			).showCoordinateOrigins,
		).toBe(true);
		expect(
			screen.queryByText("Snap to declared truss mounts"),
		).not.toBeInTheDocument();
	});

	it("sets the grid colour as a setting of this computer, never of the show", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", { name: "Architect Settings" });
		fireEvent.click(within(settings).getByRole("tab", { name: "Grid" }));
		const stored = () =>
			JSON.parse(localStorage.getItem("tosklight:viz-editor:cad-settings:v1") ?? "{}")
				.gridColour;

		const hex = within(settings).getByRole("textbox", { name: "Grid colour" });
		expect(hex).toHaveValue("#c9d1d9");
		fireEvent.change(hex, { target: { value: "ABC" } });
		fireEvent.keyDown(hex, { key: "Enter" });
		expect(stored()).toBe("#aabbcc");
		expect(hex).toHaveValue("#aabbcc");

		// Something that is not a colour is put back rather than saved.
		fireEvent.change(hex, { target: { value: "blue-ish" } });
		fireEvent.blur(hex);
		expect(hex).toHaveValue("#aabbcc");
		expect(stored()).toBe("#aabbcc");

		fireEvent.click(within(settings).getByRole("button", { name: "Amber" }));
		expect(stored()).toBe("#e3b341");
		expect(within(settings).getByRole("button", { name: "Amber" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		// The show is not touched: nothing is written into the document.
		expect(documentMocks.savePaperwork).not.toHaveBeenCalled();
		expect(transportMocks.patchFixtures).not.toHaveBeenCalled();
	});

	it("recursively adds adjacent viewports from all four tile edges", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		expect(
			screen.getAllByRole("button", { name: /Add viewport/ }),
		).toHaveLength(4);
		fireEvent.click(screen.getByRole("button", { name: "Add viewport right" }));
		await waitFor(() =>
			expect(screen.getAllByTestId("cad-canvas")).toHaveLength(2),
		);
		expect(
			screen.getAllByRole("button", { name: /Add viewport/ }),
		).toHaveLength(8);
		fireEvent.click(
			screen.getAllByRole("button", { name: "Add viewport bottom" })[0],
		);
		await waitFor(() =>
			expect(screen.getAllByTestId("cad-canvas")).toHaveLength(3),
		);
		expect(
			localStorage.getItem("tosklight:viz-editor:cad-workspace:v2"),
		).toContain("split");
	});

	it("closes the neighboring pane in the direction of its edge arrow", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		fireEvent.click(
			await screen.findByRole("button", { name: "Add viewport bottom" }),
		);
		await waitFor(() =>
			expect(screen.getAllByTestId("cad-canvas")).toHaveLength(2),
		);
		expect(
			screen.getByRole("button", { name: "Close pane bottom" }),
		).toHaveTextContent("↓");
		expect(
			screen.getByRole("button", { name: "Close pane top" }),
		).toHaveTextContent("↑");

		fireEvent.click(screen.getByRole("button", { name: "Close pane bottom" }));
		await waitFor(() =>
			expect(screen.getAllByTestId("cad-canvas")).toHaveLength(1),
		);
		expect(
			screen.queryByRole("button", { name: /Close pane/ }),
		).not.toBeInTheDocument();
	});

	it("resizes neighboring viewports by dragging their divider", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		fireEvent.click(
			await screen.findByRole("button", { name: "Add viewport right" }),
		);
		const divider = await screen.findByRole("separator", {
			name: "Resize columns",
		});
		Object.defineProperty(divider.parentElement, "getBoundingClientRect", {
			value: () => ({
				left: 0,
				top: 0,
				width: 1000,
				height: 500,
				right: 1000,
				bottom: 500,
			}),
		});
		fireEvent.pointerDown(divider, { pointerId: 1, clientX: 500 });
		fireEvent.pointerMove(divider, { pointerId: 1, clientX: 700 });
		fireEvent.pointerUp(divider, { pointerId: 1, clientX: 700 });
		await waitFor(() =>
			expect(
				localStorage.getItem("tosklight:viz-editor:cad-workspace:v2"),
			).toContain('"ratio":0.7'),
		);
	});

	it("serializes rapid selections from different viewports against the latest revision", async () => {
		mocks.replaceSelection.mockImplementation(
			async (revision: number, ids: string[]) => ({
				revision: revision + 1,
				selectedIds: ids,
			}),
		);
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		fireEvent.click(
			await screen.findByRole("button", { name: "Add viewport right" }),
		);
		const directions = screen.getAllByRole("combobox", {
			name: "View direction",
		});
		fireEvent.change(directions[1], { target: { value: "front_to_back" } });
		const viewports = screen.getAllByTestId("cad-canvas");
		fireEvent.click(viewports[0]);
		fireEvent.click(viewports[1]);
		await waitFor(() =>
			expect(mocks.replaceSelection).toHaveBeenCalledTimes(2),
		);
		expect(mocks.replaceSelection.mock.calls).toEqual([
			[4, [fixtureId]],
			[5, [secondFixtureId]],
		]);
		expect(
			screen.queryByText(/refresh before replacing/i),
		).not.toBeInTheDocument();
	});

	it("re-renders only the viewports, not the CAD around them, while a drag previews", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		fireEvent.click(
			await screen.findByRole("button", { name: "Add viewport right" }),
		);
		const viewports = screen.getAllByTestId("cad-canvas");
		renderCounts.viewBar = 0;
		drawnX.length = 0;
		for (let index = 0; index < 10; index++) fireEvent.pointerMove(viewports[0]);
		// Before the preview store, ten moves re-rendered both tiles' chrome twenty times.
		expect(renderCounts.viewBar).toBe(0);
		// Both viewports follow each move, and nothing else does.
		expect(drawnX).toEqual(Array(20).fill(250));
		expect(viewports[1]).toHaveAttribute("data-preview", "250,0,0");
	});

	it("never draws a released element back at its old position while the move commits", async () => {
		let finishTransform: (outcome: unknown) => void = () => undefined;
		mocks.transform.mockReturnValue(
			new Promise((resolve) => {
				finishTransform = resolve;
			}),
		);
		let sceneDelta: (delta: unknown) => void = () => undefined;
		mocks.onSceneDelta.mockImplementation(async (handler) => {
			sceneDelta = handler;
			return () => undefined;
		});
		const moved = {
			...snapshot,
			sceneRevision: 10,
			entities: [{ ...snapshot.entities[0], positionMillimetres: [250, 0, 4000] }],
		};
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const viewport = await screen.findByTestId("cad-canvas");
		mocks.snapshot.mockResolvedValue(moved);
		fireEvent.pointerMove(viewport);
		drawnX.length = 0;

		fireEvent.pointerUp(viewport);
		await waitFor(() => expect(mocks.transform).toHaveBeenCalledOnce());
		expect(viewport).toHaveAttribute("data-preview", "250,0,0");
		// The show broadcasts the committed scene before the transform call returns: the preview is
		// not added on top of the new position.
		sceneDelta({
			sceneRevision: 10,
			drawings: [],
			upserted: moved.entities,
			attachments: [],
		});
		finishTransform({ sceneRevision: 10, transforms: [], attachments: [] });
		await waitFor(() => expect(viewport).toHaveAttribute("data-preview", "none"));
		expect(drawnX.length).toBeGreaterThan(0);
		expect(drawnX.every((x) => x === 250)).toBe(true);
	});

	it("returns a refused move to its old position and says why", async () => {
		mocks.transform.mockRejectedValue(new Error("The fixture is locked"));
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const viewport = await screen.findByTestId("cad-canvas");
		fireEvent.pointerMove(viewport);
		fireEvent.pointerUp(viewport);
		expect(await screen.findByText("Error: The fixture is locked")).toBeInTheDocument();
		expect(viewport).toHaveAttribute("data-preview", "none");
		expect(drawnX.at(-1)).toBe(0);
	});

	it("shares a live world transform across tiles and commits once on release", async () => {
		mocks.transform.mockResolvedValue({ sceneRevision: 10 });
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		fireEvent.click(
			await screen.findByRole("button", { name: "Add viewport right" }),
		);
		const viewports = screen.getAllByTestId("cad-canvas");
		fireEvent.pointerMove(viewports[0]);
		expect(viewports[0]).toHaveAttribute("data-preview", "250,0,0");
		expect(viewports[1]).toHaveAttribute("data-preview", "250,0,0");
		expect(mocks.transform).not.toHaveBeenCalled();

		fireEvent.pointerUp(viewports[0]);
		await waitFor(() => expect(mocks.transform).toHaveBeenCalledOnce());
		expect(mocks.transform).toHaveBeenCalledWith(
			9,
			[fixtureId],
			[250, 0, 0],
			true,
			false,
		);
		await waitFor(() =>
			expect(screen.getAllByTestId("cad-canvas")[1]).toHaveAttribute(
				"data-preview",
				"none",
			),
		);
	});
});
