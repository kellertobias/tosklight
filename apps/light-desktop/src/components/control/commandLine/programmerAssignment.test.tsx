import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCommandLinePlacement } from "./programmerAssignment";
const screens = vi.hoisted(() => ({ owner_screen_id: "second" }));
vi.mock("../../../features/screens/ScreensContext", () => ({ useScreensOptional: () => ({ screens: { programmer_control_surface: screens, screens: [{ id: "second", name: "Screen1", surface_id: "screen:second" }] } }) }));
afterEach(() => window.history.replaceState({}, "", "/"));
describe("Command line Programmer placement", () => {
	it("names the actual encoder owner on Main without changing ownership", () => {
		const { result } = renderHook(useCommandLinePlacement);
		expect(result.current).toEqual({ surfaceId: "main", programmerAssignment: "Programmer is assigned to Screen1" });
	});
	it("uses the exact sibling screen route and suppresses assignment notice on its owner", () => {
		window.history.replaceState({}, "", "/?screen=second");
		const { result } = renderHook(useCommandLinePlacement);
		expect(result.current).toEqual({ surfaceId: "screen:second", programmerAssignment: undefined });
	});
	it("does not configure Main when a screen attachment has not resolved", () => {
		window.history.replaceState({}, "", "/?screen=unknown");
		const { result } = renderHook(useCommandLinePlacement);
		expect(result.current.surfaceId).toBeNull();
	});
});
