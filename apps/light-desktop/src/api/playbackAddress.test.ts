import { describe, expect, it } from "vitest";
import {
	assertPlaybackAddress,
	playbackAddressKey,
	samePlaybackAddress,
} from "./playbackAddress";

describe("exact playback addresses", () => {
	it("keeps the same position on different pages distinct without banking", () => {
		expect(playbackAddressKey({ page: 1, position: 401 })).toBe(
			"playback:1.401",
		);
		expect(playbackAddressKey({ page: 99, position: 401 })).toBe(
			"playback:99.401",
		);
		expect(
			samePlaybackAddress(
				{ page: 1, position: 401 },
				{ page: 99, position: 401 },
			),
		).toBe(false);
	});
	it.each([
		{ page: 0, position: 1 },
		{ page: 100, position: 1 },
		{ page: 1, position: 0 },
		{ page: 1, position: 1001 },
		{ page: 1.5, position: 1 },
		{ page: 1, position: NaN },
	])("rejects an invalid exact address %j", (address) => {
		expect(() => assertPlaybackAddress(address)).toThrow();
	});
	it("accepts the final sparse address", () => {
		expect(playbackAddressKey({ page: 99, position: 1000 })).toBe(
			"playback:99.1000",
		);
	});
});
