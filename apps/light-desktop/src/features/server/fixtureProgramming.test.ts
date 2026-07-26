import { describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import { createFixtureProgrammingActions } from "./fixtureProgramming";
import type { ServerController } from "./model";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const FIXTURE_ID = "22222222-2222-4222-8222-222222222222";

function harness(showRevision: number | null) {
	const showObjectsStore = new ShowObjectsStore();
	showObjectsStore.reset(SHOW_ID);
	if (showRevision != null)
		showObjectsStore.installShowRevision(SHOW_ID, showRevision);
	const generateFixturePresets = vi.fn().mockResolvedValue({
		created: [],
		showRevision: 8,
	});
	const collectionSnapshot = vi.fn().mockResolvedValue({
		objects: [],
		showRevision: 7,
	});
	const model = {
		api: {
			programming: {
				controlFixtureAction: vi.fn(),
				generateFixturePresets,
			},
			showObjects: { collectionSnapshot },
		},
		bootstrap: { active_show: { id: SHOW_ID } },
		showObjectsStore,
		setError: vi.fn(),
	} as unknown as ServerController;
	return {
		actions: createFixtureProgrammingActions(model),
		collectionSnapshot,
		generateFixturePresets,
		showObjectsStore,
	};
}

describe("fixture programming actions", () => {
	it("uses and advances the authoritative Show Objects revision", async () => {
		const {
			actions,
			collectionSnapshot,
			generateFixturePresets,
			showObjectsStore,
		} = harness(7);

		await expect(actions.generateFixturePresets([FIXTURE_ID])).resolves.toEqual(
			{
				created: [],
				showRevision: 8,
			},
		);
		expect(collectionSnapshot).not.toHaveBeenCalled();
		expect(generateFixturePresets).toHaveBeenCalledWith([FIXTURE_ID], 7);
		expect(showObjectsStore.getSnapshot().showRevision).toBe(8);
	});

	it("hydrates the scoped Preset collection when authority is not ready", async () => {
		const {
			actions,
			collectionSnapshot,
			generateFixturePresets,
			showObjectsStore,
		} = harness(null);

		await actions.generateFixturePresets([FIXTURE_ID]);

		expect(collectionSnapshot).toHaveBeenCalledWith(SHOW_ID, "preset");
		expect(generateFixturePresets).toHaveBeenCalledWith([FIXTURE_ID], 7);
		expect(showObjectsStore.getSnapshot().showRevision).toBe(8);
	});
});
