import { describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import { createGroupDerivationActions } from "./groupDerivation";
import type { ServerController } from "./model";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

describe("Group derivation actions", () => {
	it("detaches with projected portable membership and never reads playbacks", async () => {
		const showObjectsStore = new ShowObjectsStore();
		showObjectsStore.reset(SHOW_ID);
		showObjectsStore.setCollection(SHOW_ID, "group", [
			group("1", 3, "Source", ["a", "b", "c", "d"]),
			{
				...group("2", 4, "Odd", []),
				body: {
					...group("2", 4, "Odd", []).body,
					derived_from: {
						source_group_id: "1",
						rule: { type: "odd" as const },
					},
				},
			},
		]);
		const client = {
			putObject: vi
				.fn()
				.mockResolvedValue({ revision: 5, event_sequence: null }),
		};
		const model = {
			client,
			setError: vi.fn(),
			bootstrap: { active_show: { id: SHOW_ID } },
			showObjectsStore,
			setSelectedFixtures: vi.fn(),
		};
		Object.defineProperty(model, "playbacks", {
			get: () => {
				throw new Error("detach must not read the Playback facade");
			},
		});

		await createGroupDerivationActions(
			model as unknown as ServerController,
		).detachDerivedGroup("2");

		expect(client.putObject).toHaveBeenCalledWith(
			SHOW_ID,
			"group",
			"2",
			expect.objectContaining({
				fixtures: ["a", "c"],
				derived_from: null,
			}),
			4,
		);
		expect(showObjectsStore.getSnapshot().groups[1]).toMatchObject({
			revision: 5,
			body: { fixtures: ["a", "c"], derived_from: null },
		});
	});
});

function group(
	id: string,
	revision: number,
	name: string,
	fixtures: string[],
) {
	return {
		kind: "group" as const,
		id,
		revision,
		updated_at: "",
		body: { name, fixtures, master: 1 },
	};
}
