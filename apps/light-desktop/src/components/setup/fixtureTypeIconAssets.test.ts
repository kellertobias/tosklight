import { describe, expect, it } from "vitest";
import { fixtureTypeIconAsset } from "./fixtureTypeIconAssets";

/**
 * The icon map is built from a Vite glob of the shipped SVGs, so the resolved value is a bundled
 * URL in a build and an inlined data URI here. Each shipped icon names itself in its own title,
 * which identifies the icon in either form.
 */
function iconTitleFor(type: string): string {
	const asset = fixtureTypeIconAsset(type);
	const decoded = decodeURIComponent(asset);
	return /<title>fixture type ([^<]+)<\/title>/u.exec(decoded)?.[1] ?? asset;
}

describe("Fixture type icons", () => {
	it("gives an Audio Player its music note rather than a lantern or a projector", () => {
		expect(iconTitleFor("audio_player")).toBe("audio player");
		expect(iconTitleFor("Audio Player")).toBe("audio player");
		expect(iconTitleFor("sound")).toBe("audio player");
	});

	it("keeps a media server on the projector icon", () => {
		expect(iconTitleFor("media_server")).toBe("projector");
		expect(iconTitleFor("video")).toBe("projector");
	});

	it("still falls back to the parcan for an unknown type", () => {
		expect(iconTitleFor("something-nobody-shipped")).toBe("parcan");
	});
});
