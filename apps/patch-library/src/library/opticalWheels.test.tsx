import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FixtureProfile } from "../fixtureProfile";
import { OpticalWheelsSection } from "./opticalWheels";

vi.mock("./assets", () => ({ AssetField: () => null }));
afterEach(cleanup);

function Editor() {
	const [draft, onChange] = useState({
		gobos: [{ slot: 1 }],
		prisms: [],
	} as unknown as FixtureProfile);
	return (
		<>
			<OpticalWheelsSection draft={draft} onChange={onChange} />
			<output data-testid="profile">{JSON.stringify(draft)}</output>
		</>
	);
}

describe("optical wheel authoring", () => {
	it("keeps legacy wheel-one artwork while assigning the same slot to a second wheel", () => {
		render(<Editor />);
		fireEvent.click(screen.getByRole("button", { name: "Add gobo slot" }));
		fireEvent.change(screen.getAllByLabelText("Gobo wheel")[1], {
			target: { value: "2" },
		});
		fireEvent.change(screen.getAllByLabelText("Gobo slot")[1], {
			target: { value: "1" },
		});
		expect(
			JSON.parse(screen.getByTestId("profile").textContent ?? "{}").gobos,
		).toEqual([{ slot: 1 }, { wheel: 2, slot: 1 }]);
	});
	it("authors prism representations per wheel and removes only the selected slot", () => {
		render(<Editor />);
		fireEvent.click(screen.getByRole("button", { name: "Add prism slot" }));
		fireEvent.change(screen.getByLabelText("Prism wheel"), {
			target: { value: "2" },
		});
		fireEvent.change(screen.getByLabelText("Prism copies"), {
			target: { value: "5" },
		});
		fireEvent.change(screen.getByLabelText("Prism spread (degrees)"), {
			target: { value: "8" },
		});
		expect(
			JSON.parse(screen.getByTestId("profile").textContent ?? "{}").prisms,
		).toEqual([
			{
				wheel: 2,
				slot: 1,
				representation: "radial",
				facets: 5,
				spread_degrees: 8,
			},
		]);
		fireEvent.click(screen.getByRole("button", { name: "Remove prism slot" }));
		expect(
			JSON.parse(screen.getByTestId("profile").textContent ?? "{}").prisms,
		).toEqual([]);
	});
});
