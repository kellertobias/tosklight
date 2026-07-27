import { describe, expect, it, vi } from "vitest";
import type { LiveAction } from "../generated/light-wire";
import { DynamicsApiClient } from "./dynamics";
import type { LiveClientTransport } from "./transport";

describe("DynamicsApiClient live actions", () => {
	it("sends an authoritative pool toggle on the established command socket", async () => {
		const outcome = {
			request_id: "dynamic-request",
			runtime_instance_id: "11111111-1111-4111-8111-111111111111",
			controller_id: "22222222-2222-4222-8222-222222222222",
			targets: [],
			started: true,
		};
		const sendAction = vi.fn(
			async (_action: LiveAction, _requestId?: string) => outcome,
		);
		const request = vi.fn();
		const client = new DynamicsApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			currentDeskId: vi.fn(),
			sendAction,
		} as unknown as LiveClientTransport);

		await expect(
			client.toggle("33333333-3333-4333-8333-333333333333"),
		).resolves.toBe(outcome);
		expect(sendAction).toHaveBeenCalledOnce();
		const [action, requestId] = sendAction.mock.calls[0];
		expect(action).toEqual({
			type: "dynamic_toggle",
			request: {
				dynamic_id: "33333333-3333-4333-8333-333333333333",
				request: {
					request_id: requestId,
					targets: [],
					overrides: {
						size: 1,
						speed_multiplier: { numerator: 1, denominator: 1 },
						phase_offset_degrees: 0,
					},
					timing: {},
				},
			},
		});
		expect(request).not.toHaveBeenCalled();
	});
});
