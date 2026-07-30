import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LayoutWindow } from "./LayoutWindow";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	replace: vi.fn(),
	gesture: vi.fn(),
	bootstrapReady: true,
	groups: [] as Array<Record<string, unknown>>,
	selected: new Set<string>(),
}));

vi.mock("../state/AppContext", () => ({
	useApp: () => ({
		state: { layoutGroupId: "" },
		dispatch: mocks.dispatch,
	}),
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: vi.fn(),
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	usePortableGroups: () => mocks.groups,
}));
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useBootstrapReady: () => mocks.bootstrapReady,
}));
vi.mock("./stageWindow/useStageLayout", () => ({
	useStageLayout: () => ({
		positions: {
			a: { x: 0, y: 0, rotation: 0 },
			b: { x: 2, y: 0, rotation: 0 },
		},
		positions3d: {},
	}),
}));
vi.mock("./stageWindow/useStageSelection", () => ({
	useStageSelection: () => ({
		fixtureIds: [...mocks.selected],
		fixtureIdSet: mocks.selected,
		firstFixtureId: [...mocks.selected][0] ?? null,
		applyFixtureGesture: mocks.gesture,
		replaceFixtureIds: mocks.replace,
		clear: vi.fn(),
	}),
}));
vi.mock("./layoutWindow/useLayoutVisualization", () => ({
	useLayoutVisualization: () => ({
		presentations: [
			{
				fixtureId: "a",
				fixtureNumber: 1,
				name: "A",
				color: "rgb(255,0,0)",
				dimmer: 75,
			},
			{
				fixtureId: "b",
				fixtureNumber: 2,
				name: "B",
				color: "rgb(0,0,255)",
				dimmer: 25,
			},
		],
		fixtures: [],
	}),
}));

function group(id: string, name: string, fixtures: string[]) {
	return {
		kind: "group",
		id,
		revision: 1,
		updated_at: "",
		body: { name, fixtures, grid: { method: "stage2d" } },
	};
}

describe("LayoutWindow", () => {
	beforeEach(() => {
		mocks.dispatch.mockReset();
		mocks.replace.mockReset();
		mocks.gesture.mockReset();
		mocks.bootstrapReady = true;
		mocks.selected = new Set();
		mocks.groups = [group("1", "Front", ["a", "b"])];
		if (!HTMLElement.prototype.setPointerCapture)
			HTMLElement.prototype.setPointerCapture = vi.fn();
	});
	afterEach(cleanup);

	it("renders only the configured Group with live intensity and color", () => {
		render(<LayoutWindow compact paneId="layout-a" layoutGroupId="1" />);
		expect(screen.getByRole("button", { name: "Fixture 1, 75%" })).toHaveStyle(
			"--layout-fixture-color: rgb(255,0,0)",
		);
		expect(screen.getByText("25%")).toBeInTheDocument();
	});

	it("keeps two panes bound to distinct persisted Group ids", () => {
		mocks.groups = [group("1", "Front", ["a"]), group("2", "Back", ["b"])];
		render(
			<>
				<LayoutWindow compact paneId="layout-front" layoutGroupId="1" />
				<LayoutWindow compact paneId="layout-back" layoutGroupId="2" />
			</>,
		);

		const front = screen.getByLabelText("Front fixture layout");
		const back = screen.getByLabelText("Back fixture layout");
		expect(
			front.querySelector('[data-layout-fixture-id="a"]'),
		).toBeInTheDocument();
		expect(
			front.querySelector('[data-layout-fixture-id="b"]'),
		).not.toBeInTheDocument();
		expect(
			back.querySelector('[data-layout-fixture-id="b"]'),
		).toBeInTheDocument();
		expect(
			back.querySelector('[data-layout-fixture-id="a"]'),
		).not.toBeInTheDocument();
	});

	it("keeps empty and unavailable Groups distinct", () => {
		mocks.groups = [group("2", "Stored Empty", [])];
		const view = render(
			<LayoutWindow compact paneId="layout-a" layoutGroupId="2" />,
		);
		expect(screen.getByRole("status")).toHaveTextContent(
			"Stored Empty is empty",
		);
		view.rerender(
			<LayoutWindow compact paneId="layout-a" layoutGroupId="missing" />,
		);
		expect(screen.getByRole("status")).toHaveTextContent(
			"Group missing is unavailable",
		);
	});

	it("uses the shared authoritative selection for click, toggle, and range", () => {
		render(<LayoutWindow compact paneId="layout-a" layoutGroupId="1" />);
		const first = screen.getByRole("button", { name: "Fixture 1, 75%" });
		const second = screen.getByRole("button", { name: "Fixture 2, 25%" });
		fireEvent.click(first);
		expect(mocks.replace).toHaveBeenLastCalledWith(["a"]);
		mocks.selected.add("a");
		fireEvent.click(first, { metaKey: true });
		expect(mocks.gesture).toHaveBeenLastCalledWith("a", "remove");
		fireEvent.click(second, { shiftKey: true });
		expect(mocks.replace).toHaveBeenLastCalledWith(["a", "b"]);
	});

	it("marquee-selects intersecting cells in deterministic grid order", () => {
		render(<LayoutWindow compact paneId="layout-a" layoutGroupId="1" />);
		const grid = screen.getByLabelText("Front fixture layout");
		const first = screen.getByRole("button", { name: "Fixture 1, 75%" });
		const second = screen.getByRole("button", { name: "Fixture 2, 25%" });
		vi.spyOn(grid, "getBoundingClientRect").mockReturnValue(
			rect(0, 0, 120, 80),
		);
		vi.spyOn(first, "getBoundingClientRect").mockReturnValue(
			rect(10, 10, 40, 40),
		);
		vi.spyOn(second, "getBoundingClientRect").mockReturnValue(
			rect(70, 10, 40, 40),
		);
		fireEvent.pointerDown(grid, { pointerId: 1, clientX: 0, clientY: 0 });
		fireEvent.pointerMove(grid, { pointerId: 1, clientX: 55, clientY: 55 });
		fireEvent.pointerUp(grid, { pointerId: 1, clientX: 55, clientY: 55 });
		expect(mocks.replace).toHaveBeenLastCalledWith(["a"]);
	});
});

function rect(x: number, y: number, width: number, height: number): DOMRect {
	return {
		x,
		y,
		left: x,
		top: y,
		right: x + width,
		bottom: y + height,
		width,
		height,
		toJSON: () => ({}),
	} as DOMRect;
}
