import { describe, expect, it } from "vitest";
import { decodeShowObjectBody } from "../api/showObjectBodyWire";
import { windowChoices } from "../components/modals/WindowPicker";
import { builtIns } from "../components/shell/LeftDock";
import { appReducer } from "../state/appReducer";
import { initialState } from "../state/initialState";
import { windowRegistry } from "./WindowRegistry";

describe("retired Layout window compatibility", () => {
	it("decodes old layouts, removes their authority, and keeps no launch surface", () => {
		const decoded = decodeShowObjectBody(
			"user_layout",
			{
				desks: [
					{
						id: "legacy",
						name: "Legacy",
						panes: [
							{
								id: "layout",
								kind: "layout",
								title: "Layout",
								x: 1,
								y: 1,
								width: 12,
								height: 10,
								layoutGroupId: "3",
							},
						],
					},
				],
				activeDeskId: "legacy",
				windowSettings: {
					builtIn: "layout",
					lastBuiltIn: "layout",
					layoutGroupId: "4",
				},
			},
			"legacy layout",
		);
		expect(decoded.desks[0].panes[0].kind).toBe("layout");

		const hydrated = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: decoded.desks,
			activeDeskId: decoded.activeDeskId,
			windowSettings: decoded.windowSettings,
		});
		expect(hydrated.desks[0].panes).toEqual([]);
		expect(hydrated.builtIn).toBeNull();
		expect(hydrated.lastBuiltIn).toBe(initialState.lastBuiltIn);
		expect(hydrated.layoutMigrationNotice).toBe(true);
		expect("layoutGroupId" in hydrated).toBe(false);
		expect("layout" in windowRegistry).toBe(false);
		expect(builtIns.some(([kind]) => kind === "layout")).toBe(false);
		expect(windowChoices.some(([kind]) => kind === "layout")).toBe(false);
	});
});
