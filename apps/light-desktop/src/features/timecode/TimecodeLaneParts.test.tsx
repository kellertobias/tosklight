import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Waveform } from "./TimecodeLaneParts";

function envelopeOf(peaks: readonly number[]) {
	const { container } = render(<Waveform peaks={peaks} />);
	return container.querySelector("path")?.getAttribute("d") ?? "";
}

describe("Waveform", () => {
	it("normalises a quiet track so its loudest peak fills the lane", () => {
		// The same shape recorded ten times quieter has to read the same on the lane, so the
		// envelope is scaled against the track's own loudest bucket rather than against full
		// scale, which would leave a quiet master as a flat line.
		expect(envelopeOf([0.02, 0.1, 0.04])).toBe(envelopeOf([0.2, 1, 0.4]));
		expect(envelopeOf([0.02, 0.1, 0.04])).toContain("1 0 ");
	});

	it("draws silence as the lane's centre line rather than dividing by nothing", () => {
		expect(envelopeOf([0, 0])).toBe("M 0 24 L 1 24 L 1 24 L 0 24 Z");
	});

	it("stands the envelope symmetrically on the lane's centre", () => {
		// A recorded signal is symmetric about its own zero, so half the peak is drawn above the
		// centre line and half below it.
		expect(envelopeOf([1])).toBe("M 0 0 L 0 48 Z");
	});
});
