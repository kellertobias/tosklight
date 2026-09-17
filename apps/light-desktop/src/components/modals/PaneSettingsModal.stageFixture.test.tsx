import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaneModel } from "../../types";
import {
	FixtureSheetPaneSettings,
	PaneSettingsModal,
	PatchPaneSettings,
	StagePaneSettings,
} from "./PaneSettingsModal";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	state: {
		paneSettingsId: null as string | null,
		stageShowFloorGrid: true,
		stageShowSelection: true,
		stageVizAtmosphere: 0.2,
		stageEnvironmentBrightness: 0.05,
		stageVizQuality: "ultra" as const,
		stageVizExposure: 1,
		stageVizLaserBrightness: 1,
		stageVizBackground: "#04060a",
		stageVizShowLabels: true,
		activeDeskId: "desk",
		desks: [
			{
				id: "desk",
				panes: [
					{
						id: "stage-pane",
						lampFogCloudiness: 0.35,
						lampFogTurbulence: 0.45,
						laserFogCloudiness: 0.55,
						laserFogTurbulence: 0.65,
					},
					{
						id: "patch-pane",
						kind: "patch",
						title: "Show Patch",
						x: 0,
						y: 0,
						width: 8,
						height: 6,
					},
				],
			},
		],
	},
	sendStagePaneInput: vi.fn(),
	stagePaneStatus: vi.fn().mockResolvedValue(["ready", null]),
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("./cuePaneCuelistAuthority", () => ({
	useCuePaneCuelistPlaybacks: () => [],
}));
vi.mock("../../windows/fixtureSheetCuelistAuthority", () => ({
	useFixtureSheetCuelistAuthority: () => ({
		cueLists: [],
		selectedCueListId: "",
	}),
}));
vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => ({
		sendStagePaneInput: mocks.sendStagePaneInput,
		stagePaneStatus: mocks.stagePaneStatus,
	}),
}));

afterEach(() => {
	cleanup();
	mocks.dispatch.mockReset();
	mocks.sendStagePaneInput.mockReset();
});

const pane = (kind: PaneModel["kind"], options = {}) =>
	({
		id: `${kind}-pane`,
		kind,
		title: kind,
		x: 1,
		y: 1,
		width: 8,
		height: 8,
		...options,
	}) satisfies PaneModel;

describe("Stage pane settings parity", () => {
	it("offers the dedicated Stage controls relevant to each pane view", () => {
		const { rerender } = render(
			<StagePaneSettings pane={pane("stage", { stageView: "3d" })} />,
		);
		expect(screen.getByRole("button", { name: "Reset view" })).toBeVisible();
		expect(screen.getByText("Floor grid")).toBeVisible();
		expect(screen.getByText("Show selection")).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Reset view" }));
		expect(mocks.sendStagePaneInput).toHaveBeenCalledWith(
			"frame",
			0,
			0,
			"stage-pane",
		);

		rerender(
			<StagePaneSettings pane={pane("stage", { stageView: "3d-viz" })} />,
		);
		expect(screen.getByText("Fog / haze")).toBeVisible();
		expect(screen.getByText("Environment brightness")).toBeVisible();
		expect(screen.getByText("Render quality")).toBeVisible();
		expect(screen.getByText("Exposure")).toBeVisible();
		expect(screen.getByText("Fixture labels")).toBeVisible();
		for (const label of [
			"Lamp fog cloudiness",
			"Lamp fog turbulence",
			"Laser fog cloudiness",
			"Laser fog turbulence",
		])
			expect(screen.getByText(label)).toBeVisible();
	});
});

describe("Fixture Sheet pane settings parity", () => {
	it("exposes every per-pane view and column control", () => {
		render(
			<FixtureSheetPaneSettings
				pane={pane("fixtures", {
					fixtureSheetColumns: ["id", "name"],
				})}
				cueLists={[{ id: "front", name: "Front wash" }]}
				selectedCueListId=""
			/>,
		);
		for (const label of [
			"Compact mode",
			"Show active fixtures only",
			"Fixture heads",
			"Ordering",
			"Cuelist",
			"Fixture ID",
			"Patch address",
			"Show fixture type",
		])
			expect(screen.getAllByText(label)[0]).toBeVisible();

		fireEvent.click(screen.getByRole("switch", { name: /Patch address/ }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_FIXTURE_OPTIONS",
			id: "fixtures-pane",
			options: { columns: ["id", "name", "patch"] },
		});
	});
});

describe("Show Patch pane settings", () => {
	it("switches one column at a time on this pane only", () => {
		render(
			<PatchPaneSettings pane={pane("patch", { patchHiddenColumns: ["mib"] })} />,
		);
		expect(screen.getByRole("switch", { name: /MIB/ })).not.toBeChecked();
		expect(screen.getByRole("switch", { name: /Layer/ })).toBeChecked();

		fireEvent.click(screen.getByRole("switch", { name: /Layer/ }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_PATCH_HIDDEN_COLUMNS",
			id: "patch-pane",
			columns: ["mib", "layer"],
		});
		fireEvent.click(screen.getByRole("switch", { name: /MIB/ }));
		expect(mocks.dispatch).toHaveBeenLastCalledWith({
			type: "SET_PANE_PATCH_HIDDEN_COLUMNS",
			id: "patch-pane",
			columns: [],
		});
	});
});

describe("Show Patch pane Import CSV", () => {
	afterEach(() => {
		mocks.state.paneSettingsId = null;
	});

	it("closes the pane settings and asks that pane to open Import CSV", () => {
		mocks.state.paneSettingsId = "patch-pane";
		const requests: unknown[] = [];
		const listen = (event: Event) =>
			requests.push((event as CustomEvent).detail);
		window.addEventListener("light:patch-import-csv", listen);
		try {
			render(<PaneSettingsModal />);
			fireEvent.click(screen.getByRole("button", { name: "Import CSV" }));
		} finally {
			window.removeEventListener("light:patch-import-csv", listen);
		}
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_SETTINGS",
			id: null,
		});
		expect(requests).toEqual([{ paneId: "patch-pane" }]);
	});

	it("offers no Import CSV on other panes", () => {
		mocks.state.paneSettingsId = "stage-pane";
		render(<PaneSettingsModal />);
		expect(screen.queryByRole("button", { name: "Import CSV" })).toBeNull();
	});
});
