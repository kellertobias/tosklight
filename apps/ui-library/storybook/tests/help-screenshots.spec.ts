import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });
test.setTimeout(180_000);

const ROOT = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../../../..",
);
const SCREENSHOT_ROOT = path.join(ROOT, "docs/help/assets/screenshots");
const MANIFEST_PATH = path.join(ROOT, "docs/help/screenshot-manifest.json");
const ACTUAL_ROOT = path.join(
	ROOT,
	".artifacts/test/help-screenshots/storybook",
);
const UPDATE = process.env.UPDATE_HELP_SCREENSHOTS === "1";

interface ScreenshotInteraction {
	action: "click" | "fill" | "press";
	selector: string;
	value?: string;
}

interface ScreenshotEntry {
	file: string;
	source: "storybook" | "live-app";
	storyId: string | null;
	viewport: { width: number; height: number };
	theme: "dark";
	mode: "software" | "hardware";
	interactions: ScreenshotInteraction[];
	reason?: string;
}

interface ScreenshotManifest {
	version: 1;
	entries: ScreenshotEntry[];
}

test("captures the complete help screenshot manifest from truthful sources", async ({
	page,
	request,
}) => {
	const manifest = JSON.parse(
		await fs.readFile(MANIFEST_PATH, "utf8"),
	) as ScreenshotManifest;
	expect(manifest.version).toBe(1);
	expect(manifest.entries.length).toBeGreaterThan(0);
	await fs.mkdir(ACTUAL_ROOT, { recursive: true });

	const tracked = await pngFiles(SCREENSHOT_ROOT);
	const declared = manifest.entries.map((entry) => entry.file).sort();
	expect(new Set(declared).size, "manifest contains duplicate filenames").toBe(
		declared.length,
	);
	expect(
		declared,
		"every tracked help PNG must have exactly one manifest entry",
	).toEqual(tracked);

	const indexResponse = await request.get("/index.json");
	expect(indexResponse.ok()).toBe(true);
	const index = (await indexResponse.json()) as {
		entries: Record<string, { type: string }>;
	};
	const storyIds = new Set(
		Object.entries(index.entries)
			.filter(([, entry]) => entry.type === "story")
			.map(([id]) => id),
	);
	const screenshotDiffs: string[] = [];

	await page.addInitScript(() => {
		const NativeDate = Date;
		const fixed = Date.parse("2026-07-26T10:00:00Z");
		class FixedDate extends NativeDate {
			constructor(...args: ConstructorParameters<typeof Date>) {
				super(...(args.length ? args : [fixed]));
			}
			static now() {
				return fixed;
			}
		}
		globalThis.Date = FixedDate as DateConstructor;
	});

	for (const entry of manifest.entries) {
		validateEntry(entry, storyIds);
		const expectedPath = path.join(SCREENSHOT_ROOT, entry.file);
		const expected = await fs.readFile(expectedPath);
		if (entry.source === "live-app") {
			expect(
				pngDimensions(expected),
				`${entry.file} dimensions drifted from its live-app manifest entry`,
			).toEqual(entry.viewport);
			await assertNotBlank(page, expected, entry.file);
			continue;
		}

		const consoleErrors: string[] = [];
		const pageErrors: string[] = [];
		const forbiddenRequests: string[] = [];
		const onConsole = (message: { type(): string; text(): string }) => {
			if (message.type() === "error") consoleErrors.push(message.text());
		};
		const onPageError = (error: Error) => pageErrors.push(error.message);
		const onRequest = (networkRequest: { url(): string }) => {
			const url = networkRequest.url();
			if (/^wss?:/u.test(url) || /\/api(?:\/|\?|$)/u.test(url))
				forbiddenRequests.push(url);
		};
		page.on("console", onConsole);
		page.on("pageerror", onPageError);
		page.on("request", onRequest);
		await page.setViewportSize(entry.viewport);
		const storyId = entry.storyId;
		if (storyId === null) {
			throw new Error(
				`Storybook screenshot ${entry.file} is missing a storyId`,
			);
		}
		await page.goto(
			`/iframe.html?id=${encodeURIComponent(storyId)}&viewMode=story&globals=mode:${entry.mode}`,
		);
		await page.evaluate(() => document.fonts.ready);
		await expect(page.locator("[data-documentation-shot]")).toHaveAttribute(
			"data-documentation-ready",
			"true",
		);
		await page.addStyleTag({
			content:
				"*,*::before,*::after{animation:none!important;caret-color:transparent!important;transition:none!important}",
		});
		for (const interaction of entry.interactions)
			await applyInteraction(page, interaction);

		const actual = await page.screenshot({
			animations: "disabled",
			fullPage: false,
		});
		expect(pngDimensions(actual), `${entry.file} capture dimensions`).toEqual(
			entry.viewport,
		);
		await assertNotBlank(page, actual, entry.file);
		expect(forbiddenRequests, `${entry.file} requested a live desk`).toEqual(
			[],
		);
		expect(consoleErrors, `${entry.file} logged console errors`).toEqual([]);
		expect(pageErrors, `${entry.file} raised page errors`).toEqual([]);

		const actualPath = path.join(ACTUAL_ROOT, entry.file);
		await fs.mkdir(path.dirname(actualPath), { recursive: true });
		await fs.writeFile(actualPath, actual);
		if (UPDATE) {
			await fs.writeFile(expectedPath, actual);
		} else {
			const difference = await pixelDifference(page, expected, actual);
			if (difference > 0.005)
				screenshotDiffs.push(
					`${entry.file}: ${(difference * 100).toFixed(3)}% changed; inspect ${path.relative(ROOT, actualPath)}`,
				);
		}
		page.off("console", onConsole);
		page.off("pageerror", onPageError);
		page.off("request", onRequest);
	}
	expect(
		screenshotDiffs,
		"unreviewed Storybook screenshot differences; rerun with UPDATE_HELP_SCREENSHOTS=1 only after inspecting every candidate",
	).toEqual([]);
});

