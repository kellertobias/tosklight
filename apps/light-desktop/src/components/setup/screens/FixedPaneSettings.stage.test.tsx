import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { defaultFixedPane, FixedPaneSettings } from "./FixedPaneSettings";

afterEach(cleanup);

it("defaults additional-screen 3D Stage to wireframe", () => {
	expect(defaultFixedPane("stage_3d")).toMatchObject({ render_quality: "lines_only" });
});

it("keeps fixed-screen Stage controls lightweight even for a legacy full setting", () => {
	const update = vi.fn();
	const pane = { type: "stage_3d", follow_preload: false, show_floor_grid: true, show_beam_guides: false, render_quality: "full", environment_brightness: 1 } as const;
	render(<FixedPaneSettings pane={pane} cueLists={[]} textFiles={[]} update={update} />);
	expect(screen.queryByText("Render quality")).toBeNull();
	expect(screen.queryByText("Environment brightness")).toBeNull();
	fireEvent.click(screen.getByRole("switch", { name: /Beam direction guidelines/ }));
	expect(update).toHaveBeenCalledWith({ ...pane, show_beam_guides: true });
});
