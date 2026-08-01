import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	defaultPoolPresentation,
	poolSurfaceKey,
} from "../../features/poolPresentation/poolPresentation";
import { PoolColorSettings, PoolPaletteSettings } from "./PoolColorSettings";

const actions = vi.hoisted(() => ({
	setMode: vi.fn(async () => undefined),
	setTypeColor: vi.fn(async () => undefined),
	setPresetColor: vi.fn(async () => undefined),
	setItem: vi.fn(async () => undefined),
	resetColor: vi.fn(async () => undefined),
	resetAll: vi.fn(async () => undefined),
}));

vi.mock(
	"../../features/poolPresentation/poolPresentation",
	async (original) => {
		const module =
			await original<
				typeof import("../../features/poolPresentation/poolPresentation")
			>();
		return {
			...module,
			usePoolPresentationSettings: () => ({
				configuration: defaultPoolPresentation(),
				showId: "show-a",
				...actions,
			}),
		};
	},
);

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});

describe("PoolColorSettings", () => {
	it("switches a pane mode without exposing server-wide palette controls", async () => {
		render(
			<PoolColorSettings
				objectType="preset"
				paneId="pane-a"
				presetFamily="position"
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Individual colors" }));
		expect(actions.setMode).toHaveBeenCalledWith(
			poolSurfaceKey("show-a", "preset", "pane-a"),
			"individual",
		);
		await waitFor(() => expect(actions.setMode).toHaveBeenCalledOnce());
		expect(screen.queryByText("Position default")).toBeNull();
		expect(screen.queryByRole("button", { name: /Reset .*color/ })).toBeNull();
		expect(actions.resetColor).not.toHaveBeenCalled();
		expect(actions.resetAll).not.toHaveBeenCalled();
	});

	it("exposes every object and Preset-family default", () => {
		render(<PoolPaletteSettings />);
		for (const label of [
			"Groups",
			"Macros",
			"Dynamics",
			"Cuelists",
			"Sequences",
			"Mixed Presets",
			"Intensity Presets",
			"Color Presets",
			"Position Presets",
			"Beam Presets",
		]) {
			expect(screen.getByText(label)).toBeTruthy();
		}
		expect(
			screen.getByRole("button", { name: "Reset all pool colors" }),
		).toBeTruthy();
	});
});
