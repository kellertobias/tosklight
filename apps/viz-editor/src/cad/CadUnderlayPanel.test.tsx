import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadUnderlayPanel } from "./CadUnderlayPanel";
import type { CadUnderlay, CadUnderlayPreview } from "./underlays";
import type { CadUnderlays } from "./useCadUnderlays";

const mocks = {
	open: vi.fn(),
	preview: vi.fn(),
};

vi.mock("@tauri-apps/plugin-dialog", () => ({
	open: (...args: unknown[]) => mocks.open(...args),
}));

vi.mock("./underlays", () => ({
	underlaySession: {
		preview: (...args: unknown[]) => mocks.preview(...args),
	},
}));

const preview: CadUnderlayPreview = {
	name: "Ground plan.dxf",
	sourceFormat: "dxf",
	units: "millimetres",
	polylineCount: 412,
	pointCount: 3300,
	extentsMillimetres: [0, 0, 20_000, 12_000],
};

const placed: CadUnderlay = {
	id: "venue",
	name: "Ground plan.dxf",
	sourceFormat: "dxf",
	view: "top_down",
	originMillimetres: [1000, 2000],
	scale: 1,
	rotationDegrees: 0,
	visible: true,
	units: "millimetres",
	geometry: {
		polylines: [],
		extentsMillimetres: [0, 0, 0, 0],
		units: "millimetres",
	},
};

function state(overrides: Partial<CadUnderlays> = {}): CadUnderlays {
	return {
		underlays: [],
		busy: false,
		error: null,
		clearError: vi.fn(),
		place: vi.fn().mockResolvedValue(undefined),
		change: vi.fn().mockResolvedValue(undefined),
		remove: vi.fn().mockResolvedValue(undefined),
		...overrides,
	};
}

describe("the Drawings panel", () => {
	beforeEach(() => {
		mocks.open.mockReset();
		mocks.preview.mockReset();
	});

	it("reports what a chosen drawing holds before anything is placed", async () => {
		mocks.open.mockResolvedValue("/venue/Ground plan.dxf");
		mocks.preview.mockResolvedValue(preview);
		const current = state();
		render(<CadUnderlayPanel state={current} defaultView="top_down" />);

		fireEvent.click(screen.getByRole("button", { name: "Add Drawing" }));

		expect(await screen.findByText("Ground plan.dxf")).toBeInTheDocument();
		expect(screen.getByText("millimetres")).toBeInTheDocument();
		expect(screen.getByText("20 × 12 m")).toBeInTheDocument();
		expect(current.place).not.toHaveBeenCalled();
	});

	it("places the drawing on the axis the operator chose", async () => {
		mocks.open.mockResolvedValue("/venue/Section.dxf");
		mocks.preview.mockResolvedValue(preview);
		const current = state();
		render(<CadUnderlayPanel state={current} defaultView="top_down" />);
		fireEvent.click(screen.getByRole("button", { name: "Add Drawing" }));
		await screen.findByRole("combobox", { name: "Axis" });

		fireEvent.change(screen.getByRole("combobox", { name: "Axis" }), {
			target: { value: "front_to_back" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Place Drawing" }));

		await waitFor(() =>
			expect(current.place).toHaveBeenCalledWith(
				"/venue/Section.dxf",
				"front_to_back",
			),
		);
	});

	it("says what went wrong and places nothing when the file cannot be read", async () => {
		mocks.open.mockResolvedValue("/venue/notes.dxf");
		mocks.preview.mockRejectedValue(
			"notes.dxf holds no lines this version can draw",
		);
		const current = state();
		render(<CadUnderlayPanel state={current} defaultView="top_down" />);

		fireEvent.click(screen.getByRole("button", { name: "Add Drawing" }));

		expect(await screen.findByText(/holds no lines/)).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Place Drawing" }),
		).not.toBeInTheDocument();
		expect(current.place).not.toHaveBeenCalled();
	});

	it("moves a placed drawing in metres of the show", () => {
		const current = state({ underlays: [placed] });
		render(<CadUnderlayPanel state={current} defaultView="top_down" />);

		fireEvent.change(screen.getByLabelText("X (m)"), { target: { value: "3" } });

		expect(current.change).toHaveBeenCalledWith(
			expect.objectContaining({ originMillimetres: [3000, 2000] }),
		);
	});

	it("removes a placed drawing by name", () => {
		const current = state({ underlays: [placed] });
		render(<CadUnderlayPanel state={current} defaultView="top_down" />);

		fireEvent.click(
			screen.getByRole("button", { name: "Remove Ground plan.dxf" }),
		);

		expect(current.remove).toHaveBeenCalledWith("venue");
	});
});
