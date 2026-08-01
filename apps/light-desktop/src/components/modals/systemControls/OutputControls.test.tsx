import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ControlActionSemantic } from "../../../api/types";
import { FIXED_FIXTURE_ACTIONS, OutputControls } from "./OutputControls";

afterEach(cleanup);

function renderControls(fixturesSelected: boolean) {
	const onFixtureAction = vi.fn();
	render(
		<OutputControls
			master={100}
			blackout={false}
			ready
			fixtureActionResult=""
			fixturesSelected={fixturesSelected}
			availableFixtureActions={
				new Set(
					FIXED_FIXTURE_ACTIONS.map((action) => action.semantic),
				) as ReadonlySet<ControlActionSemantic>
			}
			onMaster={vi.fn()}
			onBlackout={vi.fn()}
			onFixtureAction={onFixtureAction}
		/>,
	);
	return onFixtureAction;
}

describe("OutputControls fixture action scope", () => {
	it("labels fixed actions for all fixtures when the selection is empty", () => {
		renderControls(false);
		fireEvent.click(screen.getByRole("button", { name: "Actions" }));

		for (const action of FIXED_FIXTURE_ACTIONS.slice(0, 3)) {
			expect(
				screen.getByRole("button", { name: action.all }),
			).toBeEnabled();
		}
		expect(screen.getByRole("button", { name: "Fan Mode" })).toBeEnabled();
	});

	it("labels and dispatches the same fixed actions for the selection", () => {
		const onFixtureAction = renderControls(true);
		fireEvent.click(screen.getByRole("button", { name: "Actions" }));

		for (const action of FIXED_FIXTURE_ACTIONS.slice(0, 3)) {
			fireEvent.click(
				screen.getByRole("button", { name: action.selected }),
			);
			expect(onFixtureAction).toHaveBeenCalledWith(action.semantic, "click");
		}
		fireEvent.click(screen.getByRole("button", { name: "Fan Mode" }));
		fireEvent.click(screen.getByRole("option", { name: "Fan Mode Auto" }));
		expect(onFixtureAction).toHaveBeenCalledWith("fan_auto", "click");
	});

	it("ends momentary pointer interaction when the pointer leaves", () => {
		const onFixtureAction = renderControls(true);
		fireEvent.click(screen.getByRole("button", { name: "Actions" }));
		const lampOn = screen.getByRole("button", {
			name: "Selected Lamps On",
		});

		fireEvent.pointerDown(lampOn);
		fireEvent.pointerLeave(lampOn);

		expect(onFixtureAction).toHaveBeenNthCalledWith(1, "lamp_on", "press");
		expect(onFixtureAction).toHaveBeenNthCalledWith(2, "lamp_on", "release");
	});

	it("returns to identically scoped Master controls", () => {
		renderControls(false);
		const rail = document.querySelector(".system-controls-left-rail");
		fireEvent.click(screen.getByRole("button", { name: "Actions" }));
		expect(screen.getByRole("button", { name: "Masters" })).toBeInTheDocument();
		expect(document.querySelector(".system-controls-left-rail")).toBe(rail);
		fireEvent.click(screen.getByRole("button", { name: "Masters" }));
		expect(
			screen.getByRole("slider", { name: "Grand Master" }),
		).toBeInTheDocument();
	});
});