function validateEntry(entry: ScreenshotEntry, storyIds: Set<string>) {
	expect(entry.file).toMatch(/^(?:panes\/|workflows\/)?[^/]+\.png$/u);
	expect(entry.viewport.width).toBeGreaterThanOrEqual(320);
	expect(entry.viewport.height).toBeGreaterThanOrEqual(160);
	expect(entry.theme).toBe("dark");
	expect(["software", "hardware"]).toContain(entry.mode);
	expect(Array.isArray(entry.interactions)).toBe(true);
	if (entry.source === "storybook") {
		if (entry.storyId === null) {
			throw new Error(`${entry.file} needs a Storybook story ID`);
		}
		expect(
			storyIds.has(entry.storyId),
			`${entry.file} references missing Storybook story ${entry.storyId}`,
		).toBe(true);
		expect(entry.reason).toBeUndefined();
	} else {
		expect(
			entry.storyId,
			`${entry.file} must not pretend to have a story`,
		).toBeNull();
		expect(
			entry.reason?.trim().length,
			`${entry.file} needs a live-app reason`,
		).toBeGreaterThan(10);
	}
}

async function applyInteraction(
	page: import("@playwright/test").Page,
	interaction: ScreenshotInteraction,
) {
	const target = page.locator(interaction.selector);
	await expect(target).toBeVisible();
	if (interaction.action === "click") await target.click();
	else if (interaction.action === "fill")
		await target.fill(interaction.value ?? "");
	else await target.press(interaction.value ?? "Enter");
}

async function pngFiles(directory: string, prefix = ""): Promise<string[]> {
	const entries = await fs.readdir(path.join(directory, prefix), {
		withFileTypes: true,
	});
	const files = await Promise.all(
		entries.map(async (entry) => {
			const relative = path.posix.join(prefix, entry.name);
			if (entry.isDirectory()) return pngFiles(directory, relative);
			return entry.isFile() && entry.name.endsWith(".png") ? [relative] : [];
		}),
	);
	return files.flat().sort();
}

