import { describe, expect, it, vi } from "vitest";
import { FixtureApiClient } from "./fixtures";
import type { ClientTransport } from "./transport";

describe("canonical fixture GDTF transport", () => {
	it("sends source bytes and mappings as one revision-guarded import intent", async () => {
		const profile = {
			id: "00000000-0000-4000-8000-000000000055",
			revision: 1,
			modes: [],
		};
		const request = vi.fn(async (path: string, init?: RequestInit) => {
			if (path.endsWith("/preview"))
				return {
					profile: { ...profile, revision: 0 },
					diagnostics: [
						{ node: "Emitter", message: "Unknown optical evidence" },
					],
					unknown_attributes: [],
				};
			if (path.endsWith("/update"))
				return {
					request_id: JSON.parse(String(init?.body)).request_id,
					replayed: false,
					result: { type: "profile", profile_id: profile.id, revision: 1 },
				};
			if (path === `/api/v2/fixture-library/profiles/${profile.id}/revisions`)
				return { profiles: [{ ...profile, revision: 2 }, profile] };
			throw new Error(`Unexpected request: ${path}`);
		});
		const client = new FixtureApiClient({
			request,
		} as unknown as ClientTransport);
		const source = new Uint8Array([80, 75, 3, 4, 0, 255]);
		const preview = await client.previewFixtureGdtf(source);
		expect(preview.diagnostics).toHaveLength(1);
		const saved = await client.importFixtureGdtf({
			profileId: profile.id,
			expectedRevision: 0,
			source,
			attributeMappings: [
				{ source_attribute: "gdtf.Wheel", target_attribute: "gobo.1" },
			],
		});
		expect(saved).toEqual(profile);
		expect(request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/fixture-library/gdtf/preview",
			`/api/v2/fixture-library/profiles/${profile.id}/update`,
			`/api/v2/fixture-library/profiles/${profile.id}/revisions`,
		]);
		const body = JSON.parse(String(request.mock.calls[1]?.[1]?.body));
		expect(body).toMatchObject({
			expected_revision: 0,
			source_base64: "UEsDBAD/",
			attribute_mappings: [
				{ source_attribute: "gdtf.Wheel", target_attribute: "gobo.1" },
			],
		});
		expect(body.request_id).toEqual(expect.any(String));
		expect(body.profile).toBeUndefined();
	});

	it.each([
		{ name: "a newer revision only", id: "saved-profile", revision: 8 },
		{
			name: "another profile at the exact revision",
			id: "foreign-profile",
			revision: 7,
		},
	])("rejects $name instead of falling back after import", async (candidate) => {
		const request = vi.fn(async (path: string) => {
			if (path === "/api/v2/fixture-library/profiles/input-profile/update")
				return {
					result: { type: "profile", profile_id: "saved-profile", revision: 7 },
				};
			if (path === "/api/v2/fixture-library/profiles/saved-profile/revisions")
				return { profiles: [{ ...candidate, modes: [] }] };
			throw new Error(`Unexpected request: ${path}`);
		});
		const client = new FixtureApiClient({
			request,
		} as unknown as ClientTransport);
		await expect(
			client.importFixtureGdtf({
				profileId: "input-profile",
				expectedRevision: 0,
				source: new Uint8Array([80, 75]),
				attributeMappings: [],
			}),
		).rejects.toThrow("Saved fixture profile is missing from the snapshot");
		expect(request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/fixture-library/profiles/input-profile/update",
			"/api/v2/fixture-library/profiles/saved-profile/revisions",
		]);
	});

	it("propagates scoped authority failure without a whole-library retry", async () => {
		const failure = new Error("Profile revisions unavailable");
		const request = vi.fn(async (path: string) => {
			if (path.endsWith("/update"))
				return {
					result: { type: "profile", profile_id: "saved-profile", revision: 7 },
				};
			throw failure;
		});
		const client = new FixtureApiClient({
			request,
		} as unknown as ClientTransport);
		await expect(
			client.importFixtureGdtf({
				profileId: "input-profile",
				expectedRevision: 0,
				source: new Uint8Array([80, 75]),
				attributeMappings: [],
			}),
		).rejects.toBe(failure);
		expect(request.mock.calls.map(([path]) => path)).toEqual([
			"/api/v2/fixture-library/profiles/input-profile/update",
			"/api/v2/fixture-library/profiles/saved-profile/revisions",
		]);
	});
});
