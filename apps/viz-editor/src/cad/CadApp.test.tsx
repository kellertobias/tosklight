import { ModalProvider } from "@tosklight/ui/modals";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadApp } from "./CadApp";

const fixtureId = "11111111-1111-4111-8111-111111111111";
const snapshot = {
	showId: "22222222-2222-4222-8222-222222222222",
	sceneRevision: 9,
	selectionRevision: 4,
	selectedIds: [fixtureId],
	attachments: [],
	entities: [{
		id: fixtureId,
		name: "Profile Stage 1",
		fixtureNumber: 101,
		kind: "profile",
		positionMillimetres: [0, 0, 4000],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [400, 500, 700],
		outputDirection: [0, 1, 0],
	}],
};

const mocks = vi.hoisted(() => ({
	snapshot: vi.fn(),
	replaceSelection: vi.fn(),
	transform: vi.fn(),
	undo: vi.fn(),
	redo: vi.fn(),
	onSceneDelta: vi.fn(),
	onSelectionDelta: vi.fn(),
}));
const nativeWindow = vi.hoisted(() => ({
	close: vi.fn().mockResolvedValue(undefined),
	isFullscreen: vi.fn().mockResolvedValue(false),
	setFullscreen: vi.fn().mockResolvedValue(undefined),
	startDragging: vi.fn().mockResolvedValue(undefined),
}));
const workspace = new Map<string, string>();

vi.mock("./session", () => ({ cadSession: mocks }));
vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: () => nativeWindow,
}));
vi.mock("./CadViewport", () => ({
	CadViewport: ({ view }: { view: string }) => <div data-testid="cad-canvas">{view}</div>,
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
	mocks.onSceneDelta.mockReset().mockResolvedValue(() => undefined);
	mocks.onSelectionDelta.mockReset().mockResolvedValue(() => undefined);
	for (const action of Object.values(nativeWindow)) action.mockClear();
});

describe("the CAD planning window", () => {
	it("uses the shared native chrome and exposes every view from the viewport corner", async () => {
		render(<ModalProvider><CadApp /></ModalProvider>);
		const title = screen.getByText("Rig Planner · CAD");
		expect(title.closest(".ui-window-header")).toBeInTheDocument();
		expect(screen.queryByText(/First synchronized 2D planning slice/i)).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Close window" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Enter fullscreen" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Move window" })).toBeInTheDocument();
		const direction = await screen.findByRole("combobox", { name: "View direction" });
		expect(direction.parentElement).toHaveClass("cad-view-control");
		expect(within(direction).getAllByRole("option").map((option) => option.textContent)).toEqual([
			"Top down",
			"Left to right",
			"Right to left",
			"Front to back",
			"Back to front",
		]);
		expect(screen.getByLabelText("Orientation: right +X, up −Y, depth +Z")).toBeInTheDocument();
		expect(screen.queryByText("Scene r9 · Selection r4")).not.toBeInTheDocument();
		const header = title.closest(".ui-window-header");
		if (!header) throw new Error("CAD title was not rendered in a window header");
		expect(within(header as HTMLElement).getAllByRole("button").map((button) => button.textContent)).toEqual([
			"Settings",
			"Undo",
			"Redo",
			"Fit",
		]);
	});

	it("keeps snapping inside the Settings modal", async () => {
		render(<ModalProvider><CadApp /></ModalProvider>);
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", { name: "CAD Settings" });
		const snapping = within(settings).getByRole("switch", { name: "Enable snapping" });
		expect(snapping).toBeChecked();
		fireEvent.click(snapping);
		expect(snapping).not.toBeChecked();
		expect(screen.queryByText("Snap to declared truss mounts")).not.toBeInTheDocument();
	});

	it("recursively adds adjacent viewports from all four tile edges", async () => {
		render(<ModalProvider><CadApp /></ModalProvider>);
		await screen.findByTestId("cad-canvas");
		expect(screen.getAllByRole("button", { name: /Add viewport/ })).toHaveLength(4);
		fireEvent.click(screen.getByRole("button", { name: "Add viewport right" }));
		await waitFor(() => expect(screen.getAllByTestId("cad-canvas")).toHaveLength(2));
		expect(screen.getAllByRole("button", { name: /Add viewport/ })).toHaveLength(8);
		fireEvent.click(screen.getAllByRole("button", { name: "Add viewport bottom" })[0]);
		await waitFor(() => expect(screen.getAllByTestId("cad-canvas")).toHaveLength(3));
		expect(localStorage.getItem("tosklight:viz-editor:cad-workspace:v1")).toContain("split");
	});

	it("resizes neighboring viewports by dragging their divider", async () => {
		render(<ModalProvider><CadApp /></ModalProvider>);
		fireEvent.click(await screen.findByRole("button", { name: "Add viewport right" }));
		const divider = await screen.findByRole("separator", { name: "Resize columns" });
		Object.defineProperty(divider.parentElement, "getBoundingClientRect", {
			value: () => ({ left: 0, top: 0, width: 1000, height: 500, right: 1000, bottom: 500 }),
		});
		fireEvent.pointerDown(divider, { pointerId: 1, clientX: 500 });
		fireEvent.pointerMove(divider, { pointerId: 1, clientX: 700 });
		fireEvent.pointerUp(divider, { pointerId: 1, clientX: 700 });
		await waitFor(() =>
			expect(localStorage.getItem("tosklight:viz-editor:cad-workspace:v1")).toContain('"ratio":0.7'),
		);
	});
});
