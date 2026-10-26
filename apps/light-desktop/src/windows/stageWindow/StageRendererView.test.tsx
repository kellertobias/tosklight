// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StageRendererView } from "./StageRendererView";
import type { StageOptionsModel } from "./types";
import type { StageSelectionModel } from "./useStageSelection";

const native = vi.hoisted(() =>
	vi.fn(() => ({ active: false, ref: () => undefined })),
);
vi.mock("./useNativeStagePane", () => ({ useNativeStagePane: native }));
vi.mock("./useStagePanePicks", () => ({ useStagePanePicks: vi.fn() }));
vi.mock("./useStagePaneSelection", () => ({ useStagePaneSelection: vi.fn() }));
vi.mock("./useStagePanePicture", () => ({
	useStagePanePicture: vi.fn(),
	stageViewMode: () => "lines_3d",
}));
const captureHook = vi.hoisted(() => vi.fn());
vi.mock("./useStageCapture", () => ({ useStageCapture: captureHook }));
vi.mock("./StageCaptureControls", () => ({ StageCaptureControls: () => <button>Render 4K</button> }));
vi.mock("./NativeStageSurface", () => ({ NativeStageSurface: (props: Record<string, unknown>) => {
    expect(props).not.toHaveProperty("onCapture");
    return null;
} }));
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});

it("only admits a native renderer while its Control Stage pane is active", () => {
	const options = { view: "3d", side2d: "top" } as StageOptionsModel;
	const selection = { fixtureIds: [] } as unknown as StageSelectionModel;
	const view = render(
		<StageRendererView
			options={options}
			selection={selection}
			active={false}
		/>,
	);
	expect(native).toHaveBeenLastCalledWith(false, true);
	view.rerender(
		<StageRendererView options={options} selection={selection} active />,
	);
	expect(native).toHaveBeenLastCalledWith(true, true);
	view.rerender(
		<StageRendererView
			options={{ ...options, view: "2d" }}
			selection={selection}
			active
		/>,
	);
	expect(native).toHaveBeenLastCalledWith(true, false);
});

it.each(["2d", "3d"] as const)("never mounts capture controls or ownership for Light Desk %s", (view) => {
    native.mockReturnValueOnce({ active: true, ref: () => undefined });
    render(<StageRendererView options={{ view, side2d: "top" } as StageOptionsModel}
        selection={{ fixtureIds: [] } as unknown as StageSelectionModel} active />);
    expect(screen.queryByRole("button", { name: /Render 4K/i })).toBeNull();
    expect(captureHook).not.toHaveBeenCalled();
});
