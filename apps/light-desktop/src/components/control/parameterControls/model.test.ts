import { describe, expect, it } from "vitest";
import { parameterFamilies } from "./model";

describe("parameter family fallback vocabulary", () => {
	it("offers the accepted current Control and Media identities", () => {
		expect(parameterFamilies.Control).toEqual([
			"control",
			"media.play_mode",
			"media.playback_speed",
			"media.playback_bpm",
			"media.playback.blur",
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
			"media.flip_mirror",
			"media.mask.scale.x",
			"media.mask.scale.y",
			"media.mask.position.x",
			"media.mask.position.y",
		]);
		expect(parameterFamilies.Control).not.toContain("control.mode");
		expect(parameterFamilies.Control).not.toContain("control.speed");
	});
});
