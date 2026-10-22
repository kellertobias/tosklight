import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Pane } from "./Pane";

const dispatch = vi.hoisted(() => vi.fn());
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: { stageMode: "select" }, dispatch }),
}));
vi.mock("../../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionView: () => ({ selected: [] }),
	useProgrammingCommandLineView: () => null,
	useProgrammingCommandLineActions: () => null,
}));
vi.mock("../../windows/WindowRegistry", () => ({
	isRegisteredWindow: () => true,
	windowRegistry: { stage: () => <div>Stage body</div> },
}));
afterEach(() => { cleanup(); dispatch.mockClear(); });
it("offers camera navigation in the real custom Stage pane header", () => {
	render(<Pane pane={{ id: "stage", kind: "stage", title: "Stage", x: 0, y: 0, width: 12, height: 8 }} active maximized={false} editing={false} />);
	fireEvent.click(screen.getByRole("tab", { name: "Navigate" }));
	expect(dispatch).toHaveBeenCalledWith({ type: "SET_STAGE_MODE", value: "navigate" });
	fireEvent.click(screen.getByRole("tab", { name: "Select fixtures" }));
	expect(dispatch).toHaveBeenCalledWith({ type: "SET_STAGE_MODE", value: "select" });
});
