import { describe, expect, it } from "vitest";
import { decodeShowObjectBody } from "../api/showObjectBodyWire";
import { windowChoices } from "../components/modals/WindowPicker";
import { builtIns } from "../components/shell/LeftDock";
import { appReducer, initialState } from "../state/appReducer";
import { windowRegistry } from "./WindowRegistry";

describe("Running and Macro pane-only registration", () => {
	it("keeps Running and Macro Pool in Open Window while Timecode remains a built-in", () => {
		expect(windowChoices).toContainEqual(["running", "Running"]);
		expect(windowChoices).toContainEqual(["macros", "Macro Pool"]);
		expect(windowRegistry.running).toBeDefined();
		expect(windowRegistry.macros).toBeDefined();

		const builtInKinds = builtIns.map(([kind]) => kind);
		expect(builtInKinds).not.toContain("running");
		expect(builtInKinds).not.toContain("macros");
		expect(builtInKinds).toContain("timecode");
	});

	it("adds both registered pane kinds through the window-picker action", () => {
		const emptyDesk = {
			...initialState,
			activeDeskId: "runtime-test",
			desks: [{ id: "runtime-test", name: "Runtime", panes: [] }],
			windowPicker: { x: 1, y: 1, width: 10, height: 8 },
		};
		const withRunning = appReducer(emptyDesk, {
			type: "ADD_WINDOW",
			kind: "running",
		});
		const withPicker = {
			...withRunning,
			windowPicker: { x: 12, y: 1, width: 10, height: 8 },
		};
		const withMacros = appReducer(withPicker, {
			type: "ADD_WINDOW",
			kind: "macros",
		});

		expect(withMacros.desks[0].panes.map(({ kind }) => kind)).toEqual([
			"running",
			"macros",
		]);
	});

	it("hydrates persisted Running and Macro panes without changing their data", () => {
		const decoded = decodeShowObjectBody(
			"user_layout",
			{
				desks: [
					{
						id: "runtime",
						name: "Runtime",
						panes: [
							{
								id: "running-pane",
								kind: "running",
								title: "My Running",
								x: 1,
								y: 1,
								width: 10,
								height: 8,
								runningFilter: "macro",
							},
							{
								id: "macro-pane",
								kind: "macros",
								title: "My Macros",
								x: 12,
								y: 1,
								width: 10,
								height: 8,
							},
						],
					},
				],
				activeDeskId: "runtime",
				windowSettings: {},
			},
			"layout",
		);
		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: decoded.desks,
			activeDeskId: decoded.activeDeskId,
			windowSettings: decoded.windowSettings,
		});

		expect(hydrated.desks).toEqual(decoded.desks);
		expect(hydrated.desks[0].panes).toMatchObject([
			{
				id: "running-pane",
				kind: "running",
				title: "My Running",
				runningFilter: "macro",
			},
			{ id: "macro-pane", kind: "macros", title: "My Macros" },
		]);
	});
});
