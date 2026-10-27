// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { appReducer, initialState } from "../../state/appReducer";
import { DetailedPeopleSetting } from "./DetailedPeopleSetting";

const app = vi.hoisted(() => ({ state: { stageVizDetailedPeople: false }, dispatch: vi.fn() }));
vi.mock("../../state/AppContext", () => ({ useApp: () => app }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("local detailed audience preference", () => {
	it("defaults old layouts off and restores an explicit preference without changing lightweight quality", () => {
		const enabled = appReducer(initialState, { type: "SET_STAGE_OPTIONS", vizDetailedPeople: true });
		expect(enabled.stageVizDetailedPeople).toBe(true);
		expect(enabled.stageVizQuality).toBe("draft");
		const restored = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId, windowSettings: { stageVizDetailedPeople: true } });
		expect(restored.stageVizDetailedPeople).toBe(true);
		const legacy = appReducer(enabled, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId, windowSettings: {} });
		expect(legacy.stageVizDetailedPeople).toBe(false);
		const disabled = appReducer(enabled, { type: "SET_STAGE_OPTIONS", vizDetailedPeople: false });
		expect(disabled.stageVizDetailedPeople).toBe(false);
	});
	it("offers the actual switch while Draft stays lightweight and dispatches only a local picture preference", () => {
		render(<DetailedPeopleSetting />);
		const control = screen.getByRole("switch", { name: "Detailed people" });
		expect(control).not.toBeChecked();
		control.focus();
		fireEvent.click(control);
		expect(app.dispatch).toHaveBeenCalledWith({ type: "SET_STAGE_OPTIONS", vizDetailedPeople: true });
		expect(screen.getByText(/Draft and Lines stay lightweight/)).toBeTruthy();
	});
});
