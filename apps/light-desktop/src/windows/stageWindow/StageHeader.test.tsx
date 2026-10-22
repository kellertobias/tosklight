import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StageHeader } from "./StageHeader";
import type { StageOptionsModel } from "./types";

const app = vi.hoisted(() => ({
	state: {
		stageShowSelection: true,
		stageShowFloorGrid: true,
		stageShowBeamGuides: false,
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
	showBeamGuides: false,
	environmentBrightness: 1,
};

function openSettings(stageOptions: StageOptionsModel = options) {
	const view = render(<StageHeader options={stageOptions} selectedCount={0} />);
	fireEvent.click(screen.getByRole("button", { name: /settings/i }));
	return view;
}

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

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
	it("offers a 3D Stage optional beam direction guidelines", () => {
		openSettings({ ...options, view: "3d" });
		expect(screen.getByText(/floor grid/i)).toBeTruthy();
		expect(screen.queryByText(/render style/i)).toBeNull();
		expect(screen.queryByText(/environment brightness/i)).toBeNull();
		const guidelines = screen.getByRole("switch", {
			name: /beam direction guidelines/i,
		});
		expect(guidelines).not.toBeChecked();
		fireEvent.click(guidelines);
		expect(app.dispatch).toHaveBeenCalledWith({
			type: "SET_STAGE_OPTIONS",
			showBeamGuides: true,
		});
	});

	it("treats a legacy Viz view as lightweight 3D settings", () => {
		openSettings({ ...options, view: "3d-viz" });
		expect(screen.getByRole("switch", { name: /beam direction guidelines/i })).toBeVisible();
		for (const label of [/environment brightness/i, /render quality/i, /^Background$/, /fog/i])
			expect(screen.queryByText(label)).toBeNull();
		expect(screen.queryByRole("tab", { name: "3D Viz" })).toBeNull();
	});

	it("offers one tab per view", () => {
		openSettings();
		for (const label of ["2D", "3D"]) {
			expect(screen.getByRole("tab", { name: label })).toBeTruthy();
		}
	});

	it("switches the view when its tab is opened", () => {
		const setView = vi.fn();
		openSettings({ ...options, setView });
		fireEvent.click(screen.getByRole("tab", { name: "3D" }));
		expect(setView).toHaveBeenCalledWith("3d");
	});

	/*
	 * Both views are the renderer's picture, so hiding the ones it cannot draw would mean hiding the
	 * Stage. The pane says what it cannot do instead.
	 */
	it("keeps every view's tab present whether or not the renderer can draw it", () => {
		openSettings();
		expect(
			within(
				screen.getByRole("dialog", { name: "Stage Settings" }),
			).getAllByRole("tab"),
		).toHaveLength(2);
	});
});
