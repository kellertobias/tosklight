import { describe, expect, it, vi } from "vitest";
import { CueThumbnailApiClient } from "./cueThumbnails";
import type { ClientTransport } from "./transport";

describe("CueThumbnailApiClient", () => {
	it("reads a replaced cue picture even when its URL and cue values are unchanged", async () => {
		const original = new Blob(["previous renderer picture"]);
		const replacement = new Blob(["current native stage picture"]);
		let stored = original;
		let cached: Blob | undefined;
		const blob = vi.fn(async (_path: string, init?: RequestInit) => {
			if (init?.cache === "no-store") return stored;
			cached ??= stored;
			return cached;
		});
		const createObjectURL = vi
			.spyOn(URL, "createObjectURL")
			.mockReturnValue("blob:cue-preview");
		try {
			const client = new CueThumbnailApiClient({
				blob,
			} as unknown as ClientTransport);
			await client.imageUrl("show", "cue");
			stored = replacement;
			await client.imageUrl("show", "cue");
			expect(createObjectURL.mock.calls.map(([image]) => image)).toEqual([
				original,
				replacement,
			]);
			expect(blob.mock.calls.map(([path]) => path)).toEqual([
				"/api/v2/cues/cue/thumbnail",
				"/api/v2/cues/cue/thumbnail",
			]);
			expect(blob.mock.calls[1]?.[1]?.headers).toEqual({
				"x-tosk-show": "show",
			});
		} finally {
			createObjectURL.mockRestore();
		}
	});
});
