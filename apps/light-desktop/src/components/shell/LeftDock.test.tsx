import { fireEvent, render, screen, within } from "@testing-library/react";
import { Button } from "@tosklight/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider, useApp } from "../../state/AppContext";
import { LeftDock } from "./LeftDock";

vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShow: () => null,
}));

const storedValues = new Map<string, string>();

beforeEach(() => {
	storedValues.clear();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => storedValues.get(key) ?? null,
		setItem: (key: string, value: string) => storedValues.set(key, value),
		removeItem: (key: string) => storedValues.delete(key),
		clear: () => storedValues.clear(),
	});
});

function DockHarness() {
	const { state, dispatch } = useApp();
	return (
		<>
			<Button
				onClick={() => dispatch({ type: "SET_DOCK_MODE", mode: "builtins" })}
			>
				Show Built-ins
			</Button>
			<Button
				onClick={() =>
					dispatch({ type: "SET_SHIFT_ARMED", value: !state.shiftArmed })
				}
			>
				Toggle Shift
			</Button>
			<span data-testid="active-built-in">{state.builtIn ?? "none"}</span>
			<LeftDock
				presentation={{
					clock: <span>Clock</span>,
					showIdentity: "Show",
					showIndicator: {
						className: "show-status-connected",
						connected: true,
						label: "Show active",
						detail: "Connected",
					},
				}}
			/>
		</>
	);
}

afterEach(() => {
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});

describe("LeftDock Shift Built-ins", () => {
	it("changes the same six dock positions and their targets only while Shift is held", () => {
		render(
			<AppProvider>
				<DockHarness />
			</AppProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Show Built-ins" }));

		const normal = within(
			screen.getByRole("navigation", { name: "Built-ins" }),
		);
		expect(
			normal.getAllByRole("button").map((button) => button.textContent),
		).toEqual([
			"⌖Stage",
			"♙Fixtures",
			"▣Presets",
			"▶Cue Lists",
			"∿Dynamics",
			"▥Channels",
		]);

		fireEvent.click(screen.getByRole("button", { name: "Toggle Shift" }));
		const shifted = within(
			screen.getByRole("navigation", { name: "Shift Built-ins" }),
		);
		const destinations = [
			["DMX", "dmx"],
			["Media", "media"],
			["Groups", "groups"],
			["Timecode", "timecode"],
			["Macro", "macros"],
			["Scheduler", "scheduler"],
		] as const;
		expect(
			shifted.getAllByRole("button").map((button) => button.textContent),
		).toEqual([
			"▥DMX",
			"▤Media",
			"♟Groups",
			"◷Timecode",
			"⚙Macro",
			"◫Scheduler",
		]);
		for (const [label, target] of destinations) {
			fireEvent.click(shifted.getByRole("button", { name: label }));
			expect(screen.getByTestId("active-built-in")).toHaveTextContent(target);
		}

		fireEvent.click(screen.getByRole("button", { name: "Toggle Shift" }));
		expect(
			within(screen.getByRole("navigation", { name: "Built-ins" }))
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual([
			"⌖Stage",
			"♙Fixtures",
			"▣Presets",
			"▶Cue Lists",
			"∿Dynamics",
			"▥Channels",
		]);
	});
});
