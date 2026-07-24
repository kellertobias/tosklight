import { describe, expect, it, vi } from "vitest";
import { ConfigurationApiClient } from "./configuration";
import { OutputApiClient } from "./output";
import type { LiveClientTransport } from "./transport";

const AUTHORITY_ID = "11111111-1111-4111-8111-111111111111";
const SHOW_ID = "22222222-2222-4222-8222-222222222222";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function transportReturning(value: unknown) {
	const commandWithRequestId = vi.fn(async () => value);
	return {
		transport: {
			request: vi.fn(),
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			command: vi.fn(),
			commandWithRequestId,
		} as unknown as LiveClientTransport,
		commandWithRequestId,
	};
}

describe("typed live runtime actions", () => {
	it("sends one correlated Speed Group action frame", async () => {
		const { transport, commandWithRequestId } = transportReturning({
			request_id: "speed-1",
			correlation_id: CORRELATION_ID,
			authority_id: AUTHORITY_ID,
			revision: 5,
			applied_at_millis: 42,
			groups: [
				{
					group: "A",
					manual_bpm: 128,
					paused: false,
					speed_master_scale: 1,
					phase_origin_millis: 42,
				},
			],
			status: "changed",
			event_sequence: 12,
			replayed: false,
			durability: "durable",
		});
		const client = new ConfigurationApiClient(transport);

		await expect(
			client.speedGroupRuntimeLiveAction({
				requestId: "speed-1",
				expectedAuthorityId: AUTHORITY_ID,
				expectedRevision: 4,
				action: { type: "set_bpm", group: "A", bpm: 128 },
			}),
		).resolves.toMatchObject({ requestId: "speed-1", status: "changed" });
		expect(commandWithRequestId).toHaveBeenCalledOnce();
		expect(commandWithRequestId).toHaveBeenCalledWith(
			"speed_group.action",
			{
				request_id: "speed-1",
				expected_authority_id: AUTHORITY_ID,
				expected_revision: 4,
				action: { type: "set_bpm", group: "A", bpm: 128 },
			},
			"speed-1",
		);
	});

	it("keeps combined Grand Master and blackout in one frame", async () => {
		const { transport, commandWithRequestId } = transportReturning({
			request_id: "output-1",
			correlation_id: CORRELATION_ID,
			projection: {
				scope: { show_id: SHOW_ID },
				identity: "global_master",
				revision: 8,
				grand_master: 0.4,
				blackout: true,
			},
			status: "changed",
			event_sequence: 13,
			replayed: false,
			durability: "durable",
		});
		const client = new OutputApiClient(transport);

		await expect(
			client.outputRuntimeLiveAction(SHOW_ID, {
				requestId: "output-1",
				expectedShowId: SHOW_ID,
				expectedRevision: 7,
				grandMaster: 0.4,
				blackout: true,
			}),
		).resolves.toMatchObject({ requestId: "output-1", status: "changed" });
		expect(commandWithRequestId).toHaveBeenCalledOnce();
		expect(commandWithRequestId).toHaveBeenCalledWith(
			"output_runtime.action",
			{
				request_id: "output-1",
				expected_show_id: SHOW_ID,
				expected_revision: 7,
				grand_master: 0.4,
				blackout: true,
			},
			"output-1",
		);
	});
});
