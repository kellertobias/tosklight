import { describe, expect, it, vi } from "vitest";
import { FixtureApiClient } from "./fixtures";
import type { ClientTransport } from "./transport";

describe("FixtureApiClient source mappings", () => {
	it("lists and replay-safely remembers stable source identities", async () => {
		const mapping = {
			source_format: "gdtf",
			source_attribute: "Gobo",
			target_attribute: "gobo.1",
		};
		const request = vi.fn(async (_path: string, init?: RequestInit) => {
			if (!init) return { mappings: [mapping] };
			const body = JSON.parse(String(init.body));
			return {
				request_id: body.request_id,
				replayed: false,
				result: { type: "source_mapping", mapping },
			};
		});
		const client = new FixtureApiClient({
			request,
		} as unknown as ClientTransport);
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000165",
		);

		expect(await client.fixtureSourceMappings()).toEqual([mapping]);
		expect(
			await client.rememberFixtureSourceMapping({
				sourceFormat: "gdtf",
				sourceAttribute: "Gobo",
				targetAttribute: "gobo.1",
			}),
		).toEqual(mapping);
		expect(request.mock.calls[0]?.[0]).toBe(
			"/api/v2/fixture-library/source-mappings",
		);
		expect(JSON.parse(String(request.mock.calls[1]?.[1]?.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000165",
			action: {
				type: "remember_source_mapping",
				source_format: "gdtf",
				source_attribute: "Gobo",
				target_attribute: "gobo.1",
			},
		});
	});
});
