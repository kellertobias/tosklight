// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ParameterControlView } from "./ParameterControlView";
import type { ParameterController } from "./useParameterController";

vi.mock("./EncoderSurfaces", () => ({
	EncoderSurfaces: () => <div data-testid="encoders" />,
}));
vi.mock("./ParameterFamilyTabs", () => ({
	ParameterFamilyTabs: () => <div data-testid="families" />,
}));

describe("ParameterControlView", () => {
	afterEach(cleanup);

	it.each([
		4, 6,
	] as const)("renders the ordinary programmer as a %i-column surface", (visibleEncoderCount) => {
		const { container } = render(
			<ParameterControlView
				controller={{ visibleEncoderCount } as ParameterController}
			/>,
		);

		expect(container.querySelector(".parameter-surfaces")).toHaveStyle({
			gridTemplateColumns: `repeat(${visibleEncoderCount}, minmax(0, 1fr))`,
		});
	});
});
