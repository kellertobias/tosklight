import { expect, test } from "./bench/core/fixtures";

test.describe("docs/testing/09-file-manager-and-text-editor.md", () => {
	test("FILE-001 @api › default root is confined and supports revision-safe UTF-8 text", async ({
		api,
	}) => {
		const roots = await api.request<any[]>("GET", "/api/v2/files/roots");
		expect(
			roots.some((root) => root.id === "shows" && root.label === "Shows"),
		).toBe(true);
		const name = `operator-notes-${crypto.randomUUID()}.txt`;
		await api.request("POST", "/api/v2/files/shows/operations", {
			operation: "create_file",
			sources: [],
			destination: "",
			name,
		});
		const created = await api.request<any>(
			"GET",
			`/api/v2/files/shows/text?path=${encodeURIComponent(name)}`,
		);
		const saved = await api.request<any>("PUT", "/api/v2/files/shows/text", {
			path: name,
			text: "Preset check\nStandby cue 12\n",
			revision: created.revision,
		});
		expect(saved.text).toBe("Preset check\nStandby cue 12\n");
		await expect(
			api.request("PUT", "/api/v2/files/shows/text", {
				path: name,
				text: "stale",
				revision: created.revision,
			}),
		).rejects.toThrow(/409.*changed since it was opened/);
		await expect(
			api.request("GET", "/api/v2/files/shows/entries?path=..%2F"),
		).rejects.toThrow(/400.*may not traverse parents/);
		await api.request("POST", "/api/v2/files/shows/operations", {
			operation: "delete",
			sources: [name],
		});
	});
});
