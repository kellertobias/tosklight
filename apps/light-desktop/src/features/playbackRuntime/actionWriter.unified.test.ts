import { describe, expect, it, vi } from "vitest";
import { PlaybackRuntimeActionWriter } from "./actionWriter";
import { PlaybackRuntimeStore } from "./store";
import {
	cueProjection,
	DESK_ID,
	SHOW_ID,
	playbackSnapshot,
} from "./testFixtures";
import type { PlaybackOutcome } from "./contracts";
const address = { page: 2, position: 201 };
const source = { surface_id: "screen:two", control_id: 2010 };
function setup() {
	const store = new PlaybackRuntimeStore();
	store.reset(SHOW_ID, DESK_ID);
	const projection = cueProjection(201, 0, 2);
	store.installSnapshot(
		playbackSnapshot([projection.identity], 10, [projection]),
		[projection.identity],
	);
	const applyAction = vi.fn().mockImplementation(
		async (_show, _desk, request): Promise<PlaybackOutcome> => ({
			correlation_id: "55555555-5555-4555-8555-555555555555",
			requested: request.address,
			resolved: request.address,
			outcome: { status: "applied" },
			durability: "durable",
			projection,
			related: [],
			desk: null,
			event_sequence: 11,
			desk_event_sequence: null,
		}),
	);
	const onError = vi.fn();
	const writer = new PlaybackRuntimeActionWriter({
		showId: SHOW_ID,
		deskId: DESK_ID,
		store,
		applyAction,
		onError,
	});
	return { writer, applyAction, onError, store };
}
describe("unified runtime action ownership", () => {
	it("sends exact screen address and stable control source without a durable intent ID", async () => {
		const { writer, applyAction } = setup();
		expect(
			await writer.playbackAction(address, "go", { control_source: source }),
		).not.toBeNull();
		expect(applyAction).toHaveBeenCalledWith(SHOW_ID, DESK_ID, {
			address: { kind: "playback", ...address },
			action: { type: "go", pressed: true },
			control_source: source,
		});
	});
	it("rejects another address's outcome without installing it", async () => {
		const { writer, applyAction, store, onError } = setup();
		applyAction.mockImplementation(async (_show, _desk, request) => ({
			correlation_id: "55555555-5555-4555-8555-555555555555",
			requested: request.address,
			resolved: { kind: "playback", page: 3, position: 201 },
			outcome: { status: "applied" },
			durability: "durable",
			projection: cueProjection(201, 0, 3),
			related: [],
			desk: null,
			event_sequence: 11,
			desk_event_sequence: null,
		}));
		expect(
			await writer.playbackAction(address, "go", { control_source: source }),
		).toBeNull();
		expect(onError).toHaveBeenCalledWith(expect.any(Error));
		expect(store.getSnapshot().projections.has("playback:3.201")).toBe(false);
	});
	it("sends a captured safety release even after authority unmount", async () => {
		const { writer, applyAction } = setup();
		writer.stop();
		await writer.playbackAction(address, "flash", {
			pressed: false,
			control_source: source,
		});
		expect(applyAction).toHaveBeenCalledWith(SHOW_ID, DESK_ID, {
			address: { kind: "playback", ...address },
			action: { type: "flash", pressed: false },
			control_source: source,
		});
	});
});
