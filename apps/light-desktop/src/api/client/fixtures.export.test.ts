import { describe, expect, it, vi } from "vitest";
import { FixtureApiClient } from "./fixtures";
import type { ClientTransport } from "./transport";

describe("immutable fixture package export transport", () => {
	it("uses an explicit revision operand and replay identity without changing GET download", async () => {
		const blob = new Blob([new Uint8Array([80, 75, 3, 4])]);
		const download = vi.fn().mockResolvedValue(blob);
		const request = vi.fn(async (_path: string, init?: RequestInit) => {
			const body = JSON.parse(String(init?.body));
			return {
				request_id: body.request_id,
				replayed: false,
				profile_id: "profile",
				revision: 7,
				root_id: body.root_id,
				path: "exports/exact.toskfixture",
			};
		});
		const client = new FixtureApiClient({
			request,
			blob: download,
		} as unknown as ClientTransport);
		const result = await client.exportFixturePackageFile("profile", 7, {
			root_id: "shows",
			path: "exports",
			name: "exact",
		});
		expect(request).toHaveBeenCalledOnce();
		const [path, init] = request.mock.calls[0];
		expect(path).toBe(
			"/api/v2/fixture-library/profiles/profile/revisions/7/export",
		);
		expect(init?.method).toBe("POST");
		expect(JSON.parse(String(init?.body))).toEqual({
			root_id: "shows",
			path: "exports",
			name: "exact",
			request_id: expect.any(String),
		});
		expect(result.path).toBe("exports/exact.toskfixture");
		expect(await client.exportFixturePackage("profile", 7)).toBe(blob);
		expect(download).toHaveBeenCalledWith(
			"/api/v2/fixture-library/profiles/profile/revisions/7/package",
		);
	});
	it("does not report a saved result for a different revision or destination", async () => {
		const request = vi.fn(async (_path: string, init?: RequestInit) => ({
			request_id: JSON.parse(String(init?.body)).request_id,
			replayed: false,
			profile_id: "profile",
			revision: 8,
			root_id: "other",
			path: "exact.toskfixture",
		}));
		const client = new FixtureApiClient({
			request,
		} as unknown as ClientTransport);
		await expect(
			client.exportFixturePackageFile("profile", 7, {
				root_id: "shows",
				path: "",
				name: "exact",
			}),
		).rejects.toThrow("invalid fixture export result");
		expect(request).toHaveBeenCalledOnce();
	});
});
