import { describe, expect, it, vi } from "vitest";
import {
	decodeDynamicRuntimeStopMetadata,
	decodeDynamicRuntimeStopOwner,
	dynamicRuntimeStopRequest,
} from "./dynamicRuntimeStopWire";
import { DynamicsApiClient } from "./client/dynamics";
import type { LiveClientTransport } from "./client/transport";
import type { PlaybackActionRequest } from "./generated/light-wire";
import { cueProjection } from "../features/playbackRuntime/testFixtures";
const ids = {
	dynamicId: "11111111-1111-4111-8111-111111111111",
	instanceId: "22222222-2222-4222-8222-222222222222",
	controllerId: "33333333-3333-4333-8333-333333333333",
};
const owner = {
	kind: "virtual_playback" as const,
	page: 2,
	playback_number: 1303,
};
function transport(
	sendAction: LiveClientTransport["sendAction"],
	value: unknown = { instances: [] },
) {
	return {
		sendAction,
		request: vi.fn(async () => value),
		currentDeskId: () => "desk",
		absoluteUrl: (path: string) => path,
		blob: vi.fn(),
	} as unknown as LiveClientTransport;
}
function outcome(request: PlaybackActionRequest) {
	const base = cueProjection(1303);
	return {
		request_id: request.request_id,
		correlation_id: ids.controllerId,
		requested: request.address,
		resolved: request.address,
		outcome: { status: "applied" },
		durability: "durable",
		projection: {
			scope: base.scope,
			requested: request.address,
			playback_number: 1303,
			target: "dynamic",
			dynamic_id: ids.dynamicId,
			last_known_pool_number: 100,
			embedded: false,
			runtime: null,
		},
		related: [],
		desk: null,
		event_sequence: 12,
		desk_event_sequence: null,
		replayed: false,
	};
}
describe("guarded live Dynamic Stop boundary", () => {
	it.each([
		{ kind: "physical_playback" as const, playback_number: 12 },
		owner,
	])("keeps exact typed address and three source IDs for %j", (target) => {
		const request = dynamicRuntimeStopRequest(target, ids, "request");
		expect(request.action).toEqual({
			type: "runtime_stop_dynamic",
			dynamic_id: ids.dynamicId,
			instance_id: ids.instanceId,
			controller_id: ids.controllerId,
		});
		expect(request.address).toEqual(
			target.kind === "physical_playback"
				? { kind: "playback", playback_number: 12 }
				: { kind: "virtual", page: 2, playback_number: 1303 },
		);
	});
	it.each([
		{ kind: "virtual_playback", page: 0, playback_number: 1303 },
		{ kind: "virtual_playback", page: 128, playback_number: 39103 },
		{ kind: "virtual_playback", page: 255, playback_number: 1303 },
		{ kind: "virtual_playback", page: 2, playback_number: 12 },
		{ kind: "virtual_playback", page: 1, playback_number: 1303 },
		{ kind: "virtual_playback", page: 2, playback_number: 1300 },
		{ kind: "physical_playback", playback_number: 1303 },
		{ kind: "virtual_playback", page: 2, playback_number: 0 },
		{ kind: "physical_playback", playback_number: 65536 },
		{ kind: "physical_playback", playback_number: 12, page: 2 },
		{ kind: "cue", cue_id: ids.dynamicId },
		null,
	])("rejects malformed or undeclared owner %j", (value) => {
		expect(() => decodeDynamicRuntimeStopOwner(value, "$.stop_owner")).toThrow(
			/stop_owner/,
		);
	});
	it("validates additive nested owner metadata and preserves legacy absence", async () => {
		const legacy = { instances: [{ controllers: [{ source: "Cue 1" }] }] };
		expect(decodeDynamicRuntimeStopMetadata(legacy)).toBe(legacy);
		const value = {
			instances: [
				{ controllers: [{ source: "Human label", stop_owner: owner }] },
			],
		};
		const client = new DynamicsApiClient(transport(vi.fn(), value));
		expect(await client.runtime(ids.dynamicId)).toBe(value);
		value.instances[0].controllers[0].stop_owner = {
			...owner,
			foreign: true,
		} as never;
		await expect(client.runtime(ids.dynamicId)).rejects.toThrow(
			/controllers\[0\].stop_owner.foreign/,
		);
	});
	it("uses existing live Playback transport and verifies returned request owner", async () => {
		const send = vi.fn(async (action) => {
			expect(action.type).toBe("playback");
			expect(action.request.action).toEqual({
				type: "runtime_stop_dynamic",
				dynamic_id: ids.dynamicId,
				instance_id: ids.instanceId,
				controller_id: ids.controllerId,
			});
			return outcome(action.request);
		});
		const client = new DynamicsApiClient(transport(send));
		await client.stopRuntimeLive(owner, ids);
		expect(send).toHaveBeenCalledTimes(1);
		send.mockImplementationOnce(async (action) => ({
			...outcome(action.request),
			request_id: "other",
		}));
		await expect(client.stopRuntimeLive(owner, ids)).rejects.toThrow(
			/exact requested Dynamic stop owner/,
		);
		send.mockImplementationOnce(async (action) => ({
			...outcome(action.request),
			requested: { kind: "virtual", page: 3, playback_number: 1303 },
		}));
		await expect(client.stopRuntimeLive(owner, ids)).rejects.toThrow();
	});
	it("accepts semantically identical returned addresses with reordered keys", async () => {
		const send = vi.fn(async (action) => ({
			...outcome(action.request),
			requested: { playback_number: 1303, page: 2, kind: "virtual" },
		}));
		await expect(
			new DynamicsApiClient(transport(send)).stopRuntimeLive(owner, ids),
		).resolves.toBeUndefined();
	});
	it.each([
		{ kind: "physical_playback" as const, playback_number: 1000 },
		{ kind: "virtual_playback" as const, page: 127, playback_number: 39100 },
	])("accepts exact domain boundary owner %j", (target) => {
		expect(decodeDynamicRuntimeStopOwner(target, "$.stop_owner")).toEqual(
			target,
		);
	});

	it("rejects invalid source IDs before sending any action", async () => {
		const send = vi.fn();
		const client = new DynamicsApiClient(transport(send));
		await expect(
			client.stopRuntimeLive(owner, { ...ids, instanceId: "wrong" }),
		).rejects.toThrow(/instance_id/);
		expect(send).not.toHaveBeenCalled();
	});
});
