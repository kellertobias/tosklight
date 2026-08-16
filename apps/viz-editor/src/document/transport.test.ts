import { beforeEach, describe, expect, it, vi } from "vitest";
import { TauriPatchTransport } from "./transport";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

describe("TauriPatchTransport", () => {
	beforeEach(() => {
		invoke.mockReset();
		invoke.mockResolvedValue({});
	});

	it("forwards deterministic DMX placement intents to the document command", async () => {
		const transport = new TauriPatchTransport();
		const placements = [
			{
				fixtureIds: ["fixture-one", "fixture-two"],
				splits: [
					{
						split: 1,
						universe: 2,
						address: 101,
						mode: { type: "consecutive" as const },
					},
				],
			},
		];

		await transport.patchFixtures("show", 4, {
			requestId: "request-one",
			fixtures: [],
			removeFixtureIds: [],
			placements,
		});

		expect(invoke).toHaveBeenCalledWith("patch_fixtures", {
			mutation: {
				requestId: "request-one",
				fixtures: [],
				removeFixtureIds: [],
				placements,
			},
		});
	});
});
