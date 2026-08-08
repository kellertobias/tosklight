import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StageHeader } from "./StageHeader";
import type { StageOptionsModel } from "./types";

const app = vi.hoisted(() => ({
	state: {
		stageShowSelection: true,
		stageShowFloorGrid: true,
		stage2dSide: "top",
		stageEnvironmentBrightness: 1,
		stageVizAtmosphere: 0.12,
		stageVizQuality: "high",
		stageVizExposure: 1,
		stageVizLaserBrightness: 1,
		stageVizShowLabels: false,
		stageVizBackground: "#020304",
	},
	dispatch: vi.fn(),
}));

vi.mock("../../state/AppContext", () => ({ useApp: () => app }));
vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => ({
		available: true,
		stagePaneStatus: async () => [null, null],
	}),
}));

const options: StageOptionsModel = {
	mode: "select",
	setMode: vi.fn(),
	view: "2d",
	setView: vi.fn(),
	side2d: "top",
	setSide2d: vi.fn(),
	followPreload: false,
	toggleFollowPreload: vi.fn(),
	groupsVisible: false,
	showSelection: true,
	showFloorGrid: true,
	environmentBrightness: 1,
};

function openSettings(stageOptions: StageOptionsModel = options) {
	const view = render(
		<StageHeader options={stageOptions} selectedCount={0} />,
	);
	fireEvent.click(screen.getByRole("button", { name: /settings/i }));
	return view;
}

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

/**
 * The settings an operator is offered have to be settings that do something.
 *
 * Every Stage is the renderer's picture now, and the three views are not the same kind of picture:
 * the 2D plan is a projection, the 3D view is an unlit diagram, and only 3D Viz simulates light.
 * Offering the same controls on all three would mean offering, twice, a control over something
 * that is not happening.
 */
describe("Stage settings are split between the views", () => {
	it("asks a 2D Stage only which side it is seen from", () => {
		openSettings();
		expect(screen.getByText(/viewed from/i)).toBeTruthy();
		expect(screen.queryByText(/render style/i)).toBeNull();
		expect(screen.queryByText(/environment brightness/i)).toBeNull();
	});

	/*
	 * The 3D view draws boxes and aim lines and simulates no light, so a render style and an
	 * environment brightness would both be controls over nothing. The guidelines are not offered
	 * either: here they are the picture rather than an addition to it.
	 */
	it("offers a 3D Stage no render style, no brightness and no guideline switch", () => {
		openSettings({ ...options, view: "3d" });
		expect(screen.getByText(/floor grid/i)).toBeTruthy();
		expect(screen.queryByText(/render style/i)).toBeNull();
		expect(screen.queryByText(/environment brightness/i)).toBeNull();
		expect(screen.queryByText(/beam guidelines/i)).toBeNull();
	});

	/*
	 * The Viz view draws the beams themselves, so a dotted line down the middle of one says
	 * nothing the beam did not.
	 */
	it("offers the Viz Stage its light settings but no beam guidelines", () => {
		openSettings({ ...options, view: "3d-viz" });
		expect(screen.getByText(/environment brightness/i)).toBeTruthy();
		expect(screen.getByText(/render quality/i)).toBeTruthy();
		expect(screen.getByText(/^Background$/)).toBeTruthy();
		expect(screen.queryByText(/beam guidelines/i)).toBeNull();
	});

	/*
	 * Which GPU answered and which transport the picture came over is the renderer's business. A
	 * Stage that is drawing correctly raises no question that naming the adapter answers.
	 */
	it("does not name the renderer or its transport", () => {
		openSettings({ ...options, view: "3d-viz" });
		expect(screen.queryByText(/shared surface/i)).toBeNull();
		expect(screen.queryByText(/^Renderer$/)).toBeNull();
	});

	/*
	 * The tab is the view. Opening the 3D Viz tab is asking to look at the 3D Viz picture, not to
	 * read its settings while looking at something else.
	 */
	it("offers one tab per view", () => {
		openSettings();
		for (const label of ["2D", "3D", "3D Viz"]) {
			expect(screen.getByRole("tab", { name: label })).toBeTruthy();
		}
	});

	it("switches the view when its tab is opened", () => {
		const setView = vi.fn();
		openSettings({ ...options, setView });
		fireEvent.click(screen.getByRole("tab", { name: "3D Viz" }));
		expect(setView).toHaveBeenCalledWith("3d-viz");
	});

	/*
	 * All three are the renderer's picture, so hiding the ones it cannot draw would mean hiding the
	 * Stage. The pane says what it cannot do instead.
	 */
	it("keeps every view's tab present whether or not the renderer can draw it", () => {
		openSettings();
		expect(screen.getAllByRole("tab")).toHaveLength(3);
	});
});
