import { describe, expect, it, vi } from "vitest";
import { FixtureApiClient } from "./fixtures";
import type { ClientTransport } from "./transport";

describe("FixtureApiClient gel catalogs", () => {
	it("searches, previews, and confirms through typed fixture-library routes", async () => {
		const request = vi.fn(async (path: string, init?: RequestInit) => {
			if (path.includes("?query=")) return { catalogs: [] };
			const body = JSON.parse(String(init?.body));
			if (path.endsWith("/preview"))
				return {
					catalog_id: body.target.catalog_id,
					catalog_name: body.catalog_name,
					catalog_name_changed: true,
					additions: [],
					replacements: [],
					unchanged: [],
					conflicts: [],
					invalid_rows: [],
					confirmable: true,
				};
			return {
				request_id: body.request_id,
				replayed: false,
				catalog: {
					id: body.target.catalog_id,
					revision: 1,
					name: body.catalog_name,
					entries: [],
				},
			};
		});
		const client = new FixtureApiClient({
			request,
		} as unknown as ClientTransport);
		const target = {
			type: "create" as const,
			catalog_id: "00000000-0000-4000-8000-000000000003",
		};
		const csv = new TextEncoder().encode(
			"number,name,display_rgb,visualizer_rgb\n1,Blue,#0000FF,#0000EE\n",
		);
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000103",
		);

		await client.gelCatalogs("primary blue");
		await client.previewGelCatalogCsvImport({
			target,
			catalogName: "Touring filters",
			csv,
		});
		const imported = await client.confirmGelCatalogCsvImport({
			target,
			catalogName: "Touring filters",
			csv,
		});

		expect(request.mock.calls[0]?.[0]).toBe(
			"/api/v2/fixture-library/gel-catalogs?query=primary+blue",
		);
		expect(request.mock.calls[1]?.[0]).toBe(
			"/api/v2/fixture-library/gel-catalogs/import/preview",
		);
		expect(JSON.parse(String(request.mock.calls[1]?.[1]?.body))).toMatchObject({
			target,
			catalog_name: "Touring filters",
		});
		expect(request.mock.calls[2]?.[0]).toBe(
			"/api/v2/fixture-library/gel-catalogs/00000000-0000-4000-8000-000000000003/update",
		);
		expect(JSON.parse(String(request.mock.calls[2]?.[1]?.body))).toMatchObject({
			request_id: "00000000-0000-4000-8000-000000000103",
			target,
			catalog_name: "Touring filters",
		});
		expect(imported).toMatchObject({ name: "Touring filters", revision: 1 });
	});
});
