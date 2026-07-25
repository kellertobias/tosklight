import fs from "node:fs/promises";
import path from "node:path";
import { expect, test } from "./bench/core/fixtures";
import { ControllableHostedFilePickerDriver } from "./bench/window-system/hostedFilePicker";
import type {
	Locator,
	Page,
} from "@playwright/test";

test.describe("docs/testing/09-file-manager-and-text-editor.md", () => {
	test("FILE-016 @api @failure-mode › confined file services authenticate, stream ranges, expose native capabilities, and resolve conflicts", async ({
		api,
		bench,
		request,
	}) => {
		const unauthenticated = await request.get(
			`${bench.baseUrl}/api/v2/files/roots`,
		);
		expect(unauthenticated.status()).toBe(401);
		const authorization = { authorization: `Bearer ${api.session!.token}` };
		const rootsResponse = await request.get(
			`${bench.baseUrl}/api/v2/files/roots`,
			{ headers: authorization },
		);
		expect(rootsResponse.status()).toBe(200);
		const roots = (await rootsResponse.json()) as Array<Record<string, any>>;
		expect(roots).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					id: "shows",
					label: "Shows",
					removable: false,
				}),
			]),
		);
		expect(roots[0]).not.toHaveProperty("path");
		expect(roots[0].capabilities).toEqual(
			expect.objectContaining({
				range_streaming: true,
				thumbnails: true,
				native_notes: expect.any(Boolean),
				trash: expect.any(Boolean),
			}),
		);

		const workspace = `file-contract-${crypto.randomUUID()}`;
		try {
			await api.request("POST", "/api/v2/files/shows/operations", {
				operation: "create_folder",
				sources: [],
				destination: "",
				name: workspace,
			});
			await api.request("POST", "/api/v2/files/shows/operations", {
				operation: "create_folder",
				sources: [],
				destination: workspace,
				name: "Destination",
			});
			await api.request("POST", "/api/v2/files/shows/operations", {
				operation: "create_file",
				sources: [],
				destination: workspace,
				name: "range.txt",
			});
			await api.request("POST", "/api/v2/files/shows/operations", {
				operation: "create_file",
				sources: [],
				destination: workspace,
				name: ".hidden",
			});
			const document = await api.request<any>(
				"GET",
				`/api/v2/files/shows/text?path=${encodeURIComponent(`${workspace}/range.txt`)}`,
			);
			await api.request("PUT", "/api/v2/files/shows/text", {
				path: `${workspace}/range.txt`,
				text: "0123456789",
				revision: document.revision,
			});

			const ordinary = await api.request<any>(
				"GET",
				`/api/v2/files/shows/entries?path=${encodeURIComponent(workspace)}`,
			);
			expect(ordinary.entries.map((entry: any) => entry.name)).not.toContain(
				".hidden",
			);
			const withHidden = await api.request<any>(
				"GET",
				`/api/v2/files/shows/entries?path=${encodeURIComponent(workspace)}&hidden=true`,
			);
			expect(withHidden.entries.map((entry: any) => entry.name)).toContain(
				".hidden",
			);

			const traversal = await request.get(
				`${bench.baseUrl}/api/v2/files/shows/entries?path=${encodeURIComponent("../")}`,
				{ headers: authorization },
			);
			expect(traversal.status()).toBe(400);
			const range = await request.get(
				`${bench.baseUrl}/api/v2/files/shows/content?path=${encodeURIComponent(`${workspace}/range.txt`)}`,
				{ headers: { ...authorization, range: "bytes=2-5" } },
			);
			expect(range.status()).toBe(206);
			expect(range.headers()["accept-ranges"]).toBe("bytes");
			expect(range.headers()["content-range"]).toBe("bytes 2-5/10");
			expect(await range.text()).toBe("2345");
			const suffix = await request.get(
				`${bench.baseUrl}/api/v2/files/shows/content?path=${encodeURIComponent(`${workspace}/range.txt`)}`,
				{ headers: { ...authorization, range: "bytes=-3" } },
			);
			expect(await suffix.text()).toBe("789");

			const metadata = await api.request<any>(
				"GET",
				`/api/v2/files/shows/metadata?path=${encodeURIComponent(`${workspace}/range.txt`)}`,
			);
			expect(metadata).toEqual(
				expect.objectContaining({
					name: "range.txt",
					mime: "text/plain; charset=utf-8",
				}),
			);
			expect(
				metadata.created_millis === null ||
					typeof metadata.created_millis === "number",
			).toBe(true);
			if (metadata.note_supported) {
				await api.request("PUT", "/api/v2/files/shows/notes", {
					path: `${workspace}/range.txt`,
					note: "Operator metadata",
				});
				const note = await api.request<any>(
					"GET",
					`/api/v2/files/shows/notes?path=${encodeURIComponent(`${workspace}/range.txt`)}`,
				);
				expect(note).toEqual(
					expect.objectContaining({
						supported: true,
						note: "Operator metadata",
					}),
				);
				const names = (
					await api.request<any>(
						"GET",
						`/api/v2/files/shows/entries?path=${encodeURIComponent(workspace)}&hidden=true`,
					)
				).entries.map((entry: any) => entry.name as string);
				expect(
					names.some((name: string) => /tosklight.*note|\.note/i.test(name)),
				).toBe(false);
			}

			await api.request("POST", "/api/v2/files/shows/operations", {
				operation: "copy",
				sources: [`${workspace}/range.txt`],
				destination: `${workspace}/Destination`,
			});
			const conflict = await request.post(
				`${bench.baseUrl}/api/v2/files/shows/operations`,
				{
					headers: { ...authorization, "content-type": "application/json" },
					data: {
						request_id: crypto.randomUUID(),
						operation: "copy",
						sources: [`${workspace}/range.txt`],
						destination: `${workspace}/Destination`,
					},
				},
			);
			expect(conflict.status()).toBe(409);
			const keepBoth = await api.request<any>(
				"POST",
				"/api/v2/files/shows/operations",
				{
					operation: "copy",
					sources: [`${workspace}/range.txt`],
					destination: `${workspace}/Destination`,
					conflict: "keep_both",
					apply_to_all: true,
				},
			);
			expect(keepBoth).toEqual(
				expect.objectContaining({
					complete: true,
					paths: expect.arrayContaining([
						`${workspace}/Destination/range copy.txt`,
					]),
				}),
			);

			await api.setCommandLineText("COPY");
			const claimed = await api.request<any>(
				"POST",
				"/api/v2/files/input-context",
				{
					instance_id: "acceptance-file-manager",
					action: "copy",
					origin: "pending",
				},
			);
			expect(claimed).toEqual(
				expect.objectContaining({
					instance_id: "acceptance-file-manager",
					action: "copy",
					session_id: api.session!.session_id,
					desk_id: api.session!.desk.id,
				}),
			);
			const programmers = await api.request<any[]>(
				"GET",
				"/api/v2/programmers",
			);
			expect(
				programmers.find(
					(programmer) => programmer.session_id === api.session!.session_id,
				)?.command_line,
			).toBe("");
			const competingClaim = await request.post(
				`${bench.baseUrl}/api/v2/files/input-context/claim`,
				{
					headers: { ...authorization, "content-type": "application/json" },
					data: {
						request_id: crypto.randomUUID(),
						instance_id: "another-pane",
						action: "copy",
						origin: "toolbar",
					},
				},
			);
			expect(competingClaim.status()).toBe(409);
			const hardware = await bench.osc();
			try {
				const alias = api.session!.desk.osc_alias;
				await hardware.subscribe(`file-manager-${crypto.randomUUID()}`, alias);
				await hardware.send(`/light/${alias}/programmer/enter`, [true]);
				await expect
					.poll(() => api.request<any>("GET", "/api/v2/files/input-context"))
					.toEqual(
						expect.objectContaining({
							instance_id: "acceptance-file-manager",
							action: "copy",
						}),
					);
				expect(
					(await api.request<any[]>("GET", "/api/v2/programmers")).find(
						(programmer) => programmer.session_id === api.session!.session_id,
					)?.command_line,
				).toBe("");
				await hardware.send(`/light/${alias}/programmer/escape`, [true]);
				await expect
					.poll(() => api.request("GET", "/api/v2/files/input-context"))
					.toBeNull();
			} finally {
				await hardware.close();
			}
		} finally {
			await api
				.request("POST", "/api/v2/files/shows/operations", {
					operation: "delete",
					sources: [workspace],
				})
				.catch(() => undefined);
		}
	});
});
