import { describe, expect, it } from "vitest";
import {
	MAX_VIRTUAL_PLAYBACK_NUMBER,
	isVirtualPlaybackNumberForPage,
	virtualPlaybackBankStart,
	virtualPlaybackNumber,
	virtualPlaybackPage,
} from "./virtualPlaybackAddress";

describe("Virtual Playback page banks", () => {
	it("allocates 300 stable numbers per page", () => {
		expect(virtualPlaybackBankStart(1)).toBe(1_001);
		expect(virtualPlaybackNumber(1, 300)).toBe(1_300);
		expect(virtualPlaybackBankStart(2)).toBe(1_301);
		expect(virtualPlaybackNumber(2, 300)).toBe(1_600);
		expect(virtualPlaybackBankStart(3)).toBe(1_601);
	});

	it("covers all 127 pages without reusing a number", () => {
		expect(virtualPlaybackNumber(127, 300)).toBe(39_100);
		expect(MAX_VIRTUAL_PLAYBACK_NUMBER).toBe(39_100);
		expect(virtualPlaybackPage(1_001)).toBe(1);
		expect(virtualPlaybackPage(1_301)).toBe(2);
		expect(virtualPlaybackPage(39_100)).toBe(127);
	});

	it("rejects a number paired with the wrong page", () => {
		expect(isVirtualPlaybackNumberForPage(1, 1_001)).toBe(true);
		expect(isVirtualPlaybackNumberForPage(2, 1_301)).toBe(true);
		expect(isVirtualPlaybackNumberForPage(2, 1_001)).toBe(false);
		expect(isVirtualPlaybackNumberForPage(1, 1_301)).toBe(false);
		expect(virtualPlaybackPage(1_000)).toBeNull();
		expect(virtualPlaybackPage(39_101)).toBeNull();
	});
});
