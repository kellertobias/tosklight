import type { FixtureProfileScenery } from "@tosklight/patch";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SceneryParameters } from "./CadInfoFields";

const stairs = (handrails: boolean): FixtureProfileScenery => ({
	kind: "stairs",
	chords: 0,
	handrails,
	default_size_metres: { x: 1, y: 0.6, z: 2.8 },
	adjustable: { width: false, height: true, depth: false },
	minimum_size_metres: { x: 1, y: 0.2, z: 2.8 },
	maximum_size_metres: { x: 1, y: 1.2, z: 2.8 },
});

describe("a flight's handrails in Info", () => {
	it("shows the chosen sides, else what the profile has, and writes a new choice", () => {
		const onCommit = vi.fn();
		const { rerender } = render(
			<SceneryParameters scenery={stairs(false)} options={null} shared="" onCommit={onCommit} />,
		);
		const select = screen.getByRole("combobox", { name: "Handrails" });
		expect(select).toHaveValue("none");
		fireEvent.change(select, { target: { value: "right" } });
		expect(onCommit).toHaveBeenCalledWith({ handrails: "right" });

		// A flight made with rails reads as both sides until the operator chooses otherwise.
		rerender(<SceneryParameters scenery={stairs(true)} options={null} shared="" onCommit={onCommit} />);
		expect(screen.getByRole("combobox", { name: "Handrails" })).toHaveValue("both");
		rerender(
			<SceneryParameters
				scenery={stairs(true)}
				options={{ handrails: "left", colourSrgb: "#112233" }}
				shared=""
				onCommit={onCommit}
			/>,
		);
		expect(screen.getByRole("combobox", { name: "Handrails" })).toHaveValue("left");
	});

	it("offers no handrails on anything but stairs", () => {
		render(
			<SceneryParameters
				scenery={{ ...stairs(false), kind: "riser" }}
				options={null}
				shared=""
				onCommit={vi.fn()}
			/>,
		);
		expect(screen.queryByRole("combobox", { name: "Handrails" })).toBeNull();
	});
});
