import {
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadApp } from "./CadApp";

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
	exportPdf: vi.fn(),
	onSceneDelta: vi.fn(),
	onSelectionDelta: vi.fn(),
}));
const documentMocks = vi.hoisted(() => ({
	current: vi.fn(),
	savePaperwork: vi.fn(),
}));
const nativeWindow = vi.hoisted(() => ({
	close: vi.fn().mockResolvedValue(undefined),
	isFullscreen: vi.fn().mockResolvedValue(false),
	setFullscreen: vi.fn().mockResolvedValue(undefined),
	startDragging: vi.fn().mockResolvedValue(undefined),
}));
const workspace = new Map<string, string>();

vi.mock("./session", () => ({ cadSession: mocks }));
vi.mock("../document/session", () => ({ documentSession: documentMocks }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
	save: vi.fn().mockResolvedValue("/tmp/rig-plan.pdf"),
}));
vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: () => nativeWindow,
}));
vi.mock("./CadViewport", () => ({
	CadViewport: ({
		view,
		rotationQuarterTurns,
		camera,
		onSelection,
		preview,
		onPreview,
		onMove,
		showCoordinateOrigins,
	}: {
		view: string;
		rotationQuarterTurns: number;
		camera: { pan: [number, number]; zoom: number };
		onSelection(change: unknown): void;
		preview: { deltaMillimetres: [number, number, number] } | null;
		onPreview(preview: unknown): void;
		onMove(
			delta: [number, number, number],
			ids: readonly string[],
			spread: boolean,
		): void;
		showCoordinateOrigins: boolean;
	}) => (
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
			onPointerUp={() => {
				onPreview(null);
				onMove([250, 0, 0], [fixtureId], false);
			}}
			onClick={() =>
				onSelection({
					type: "replace",
					ids: [view === "top_down" ? fixtureId : secondFixtureId],
				})
			}
		>
			{view}
		</button>
	),
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
	for (const action of Object.values(nativeWindow)) action.mockClear();
});

describe("the CAD planning window", () => {
	it("uses the shared native chrome and exposes every view from the viewport corner", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		const title = screen.getByText("ToskLight Architect");
		expect(title.closest(".ui-window-header")).toBeInTheDocument();
		expect(await screen.findByText("demo.show")).toBeInTheDocument();
		expect(
			screen.queryByText(/First synchronized 2D planning slice/i),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Close window" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Enter fullscreen" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Move window" }),
		).toBeInTheDocument();
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
			screen.getByLabelText("Orientation: right +X, up −Y, depth +Z"),
		).toBeInTheDocument();
		expect(
			screen.queryByText("Scene r9 · Selection r4"),
		).not.toBeInTheDocument();
		const header = title.closest(".ui-window-header");
		if (!header)
			throw new Error("CAD title was not rendered in a window header");
		expect(
			within(header as HTMLElement)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(["Print", "Undo", "Redo", "⚙"]);
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
		fireEvent.click(screen.getByRole("button", { name: "Print" }));
		fireEvent.click(screen.getByRole("button", { name: "Add New Page" }));
		fireEvent.click(screen.getByRole("button", { name: "Add Fixture List" }));
		expect(
			screen.getByRole("complementary", { name: "Print pages" }),
		).toHaveTextContent("1. Page 1");
		expect(
			screen.getByRole("complementary", { name: "Print pages" }),
		).toHaveTextContent("2. Fixture List");
		expect(screen.getByText("Fixture table")).toBeInTheDocument();
		expect(screen.getAllByText("A4 landscape")).toHaveLength(2);

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
		expect(workspace.get("tosklight:viz-editor:cad-print-pages:v1")).toContain(
			"Page 1",
		);
	});

	it("shows project paperwork and print pages on separate sidebar tabs", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		fireEvent.click(screen.getByRole("button", { name: "Print" }));

		// Pages first: the paperwork must not be taking room from the page list. "Add New Page"
		// is a toolbar control and stays put, so the sidebar's own Export button is the tell.
		expect(screen.getByRole("button", { name: "Export to PDF" })).toBeVisible();
		expect(
			screen.queryByRole("textbox", { name: "Lighting designer" }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("tab", { name: "Project" }));
		expect(
			screen.getByRole("textbox", { name: "Lighting designer" }),
		).toBeVisible();
		// ...and the page list is not competing for the same shallow window.
		expect(
			screen.queryByRole("button", { name: "Export to PDF" }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("tab", { name: "Pages" }));
		expect(screen.getByRole("button", { name: "Export to PDF" })).toBeVisible();
	});

	it("keeps the CAD window mounted while editing and saving project information", async () => {
		render(
			<ModalProvider>
				<CadApp />
			</ModalProvider>,
		);
		await screen.findByTestId("cad-canvas");
		fireEvent.click(screen.getByRole("button", { name: "Print" }));
		// Project paperwork now has its own sidebar tab, so a shallow window can show all of
		// it without competing with the page list.
		fireEvent.click(screen.getByRole("tab", { name: "Project" }));

		const lightingDesigner = screen.getByRole("textbox", {
			name: "Lighting designer",
		});
		const showVersion = screen.getByRole("textbox", { name: "Show version" });
		fireEvent.change(lightingDesigner, { target: { value: "T" } });
		fireEvent.change(showVersion, { target: { value: "v" } });

		expect(lightingDesigner).toHaveValue("T");
		expect(showVersion).toHaveValue("v");
		expect(screen.getByTestId("cad-canvas")).toBeInTheDocument();
		expect(
			screen.getByRole("complementary", { name: "Print pages" }),
		).toBeInTheDocument();

		fireEvent.change(lightingDesigner, {
			target: { value: "Tobias Keller" },
		});
		fireEvent.change(showVersion, { target: { value: "1.2" } });
		fireEvent.click(screen.getByRole("button", { name: "Save project info" }));

		await waitFor(() =>
			expect(documentMocks.savePaperwork).toHaveBeenCalledWith({
				lightingDesigner: "Tobias Keller",
				showVersion: "1.2",
				venue: "",
				contactEmail: "",
				contactPhone: "",
				project: "",
				showDate: "",
			}),
		);
		expect(screen.getByTestId("cad-canvas")).toBeInTheDocument();
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
			name: "Orientation: right +X, up −Y, depth +Z",
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
			screen.getByLabelText("Orientation: right −Y, up −X, depth +Z"),
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
			localStorage.getItem("tosklight:viz-editor:cad-workspace:v1"),
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
				localStorage.getItem("tosklight:viz-editor:cad-workspace:v1"),
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
		expect(screen.getAllByTestId("cad-canvas")[1]).toHaveAttribute(
			"data-preview",
			"none",
		);
	});
});
