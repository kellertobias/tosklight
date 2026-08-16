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
const workspace = new Map<string, string>();

vi.mock("./session", () => ({ cadSession: mocks }));
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
});

describe("the CAD planning window", () => {
	it("identifies itself as the first TL-60 slice and exposes every view", async () => {
		render(<ModalProvider><CadApp /></ModalProvider>);
		expect(screen.getByRole("heading", { name: "Rig Planner · CAD" })).toBeInTheDocument();
		expect(screen.getByText("First synchronized 2D planning slice of TL-60")).toBeInTheDocument();
		const direction = await screen.findByRole("combobox", { name: "View direction" });
		expect(within(direction).getAllByRole("option").map((option) => option.textContent)).toEqual([
			"Top down",
			"Left to right",
			"Right to left",
			"Front to back",
			"Back to front",
		]);
		expect(screen.getByText("Profile Stage 1")).toBeInTheDocument();
		expect(screen.getByText("Scene r9 · Selection r4")).toBeInTheDocument();
	});

	it("recursively splits an individual tile into an asymmetric workspace", async () => {
		render(<ModalProvider><CadApp /></ModalProvider>);
		await screen.findByText("Profile Stage 1");
		fireEvent.click(screen.getByRole("button", { name: "Split horizontally" }));
		await waitFor(() => expect(screen.getAllByTestId("cad-canvas")).toHaveLength(2));
		fireEvent.click(screen.getAllByRole("button", { name: "Split vertically" })[0]);
		await waitFor(() => expect(screen.getAllByTestId("cad-canvas")).toHaveLength(3));
		expect(localStorage.getItem("tosklight:viz-editor:cad-workspace:v1")).toContain("split");
	});
});
