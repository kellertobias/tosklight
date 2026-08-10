import { describe, expect, it, vi } from "vitest";
import { ApiRequestError } from "../ApiRequestError";
import {
	AttributeConfigurationApiClient,
	type AttributeConfigurationSnapshot,
} from "./attributeConfiguration";
import type { ClientTransport } from "./transport";

const snapshot: AttributeConfigurationSnapshot = {
	show_id: "show-29",
	show_revision: 8,
	object_revision: 3,
	configuration: {
		version: 1,
		custom_attributes: [],
		placements: [],
		activation_groups: [],
	},
	recommended_configuration: {
		version: 1,
		custom_attributes: [],
		placements: [],
		activation_groups: [],
	},
	descriptors: [],
	validation_error: null,
};

describe("AttributeConfigurationApiClient", () => {
	it("loads and revision-guards show-owned collection patches", async () => {
		const request = vi.fn(async (path: string, _init?: RequestInit) =>
			path.endsWith("/update")
				? {
						request_id: "request-29",
						replayed: false,
						snapshot,
						event_sequence: 4,
					}
				: snapshot,
		);
		const client = new AttributeConfigurationApiClient({
			request,
		} as unknown as ClientTransport);
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000029",
		);

		await client.snapshot("show-29");
		await client.update("show-29", snapshot, {
			placements: [
				{
					attribute: "prism.1.rotation",
					encoder_group: "beam",
					encoder_page: 1,
					encoder_slot: 6,
					push_turn_of: "prism.1",
				},
			],
			activation_groups: [
				{ id: "intensity", label: "Intensity", members: ["intensity"] },
			],
		});

		expect(request.mock.calls[0]?.[0]).toBe("/api/v2/attribute-configuration");
		expect(
			new Headers(request.mock.calls[0]?.[1]?.headers).get("x-tosk-show"),
		).toBe("show-29");
		const [path, init] = request.mock.calls[1] ?? [];
		expect(path).toBe("/api/v2/attribute-configuration/update");
		expect(init?.method).toBe("POST");
		expect(new Headers(init?.headers).get("x-tosk-show")).toBe("show-29");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000029",
			expected_show_revision: 8,
			expected_object_revision: 3,
			patch: {
				placements: [
					{
						attribute: "prism.1.rotation",
						encoder_group: "beam",
						encoder_page: 1,
						encoder_slot: 6,
						push_turn_of: "prism.1",
					},
				],
				activation_groups: [
					{
						id: "intensity",
						label: "Intensity",
						members: ["intensity"],
					},
				],
			},
		});
	});

	it("rebases an unchanged Attribute object after an unrelated show revision", async () => {
		const latest = { ...snapshot, show_revision: 9 };
		const request = vi
			.fn()
			.mockResolvedValueOnce(snapshot)
			.mockRejectedValueOnce(new ApiRequestError("show revision conflict", 409))
			.mockResolvedValueOnce(latest)
			.mockResolvedValueOnce({
				request_id: "retry",
				replayed: false,
				snapshot: latest,
				event_sequence: 5,
			});
		const client = new AttributeConfigurationApiClient({
			request,
		} as unknown as ClientTransport);

		await client.snapshot("show-29");
		await expect(
			client.update("show-29", snapshot, { activation_groups: [] }),
		).resolves.toEqual({ snapshot: latest });
		const firstUpdate = JSON.parse(String(request.mock.calls[1]?.[1]?.body));
		const secondUpdate = JSON.parse(String(request.mock.calls[3]?.[1]?.body));
		expect(firstUpdate.expected_show_revision).toBe(8);
		expect(secondUpdate.expected_show_revision).toBe(9);
		expect(secondUpdate.expected_object_revision).toBe(3);
	});

	it("preserves the conflict when the Attribute object itself changed", async () => {
		const conflict = new ApiRequestError("object revision conflict", 409);
		const request = vi
			.fn()
			.mockRejectedValueOnce(conflict)
			.mockResolvedValueOnce({
				...snapshot,
				show_revision: 9,
				object_revision: 4,
			});
		const client = new AttributeConfigurationApiClient({
			request,
		} as unknown as ClientTransport);

		await expect(
			client.update("show-29", snapshot, { activation_groups: [] }),
		).rejects.toBe(conflict);
		expect(request).toHaveBeenCalledTimes(2);
	});
});
