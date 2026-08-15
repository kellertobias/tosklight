import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { VisualizerView } from "../../../api/client/visualizerView";
import { VISUALIZER_VIEWS, VisualizerControls } from "./VisualizerControls";

afterEach(cleanup);

/// Drive a `SelectField`, which is a listbox behind a button rather than a native select.
function choose(control: string, option: string) {
	fireEvent.click(screen.getByRole("button", { name: new RegExp(control) }));
	fireEvent.click(screen.getByRole("option", { name: option }));
}

const view: VisualizerView = {
	target: "main",
	mode: "top_down",
	quality: "high",
	exposure: 1,
	ambient: 0.06,
	revision: 4,
	physicsResetGeneration: 0,
};

function renderControls(overrides: Partial<Parameters<typeof VisualizerControls>[0]> = {}) {
	const onSelectMode = vi.fn();
	const onSelectQuality = vi.fn();
	const onSelectTarget = vi.fn();
	const onResetPhysics = vi.fn();
	render(
		<VisualizerControls
			view={view}
			targets={["main"]}
			target="main"
			busy={false}
			error={null}
			onSelectTarget={onSelectTarget}
			onSelectMode={onSelectMode}
			onSelectQuality={onSelectQuality}
			onResetPhysics={onResetPhysics}
			{...overrides}
		/>,
	);
	return { onSelectMode, onSelectQuality, onSelectTarget, onResetPhysics };
}

describe("VisualizerControls", () => {
	it("offers every named view the renderer can present", () => {
		renderControls();
		for (const entry of VISUALIZER_VIEWS) {
			expect(
				screen.getByRole("button", { name: entry.label }),
			).toBeEnabled();
		}
		expect(VISUALIZER_VIEWS).toHaveLength(8);
	});

	it("shows which view the renderer is on and sends the one that is pressed", () => {
		const { onSelectMode } = renderControls();
		expect(
			screen.getByRole("button", { name: "Top Down" }),
		).toHaveAttribute("aria-pressed", "true");
		expect(screen.getByRole("button", { name: "3D Full" })).toHaveAttribute(
			"aria-pressed",
			"false",
		);

		fireEvent.click(screen.getByRole("button", { name: "3D Lines" }));
		expect(onSelectMode).toHaveBeenCalledWith("lines_3d");
	});

	it("sends a quality without naming the view again", () => {
		const { onSelectQuality, onSelectMode } = renderControls();
		choose("Rendering quality", "Ultra");
		expect(onSelectQuality).toHaveBeenCalledWith("ultra");
		expect(onSelectMode).not.toHaveBeenCalled();
	});

	it("orders every renderer quality from Draft through Extreme", () => {
		renderControls();
		fireEvent.click(screen.getByRole("button", { name: /Rendering quality/ }));
		expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
			"Draft",
			"Standard",
			"High",
			"Ultra",
			"Extreme",
		]);
	});

	it("sends the explicit physics reset action", () => {
		const { onResetPhysics } = renderControls();
		fireEvent.click(screen.getByRole("button", { name: "Reset physics scenery" }));
		expect(onResetPhysics).toHaveBeenCalledOnce();
	});

	/// A desk driving one renderer must not be made to choose between renderers.
	it("only offers a renderer chooser when there is more than one", () => {
		renderControls();
		expect(screen.queryByRole("button", { name: /Renderer/ })).toBeNull();
		cleanup();
		const { onSelectTarget } = renderControls({
			targets: ["front-of-house", "main"],
		});
		choose("Renderer", "front-of-house");
		expect(onSelectTarget).toHaveBeenCalledWith("front-of-house");
	});

	it("says so while the view is being read, and says what went wrong", () => {
		renderControls({ view: null });
		expect(screen.getByRole("status")).toHaveTextContent(
			"Reading the visualizer view",
		);
		for (const entry of VISUALIZER_VIEWS) {
			expect(screen.getByRole("button", { name: entry.label })).toBeDisabled();
		}
		cleanup();
		renderControls({ error: "the desk refused: exposure must be within 0.05-4.0" });
		expect(screen.getByRole("status")).toHaveTextContent("the desk refused");
	});

	it("takes no press while an edit is in flight", () => {
		renderControls({ busy: true });
		expect(screen.getByRole("button", { name: "3D Full" })).toBeDisabled();
	});
});
