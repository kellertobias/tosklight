import { describe, expect, it, vi } from "vitest";
import type {
	DynamicDefinitionProjection,
	DynamicUpdateIntent,
	ShowObjectActionOutcome,
} from "../../api/generated/light-wire";
import { createDefaultDynamicDefinition } from "../../windows/DynamicsWindow";
import type { ShowObject } from "../showObjects/contracts";
import { ShowObjectsStore } from "../showObjects/store";
import { DynamicMutationWriter } from "./DynamicMutationWriter";
import { applyDynamicUpdateIntent } from "./dynamicUpdateIntent";

const SHOW_ID = "show-a";
const DYNAMIC_ID = "dynamic-a";

describe("DynamicMutationWriter", () => {
	it("projects an encoder mutation before the server response settles", async () => {
		const { store, object } = readyStore();
		let resolve!: (outcome: ShowObjectActionOutcome) => void;
		const updateDynamic = vi.fn(
			() =>
				new Promise<ShowObjectActionOutcome>((next) => {
					resolve = next;
				}),
		);
		const writer = new DynamicMutationWriter(store, {
			object: vi.fn(),
			updateDynamic,
		});
		const lane = object.body.lanes[0];
		const nextLane = { ...lane, width: 0.42 };
		const pending = writer.update(SHOW_ID, DYNAMIC_ID, {
			type: "replace_lane",
			lane_id: lane.id,
			lane: nextLane,
		});

		expect(store.getSnapshot().dynamics[0].body.lanes[0].width).toBe(0.42);
		expect(store.getSnapshot().pendingObjectKeys).toContain(
			`dynamic:${DYNAMIC_ID}`,
		);

		await vi.waitFor(() => expect(updateDynamic).toHaveBeenCalledOnce());
		resolve(
			outcome(2, {
				type: "replace_lane",
				lane_id: lane.id,
				lane: nextLane,
			}),
		);
		await pending;
		expect(store.getSnapshot().dynamics[0].revision).toBe(2);
		expect(store.getSnapshot().pendingObjectKeys.size).toBe(0);
	});

	it("serializes a burst and advances the expected revision for every write", async () => {
		const { store, object } = readyStore();
		let serverBody = object.body;
		let serverRevision = object.revision;
		const revisions: number[] = [];
		const updateDynamic = vi.fn(
			async (
				_showId: string,
				_id: string,
				expectedRevision: number,
				intent: DynamicUpdateIntent,
			) => {
				revisions.push(expectedRevision);
				expect(expectedRevision).toBe(serverRevision);
				serverBody = applyDynamicUpdateIntent(serverBody, intent);
				serverRevision += 1;
				return actionOutcome(serverRevision, serverBody);
			},
		);
		const writer = new DynamicMutationWriter(store, {
			object: vi.fn(),
			updateDynamic,
		});
		const lane = object.body.lanes[0];
		const writes = Array.from({ length: 10 }, (_, index) =>
			writer.update(SHOW_ID, DYNAMIC_ID, {
				type: "replace_lane",
				lane_id: lane.id,
				lane: { ...lane, width: (index + 1) / 10 },
			}),
		);

		expect(store.getSnapshot().dynamics[0].body.lanes[0].width).toBe(1);
		await Promise.all(writes);
		expect(revisions).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		expect(store.getSnapshot().dynamics[0].revision).toBe(11);
		expect(store.getSnapshot().dynamics[0].body.lanes[0].width).toBe(1);
	});

	it("rolls back the optimistic body and exposes the write error", async () => {
		const { store, object } = readyStore();
		const writer = new DynamicMutationWriter(store, {
			object: vi.fn(),
			updateDynamic: vi.fn().mockRejectedValue(new Error("disk unavailable")),
		});
		const lane = object.body.lanes[0];

		await expect(
			writer.update(SHOW_ID, DYNAMIC_ID, {
				type: "replace_lane",
				lane_id: lane.id,
				lane: { ...lane, width: 0.25 },
			}),
		).rejects.toThrow("disk unavailable");
		expect(store.getSnapshot().dynamics[0].body.lanes[0].width).toBe(
			lane.width,
		);
		expect(store.getSnapshot().error?.message).toBe("disk unavailable");
	});

	it("projects a phase-spread scope change while the server write is pending", async () => {
		const { store } = readyStore();
		let resolve!: (outcome: ShowObjectActionOutcome) => void;
		const updateDynamic = vi.fn(
			() =>
				new Promise<ShowObjectActionOutcome>((next) => {
					resolve = next;
				}),
		);
		const writer = new DynamicMutationWriter(store, {
			object: vi.fn(),
			updateDynamic,
		});
		const intent: DynamicUpdateIntent = {
			type: "set_phase_mode",
			phase_mode: "per_lane",
		};
		const pending = writer.update(SHOW_ID, DYNAMIC_ID, intent);

		expect(store.getSnapshot().dynamics[0].body.phase_mode).toBe("per_lane");
		await vi.waitFor(() => expect(updateDynamic).toHaveBeenCalledOnce());
		resolve(outcome(2, intent));
		await pending;
		expect(store.getSnapshot().dynamics[0].body.phase_mode).toBe("per_lane");
	});
});

function readyStore() {
	const store = new ShowObjectsStore();
	store.reset(SHOW_ID, "server-a");
	const object: ShowObject<"dynamic"> = {
		kind: "dynamic",
		id: DYNAMIC_ID,
		revision: 1,
		updated_at: "2026-07-28T00:00:00Z",
		body: createDefaultDynamicDefinition(1, "intensity", {
			lane: "lane-a",
			definition: DYNAMIC_ID,
		}),
	};
	store.setCollection(SHOW_ID, "dynamic", [object], 1, 1);
	return { store, object };
}

function outcome(
	revision: number,
	intent: DynamicUpdateIntent,
): ShowObjectActionOutcome {
	const { object } = readyStore();
	return actionOutcome(
		revision,
		applyDynamicUpdateIntent(object.body, intent),
	);
}

function actionOutcome(
	revision: number,
	body: DynamicDefinitionProjection,
): ShowObjectActionOutcome {
	return {
		request_id: `request-${revision}`,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: revision,
		event_sequence: revision,
		object: {
			kind: "dynamic",
			id: DYNAMIC_ID,
			revision,
			updated_at: "2026-07-28T00:00:01Z",
			body,
		},
	};
}
