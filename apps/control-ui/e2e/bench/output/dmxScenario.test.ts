import { describe, expect, it, vi } from "vitest";
import type { DmxSnapshot } from "../../../src/api/types";
import { BrowserDmx } from "./dmxScenario";

describe("low-level logical DMX observations", () => {
	it("reads the latest frame and supports channel, channel-map, and range assertions", async () => {
		const snapshot: DmxSnapshot = {
			revision: 9,
			universes: [{ universe: 2, slots: [10, 20, 30] }],
			overrides: [],
		};
		const dmx = new BrowserDmx({
			request: vi.fn(async () => snapshot),
		});
		expect(await dmx.frame(2)).toEqual([10, 20, 30]);
		await dmx.expect(2).channel(1, 10);
		await dmx.expect(2).channels({ 2: 20, 3: 30 });
		await dmx.expect(2).range(1, [10, 20, 30]);
	});

	it("validates raw addresses and bytes before polling", async () => {
		const dmx = new BrowserDmx({
			request: vi.fn(async () => ({
				revision: 1,
				universes: [],
				overrides: [],
			})),
		});
		await expect(dmx.expect(1).channel(0, 0)).rejects.toThrow(
			"address must be an integer from 1 through 512",
		);
		await expect(dmx.expect(1).channel(1, 300)).rejects.toThrow(
			"byte must be an integer from 0 through 255",
		);
	});
});
