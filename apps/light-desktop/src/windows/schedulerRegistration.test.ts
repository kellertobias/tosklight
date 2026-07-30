import { describe, expect, it } from "vitest";
import { decodeShowObjectBody } from "../api/showObjectBodyWire";
import { windowChoices } from "../components/modals/WindowPicker";
import { windowRegistry } from "./WindowRegistry";

describe("Scheduler production window registration", () => {
	it("is available to the Window Picker and live registry", () => {
		expect(windowChoices).toContainEqual(["scheduler", "Scheduler"]);
		expect(windowRegistry.scheduler).toBeDefined();
	});

	it("accepts Scheduler panes in persisted user layouts", () => {
		const decoded = decodeShowObjectBody(
			"user_layout",
			{
				desks: [
					{
						id: "desk",
						name: "Desk",
						panes: [
							{
								id: "scheduler",
								kind: "scheduler",
								title: "Scheduler",
								x: 1,
								y: 1,
								width: 12,
								height: 10,
								schedulerShowList: true,
								schedulerShowCalendar: false,
							},
						],
					},
				],
				activeDeskId: "desk",
				windowSettings: {},
			},
			"layout",
		);
		expect(decoded.desks[0].panes[0]).toMatchObject({
			kind: "scheduler",
			schedulerShowList: true,
			schedulerShowCalendar: false,
		});
	});
});
