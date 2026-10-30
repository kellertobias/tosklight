import type { Page } from "@playwright/test";
import type { TimecodeDefinition } from "../apps/light-desktop/src/api/types/timecode";
import { expect, test } from "./bench/core/fixtures";

const RATE = 16_000;
const BEAT = RATE / 2; // 120 BPM

/** A 64-second song: kick on every beat; quiet D major, loud G major, quiet C major, loud G major. */
function songWav() {
	const sections: Array<[number[], number, boolean]> = [
		[[293.66, 369.99, 440], 0.08, false],
		[[196, 246.94, 293.66], 0.45, true],
		[[261.63, 329.63, 392], 0.15, false],
		[[196, 246.94, 293.66], 0.45, true],
	];
	const frames = sections.length * 8 * 4 * BEAT;
	const buffer = Buffer.alloc(44 + frames * 2);
	buffer.write("RIFF", 0);
	buffer.writeUInt32LE(buffer.length - 8, 4);
	buffer.write("WAVEfmt ", 8);
	buffer.writeUInt32LE(16, 16);
	buffer.writeUInt16LE(1, 20);
	buffer.writeUInt16LE(1, 22);
	buffer.writeUInt32LE(RATE, 24);
	buffer.writeUInt32LE(RATE * 2, 28);
	buffer.writeUInt16LE(2, 32);
	buffer.writeUInt16LE(16, 34);
	buffer.write("data", 36);
	buffer.writeUInt32LE(frames * 2, 40);
	let seed = 12345;
	for (let index = 0; index < frames; index++) {
		const [chord, level, hats] = sections[Math.floor(index / (8 * 4 * BEAT))];
		const t = index / RATE;
		const inBeat = index % BEAT;
		const kickT = inBeat / RATE;
		const kick = Math.sin(2 * Math.PI * 55 * kickT) * Math.exp(-kickT * 18) * 0.6;
		const pad = chord.reduce((sum, frequency) => sum + Math.sin(2 * Math.PI * frequency * t), 0) / chord.length;
		seed = (seed * 1103515245 + 12345) & 0x7fffffff;
		const hat = hats && inBeat % (BEAT / 2) < 130 ? ((seed % 2000) / 1000 - 1) * 0.12 : 0;
		const sample = Math.max(-1, Math.min(1, kick + pad * level + hat));
		buffer.writeInt16LE(Math.round(sample * 30_000), 44 + index * 2);
	}
	return buffer;
}

async function openTimecode(page: Page, name: string) {
	await expect(page.locator(".connection-cover")).toBeHidden();
	const pool = page.getByRole("button", { name, exact: true });
	if (!(await pool.isVisible().catch(() => false))) {
		const toggle = page.getByRole("button", { name: "Desktops / Built-ins", exact: true });
		if ((await toggle.getAttribute("data-dock-mode")) !== "builtins") await toggle.click();
		const shift = page.locator('[data-keypad-key="SHIFT"]:visible').first();
		if (!(await shift.isVisible().catch(() => false))) await page.locator(".mode-toggle").click();
		await shift.click();
		await page.locator("[aria-label='Shift Built-ins']").getByRole("button", { name: "Timecode", exact: true }).click();
	}
	await pool.click({ button: "right" });
	await expect(page.getByLabel("Timecode timeline editor", { exact: true })).toBeVisible();
}

test("TL-856 @ui › song analysis shows progress, a beat grid and suggested sections that become markers, and is cached", async ({ api, desk, page, request, show }) => {
	test.setTimeout(150_000);
	const showId = show.id;
	const imported = await request.post(`${api.baseUrl}/api/v2/timecodes/audio/import?name=Song.wav`, {
		headers: { authorization: `Bearer ${api.session!.token}`, "x-tosk-show": showId, "content-type": "audio/wav" },
		data: songWav(),
	});
	expect(imported.ok()).toBe(true);
	const asset = (await imported.json()) as { asset_id: string; asset_revision: number };
	const definition: TimecodeDefinition = {
		id: crypto.randomUUID(),
		number: 856,
		name: "Song analysis",
		duration_frame: 64 * 44,
		transport_offset_frame: 0,
		auto_start: false,
		audio: { asset_id: asset.asset_id, asset_revision: asset.asset_revision, file_name: "Song.wav" },
		markers: [],
		lanes: [],
	};
	await api.request("POST", "/api/v2/timecodes/actions", { request_id: crypto.randomUUID(), action: { type: "create", definition } }, true, undefined, { showId });

	await page.setViewportSize({ width: 1600, height: 1000 });
	await desk.open(api.baseUrl);
	await openTimecode(page, "Timecode 856 Song analysis");
	const panel = page.getByRole("region", { name: "Song analysis" });
	// Nothing takes room above the timeline until the operator asks.
	await expect(panel).toHaveCount(0);
	await page.getByRole("button", { name: "Analyse song", exact: true }).click();
	// Progress is visible while the desk analyses.
	await expect(panel.getByRole("status")).toContainText(/Analysing audio|BPM/);
	await expect(panel.getByRole("status")).toContainText("BPM", { timeout: 60_000 });
	await expect(panel.getByRole("status")).toContainText(/1(19|20|21)\.\d BPM/);
	await expect(page.locator(".timecode-bar-line").first()).toBeAttached();
	const suggestions = panel.getByRole("list", { name: "Suggested sections" }).getByRole("listitem");
	const count = await suggestions.count();
	expect(count).toBeGreaterThanOrEqual(3);
	await expect(page.locator(".timecode-suggested-section")).toHaveCount(count);

	// Rename and accept the first, discard the second, accept the rest.
	const first = suggestions.first();
	await first.getByRole("textbox").fill("Opening");
	await first.getByRole("button", { name: /^Accept / }).click();
	await expect(suggestions).toHaveCount(count - 1);
	await suggestions.first().getByRole("button", { name: /^Discard / }).click();
	await expect(suggestions).toHaveCount(count - 2);
	await panel.getByRole("button", { name: "Accept All", exact: true }).click();
	await expect(suggestions).toHaveCount(0);
	await expect(page.locator(".timecode-suggested-section")).toHaveCount(0);
	await expect
		.poll(async () => {
			const stored = await api.request<{ objects: Array<{ definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId });
			return stored.objects.find((item) => item.definition.id === definition.id)?.definition.markers.map((marker) => marker.name) ?? [];
		}, { timeout: 10_000 })
		.toHaveLength(count - 1);
	const markers = (await api.request<{ objects: Array<{ definition: TimecodeDefinition }> }>("GET", "/api/v2/timecodes", undefined, true, undefined, { showId })).objects.find((item) => item.definition.id === definition.id)!.definition.markers;
	expect(markers[0].name).toBe("Opening");
	expect(markers.some((marker) => marker.name === "Chorus")).toBe(true);

	// Unchanged audio is never analysed twice: a fresh read is Ready at once.
	const cached = await api.request<{ state: string; analysis: { digest: string; bpm: number } }>("GET", `/api/v2/timecodes/${definition.id}/audio/analysis`, undefined, true, undefined, { showId });
	expect(cached.state).toBe("ready");
	await page.reload();
	await openTimecode(page, "Timecode 856 Song analysis");
	await expect(page.locator(".timecode-bar-line").first()).toBeAttached();
	// Accepted sections stay markers; a discarded one was never stored, so it is offered again.
	await page.getByRole("button", { name: "Show song analysis", exact: true }).click();
	await expect(page.getByRole("region", { name: "Song analysis" }).getByRole("status")).toContainText("BPM · 1 suggested section");
});
