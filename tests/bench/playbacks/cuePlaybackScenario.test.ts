import { describe, expect, it, vi } from "vitest";
import type { ApiDriver } from "../core/api";
import {
	cueListForPlayback,
	playbackLocation,
	validatePlaybackAddress,
} from "./cuePlaybackScenario";

function driver(
	assignments: Record<string, unknown> = {
		"1000": { target: { type: "cue_list", cue_list_id: "list" } },
	},
	pageNumber = 99,
) {
	const showObject = vi.fn(async (_show: string, kind: string, id: string) => {
		if (kind === "playback_page" && id === String(pageNumber))
			return { body: { number: pageNumber, assignments } };
		if (kind === "cue_list" && id === "list")
			return { id: "stored-list", body: { id: "list", cues: [] } };
		return null;
	});
	return { api: { showObject } as unknown as ApiDriver, showObject };
}

describe("canonical Cue Playback scenario authority", () => {
	it("loads exact sparse page-position assignment and its Cuelist without a global pool", async () => {
		const { api, showObject } = driver();
		await expect(
			cueListForPlayback(api, "show", { page: 99, position: 1000 }),
		).resolves.toMatchObject({ id: "stored-list" });
		expect(showObject.mock.calls).toEqual([
			["show", "playback_page", "99"],
			["show", "cue_list", "list"],
		]);
	});
	it("keeps equal positions on different pages distinct", async () => {
		const { api } = driver();
		await expect(
			playbackLocation(api, "show", { page: 1, position: 1000 }),
		).rejects.toThrow("1.1000 is absent");
		await expect(
			playbackLocation(api, "show", { page: 99, position: 1000 }),
		).resolves.toEqual({ page: 99, position: 1000 });
	});
	it("rejects an unassigned exact position", async () => {
		const { api, showObject } = driver();
		await expect(
			cueListForPlayback(api, "show", { page: 99, position: 999 }),
		).rejects.toThrow("99.999 is absent");
		expect(showObject).toHaveBeenCalledTimes(1);
	});
	it("rejects a non-Cuelist assignment before any Cuelist lookup", async () => {
		const { api, showObject } = driver({
			"1000": { target: { type: "group", group_id: "group" } },
		});
		await expect(
			cueListForPlayback(api, "show", { page: 99, position: 1000 }),
		).rejects.toThrow("does not target a Cuelist");
		expect(showObject).toHaveBeenCalledTimes(1);
	});
	it.each([
		{ page: 0, position: 1 },
		{ page: 100, position: 1 },
		{ page: 1, position: 0 },
		{ page: 1, position: 1001 },
		{ page: 1, position: 1.5 },
	])("rejects invalid canonical address %j", (address) => {
		expect(() => validatePlaybackAddress(address)).toThrow(/must be between/);
	});
	it("captures a copied address instead of sharing mutable fixture state", () => {
		const address = { page: 1, position: 2 };
		const captured = validatePlaybackAddress(address);
		address.page = 3;
		expect(captured).toEqual({ page: 1, position: 2 });
	});
});
