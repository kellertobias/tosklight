import { describe, expect, it } from "vitest";
import { parameterFamilies } from "./model";

describe("parameter family fallback vocabulary", () => {
	it("offers the accepted current Control and Media identities", () => {
		expect(parameterFamilies.Control).toEqual([
			"control",
			"media.play_mode",
			"media.playback_speed",
			"media.playback_bpm",
			"media.scaling_mode",
		]);
		expect(parameterFamilies.Media).toEqual([
			"media.folder",
			"media.file",
			"audio.folder",
			"audio.file",
			"audio.transport",
			"audio.repeat",
			"audio.volume",
			"media.mask.folder",
			"media.mask.file",
			"media.mask.invert",
		]);
		expect(parameterFamilies.Control).not.toContain("control.mode");
		expect(parameterFamilies.Control).not.toContain("control.speed");
	});
});