function pngDimensions(buffer: Buffer) {
	expect(buffer.subarray(1, 4).toString("ascii")).toBe("PNG");
	return {
		width: buffer.readUInt32BE(16),
		height: buffer.readUInt32BE(20),
	};
}

async function assertNotBlank(
	page: import("@playwright/test").Page,
	buffer: Buffer,
	label: string,
) {
	const metrics = await imageMetrics(page, buffer);
	expect(
		metrics.opaqueRatio,
		`${label} is transparent or blank`,
	).toBeGreaterThan(0.95);
	expect(
		metrics.uniqueColors,
		`${label} has no meaningful rendered content`,
	).toBeGreaterThan(8);
	expect(
		metrics.luminanceRange,
		`${label} has no meaningful contrast`,
	).toBeGreaterThan(12);
}

async function pixelDifference(
	page: import("@playwright/test").Page,
	expected: Buffer,
	actual: Buffer,
) {
	return page.evaluate(
		async ({ expectedSource, actualSource }) => {
			const decode = async (source: string) => {
				const image = new Image();
				image.src = `data:image/png;base64,${source}`;
				await image.decode();
				const canvas = document.createElement("canvas");
				canvas.width = image.naturalWidth;
				canvas.height = image.naturalHeight;
				const context = canvas.getContext("2d", { willReadFrequently: true });
				if (!context) throw new Error("Unable to create comparison canvas");
				context.drawImage(image, 0, 0);
				return {
					width: canvas.width,
					height: canvas.height,
					pixels: context.getImageData(0, 0, canvas.width, canvas.height).data,
				};
			};
			const left = await decode(expectedSource);
			const right = await decode(actualSource);
			if (left.width !== right.width || left.height !== right.height) return 1;
			let changed = 0;
			for (let index = 0; index < left.pixels.length; index += 4) {
				if (
					Math.max(
						Math.abs(left.pixels[index] - right.pixels[index]),
						Math.abs(left.pixels[index + 1] - right.pixels[index + 1]),
						Math.abs(left.pixels[index + 2] - right.pixels[index + 2]),
						Math.abs(left.pixels[index + 3] - right.pixels[index + 3]),
					) > 16
				)
					changed += 1;
			}
			return changed / (left.width * left.height);
		},
		{
			expectedSource: expected.toString("base64"),
			actualSource: actual.toString("base64"),
		},
	);
}

async function imageMetrics(
	page: import("@playwright/test").Page,
	buffer: Buffer,
) {
	return page.evaluate(async (source) => {
		const image = new Image();
		image.src = `data:image/png;base64,${source}`;
		await image.decode();
		const canvas = document.createElement("canvas");
		canvas.width = image.naturalWidth;
		canvas.height = image.naturalHeight;
		const context = canvas.getContext("2d", { willReadFrequently: true });
		if (!context)
			throw new Error("Unable to create screenshot comparison canvas");
		context.drawImage(image, 0, 0);
		const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
		const colors = new Set<number>();
		let opaque = 0;
		let minimum = 255;
		let maximum = 0;
		const stride = Math.max(
			1,
			Math.floor((canvas.width * canvas.height) / 100_000),
		);
		for (let pixel = 0; pixel < canvas.width * canvas.height; pixel += stride) {
			const index = pixel * 4;
			const red = pixels[index];
			const green = pixels[index + 1];
			const blue = pixels[index + 2];
			const alpha = pixels[index + 3];
			if (alpha > 240) opaque += 1;
			const luminance = Math.round(
				red * 0.2126 + green * 0.7152 + blue * 0.0722,
			);
			minimum = Math.min(minimum, luminance);
			maximum = Math.max(maximum, luminance);
			colors.add(((red >> 4) << 8) | ((green >> 4) << 4) | (blue >> 4));
		}
		const samples = Math.ceil((canvas.width * canvas.height) / stride);
		return {
			opaqueRatio: opaque / samples,
			uniqueColors: colors.size,
			luminanceRange: maximum - minimum,
		};
	}, buffer.toString("base64"));
}
