import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "./bench/core/fixtures";
import type { ApiDriver } from "./bench/core/api";

const UPDATE = process.env.LIGHT_UPDATE_CANONICAL_SHOWS === "1";
const CANONICAL_SHOWS = [
	["demo", new URL("../assets/demo.show", import.meta.url)],
	["compact-rig", new URL("./fixtures/compact-rig.show", import.meta.url)],
	["default-stage", new URL("./fixtures/default-stage.show", import.meta.url)],
] as const;

test("PLAYBACK-CANONICAL @api › owned shows contain unified sparse assignments without retired objects", async ({
	api,
}) => {
	for (const [name, url] of CANONICAL_SHOWS) {
		const bytes = await fs.readFile(url);
		const imported = await api.createShow<{ id: string }>({
			name: `${name}-unified-schema-${crypto.randomUUID()}`,
			data_base64: bytes.toString("base64"),
			overwrite: false,
		});
		await api.openShow(imported.id, { transition: "hold_current" });
		expect(await api.showObjects(imported.id, "playback")).toHaveLength(0);
		expect(
			await api.showObjects(imported.id, "virtual_playback_exclusion_zones"),
		).toHaveLength(0);
		const pages = await api.showObjects<Record<string, unknown>>(
			imported.id,
			"playback_page",
		);
		for (const page of pages) {
			expect(page.body).not.toHaveProperty("slots");
			expect(page.body).not.toHaveProperty("virtual_playbacks");
			expect(page.body.number).toBeGreaterThanOrEqual(1);
			expect(page.body.number).toBeLessThanOrEqual(99);
			for (const [position, definition] of Object.entries(
				page.body.assignments as Record<string, object>,
			)) {
				expect(Number(position)).toBeGreaterThanOrEqual(1);
				expect(Number(position)).toBeLessThanOrEqual(1000);
				expect(definition).not.toHaveProperty("number");
			}
		}
		if (UPDATE) await publishDownloadedShow(api, imported.id, url);
	}
});

async function publishDownloadedShow(
	api: ApiDriver,
	showId: string,
	url: URL,
): Promise<void> {
	const response = await fetch(
		`${api.baseUrl}/api/v2/shows/${showId}/download`,
		{
			headers: { authorization: `Bearer ${api.session?.token}` },
		},
	);
	expect(response.ok).toBe(true);
	const target = fileURLToPath(url);
	const temporary = `${target}.${process.pid}.${crypto.randomUUID()}.tmp`;
	await fs.mkdir(path.dirname(target), { recursive: true });
	await fs.writeFile(temporary, Buffer.from(await response.arrayBuffer()));
	await fs.rename(temporary, target);
}
