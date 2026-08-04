// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import {
	resolveVisibleEncoderCount,
	useVisibleEncoderCount,
	VisibleEncoderCountProvider,
} from "./VisibleEncoderCount";

function EncoderCountProbe() {
	return <output>{useVisibleEncoderCount()}</output>;
}

describe("visible encoder count", () => {
	afterEach(cleanup);

	it("defaults ordinary programmer surfaces to six encoders", () => {
		render(<EncoderCountProbe />);

		expect(screen.getByText("6")).toBeTruthy();
	});

	it("provides a four-encoder software surface", () => {
		render(
			<VisibleEncoderCountProvider count={4}>
				<EncoderCountProbe />
			</VisibleEncoderCountProvider>,
		);

		expect(screen.getByText("4")).toBeTruthy();
	});

	it("keeps attached hardware on its fixed six-encoder contract", () => {
		expect(resolveVisibleEncoderCount(4, true)).toBe(6);
		expect(resolveVisibleEncoderCount(4, false)).toBe(4);
	});
});
