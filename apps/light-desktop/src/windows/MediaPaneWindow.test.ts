import { describe, expect, it } from "vitest";
import { mediaFileMutations } from "./MediaPaneWindow";

describe("Media pane programmer transaction", () => {
	it("commits folder and file together for the exact logical layer", () => {
		expect(mediaFileMutations("layer-7", 2, 19)).toEqual([
			{
				action: "set_fixture",
				fixtureId: "layer-7",
				attribute: "media.folder",
				value: { kind: "normalized", value: 2 / 255 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
			{
				action: "set_fixture",
				fixtureId: "layer-7",
				attribute: "media.file",
				value: { kind: "normalized", value: 19 / 255 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
		]);
	});
});
