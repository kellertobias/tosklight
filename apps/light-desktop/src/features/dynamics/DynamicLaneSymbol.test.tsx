import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { createDefaultDynamicLane } from "../../windows/dynamics/DynamicsEditor";
import { DynamicLaneSymbol, dynamicLaneFamilies } from "./DynamicLaneSymbol";

const keys = ["intensity", "position.pan", "color.hue", "zoom"];
const families = ["intensity", "position", "color", "other"];

afterEach(cleanup);

describe("Dynamic lane symbol", () => {
	it.each(Array.from({ length: 16 }, (_, mask) => mask))("shows configured family combination %i", (mask) => {
		const lanes = keys.filter((_, index) => mask & (1 << index)).map((key) => createDefaultDynamicLane(key));
		const { container } = render(<DynamicLaneSymbol lanes={lanes} attributes={[]} />);
		families.forEach((family, index) => {
			expect(container.querySelector(`[data-lane-family="${family}"]`)?.getAttribute("data-active")).toBe(String(Boolean(mask & (1 << index))));
		});
		expect(screen.getByRole("img").getAttribute("aria-label")).toMatch(/^Lanes: /);
	});

	it("classifies legacy scalar and registry-defined lanes without losing unknown families", () => {
		const lanes = ["pan.continuous", "color.wheel.1", "shutter", "gobo", "custom"].map((key) => createDefaultDynamicLane(key));
		expect([...dynamicLaneFamilies(lanes, [{ id: "shutter", family: "Intensity" }])]).toEqual(["position", "color", "intensity", "other"]);
	});
});
