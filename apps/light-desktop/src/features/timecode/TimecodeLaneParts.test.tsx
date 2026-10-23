import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { audioVolumeLinePoints, volumeKeyframePosition } from "./audioLaneGeometry";
import { AudioVolumeLaneBackground, SpeedGroupLaneBackground, Waveform } from "./TimecodeLaneParts";

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


describe("audio volume axes", () => {
	it("places zero and full volume inside the lane and half volume at its center", () => {
		expect([0, 0.5, 1].map(volumeKeyframePosition)).toEqual([88, 50, 12]);
		expect(audioVolumeLinePoints([
			{ frame: 0, value: 0 }, { frame: 220, value: 0.5 }, { frame: 440, value: 1 },
		], 440)).toBe("0,88 50,50 100,12");
	});

	it("anchors audio width to frame scale rather than extra canvas scroll space", () => {
		const { container, rerender } = render(<AudioVolumeLaneBackground
			content={{ kind: "audio_volume", keyframes: [] }} duration={440} pixelsPerFrame={2}
			waveformPeaks={[0, 1, 0]} />);
		expect(container.querySelector(".timecode-audio-lane-content")).toHaveStyle({ width: "880px" });
		rerender(<AudioVolumeLaneBackground content={{ kind: "audio_volume", keyframes: [] }}
			duration={440} pixelsPerFrame={4} waveformPeaks={[0, 1, 0]} />);
		expect(container.querySelector(".timecode-audio-lane-content")).toHaveStyle({ width: "1760px" });
	});
});


it("keeps the Speed Group line on the frame axis despite extra canvas scroll space", () => {
	const { container } = render(<SpeedGroupLaneBackground name="Movement" duration={440}
		pixelsPerFrame={2} content={{ kind: "speed_group", group: "A", keyframes: [
			{ id: "speed", frame: 220, bpm: 120, phase: 0 },
		] }} />);
	expect(container.querySelector("svg")).toHaveStyle({ width: "880px" });
	expect(container.querySelector("polyline")).toHaveAttribute("points", "0,50 50,50 100,50");
});
