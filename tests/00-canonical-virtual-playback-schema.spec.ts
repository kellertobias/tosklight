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

test("VPB-CANONICAL @api › canonical shows use the dedicated Virtual Playback schema", async ({
	api,
}) => {
	for (const [name, url] of CANONICAL_SHOWS) {
		const bytes = await fs.readFile(url);
		const imported = await api.createShow<{ id: string }>({
			name: `${name}-vpb-schema-${crypto.randomUUID()}`,
			data_base64: bytes.toString("base64"),
			overwrite: false,
		});
		await api.openShow(imported.id, { transition: "hold_current" });
		let pages = await api.showObjects<Record<string, unknown>>(
			imported.id,
			"playback_page",
		);
		if (UPDATE) {
			for (const page of pages) {
				if (!("virtual_playbacks" in page.body)) {
					await api.seedShowObject(
						imported.id,
						"playback_page",
						page.id,
						{ ...page.body, virtual_playbacks: {} },
						page.revision,
					);
				}
			}
			pages = await api.showObjects<Record<string, unknown>>(
				imported.id,
				"playback_page",
			);
		}
		const physical =
			await api.showObjects<{ number: number }>(imported.id, "playback");
		for (const page of pages) {
			expect(page.body).toHaveProperty("virtual_playbacks");
			expect(page.body.virtual_playbacks).toEqual(
				expect.objectContaining({}),
			);
			for (const number of Object.values(
				page.body.slots as Record<string, number>,
			)) {
				expect(number).toBeGreaterThanOrEqual(1);
				expect(number).toBeLessThanOrEqual(1_000);
			}
		}
		for (const playback of physical) {
			expect(playback.body.number).toBeGreaterThanOrEqual(1);
			expect(playback.body.number).toBeLessThanOrEqual(1_000);
		}
		if (UPDATE) await publishDownloadedShow(api, imported.id, url);
	}
});

async function publishDownloadedShow(
	api: ApiDriver,
	showId: string,
	url: URL,
): Promise<void> {
	const response = await fetch(`${api.baseUrl}/api/v2/shows/${showId}/download`, {
		headers: { authorization: `Bearer ${api.session?.token}` },
	});
	expect(response.ok).toBe(true);
	const target = fileURLToPath(url);
	const temporary = `${target}.${process.pid}.${crypto.randomUUID()}.tmp`;
	await fs.mkdir(path.dirname(target), { recursive: true });
	await fs.writeFile(temporary, Buffer.from(await response.arrayBuffer()));
	await fs.rename(temporary, target);
}
