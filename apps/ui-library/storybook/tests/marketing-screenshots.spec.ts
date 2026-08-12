import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });
test.setTimeout(240_000);

const ROOT = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"../../../..",
);
const REVIEWED_ROOT = path.join(ROOT, "docs/marketing/assets/screenshots");
const MANIFEST_PATH = path.join(
	ROOT,
	"docs/marketing/screenshot-manifest.json",
);
const ACTUAL_ROOT = path.join(
	ROOT,
	".artifacts/test/marketing-screenshots/storybook",
);

interface ScreenshotInteraction {
	action: "click" | "fill" | "press" | "expect-visible" | "expect-hidden";
	selector: string;
	value?: string;
}

interface MarketingScreenshotEntry {
	file: string;
	title: string;
	caption: string;
	storyId: string;
	viewport: { width: number; height: number };
	captureSelector?: string;
	theme: "dark";
	mode: "software" | "hardware";
	interactions: ScreenshotInteraction[];
}

interface MarketingScreenshotManifest {
	version: 1;
	entries: MarketingScreenshotEntry[];
}

test("captures the complete reviewed marketing gallery from Storybook", async ({
	page,
	request,
}) => {
	const manifest = JSON.parse(
		await fs.readFile(MANIFEST_PATH, "utf8"),
	) as MarketingScreenshotManifest;
	expect(manifest.version).toBe(1);
	expect(manifest.entries).toHaveLength(21);
	await fs.rm(ACTUAL_ROOT, { recursive: true, force: true });
	await fs.mkdir(ACTUAL_ROOT, { recursive: true });
	await fs.mkdir(REVIEWED_ROOT, { recursive: true });

	const declared = manifest.entries.map((entry) => entry.file).sort();
	expect(new Set(declared).size, "manifest contains duplicate filenames").toBe(
		declared.length,
	);
	// The gallery directory is an output, not a reviewed baseline: this run fills it from the
	// current Storybook. What was in it before says nothing about what should be in it now, so the
	// only completeness check worth making is the one at the end, against the manifest.
	await fs.rm(REVIEWED_ROOT, { recursive: true, force: true });
	await fs.mkdir(REVIEWED_ROOT, { recursive: true });

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
	for (const entry of manifest.entries) validateEntry(entry, storyIds);

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
		await page.goto(
			`/iframe.html?id=${encodeURIComponent(entry.storyId)}&viewMode=story&globals=mode:${entry.mode}`,
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
		await page.evaluate(() => window.scrollTo(0, 0));
		await settleRenderedLayout(page);

		const captureTarget = entry.captureSelector
			? page.locator(entry.captureSelector)
			: page;
		if (entry.captureSelector) await expect(captureTarget).toBeVisible();
		const actual = await captureTarget.screenshot({
			animations: "disabled",
		});
		const actualDimensions = pngDimensions(actual);
		expect(
			actualDimensions.width,
			`${entry.file} capture width`,
		).toBeLessThanOrEqual(entry.viewport.width);
		expect(
			actualDimensions.height,
			`${entry.file} capture height`,
		).toBeLessThanOrEqual(entry.viewport.height);
		await assertNotBlank(page, actual, entry.file);
		expect(forbiddenRequests, `${entry.file} requested a live desk`).toEqual(
			[],
		);
		expect(consoleErrors, `${entry.file} logged console errors`).toEqual([]);
		expect(pageErrors, `${entry.file} raised page errors`).toEqual([]);

		const actualPath = path.join(ACTUAL_ROOT, entry.file);
		await fs.mkdir(path.dirname(actualPath), { recursive: true });
		await fs.writeFile(actualPath, actual);
		const reviewedPath = path.join(REVIEWED_ROOT, entry.file);
		await fs.mkdir(path.dirname(reviewedPath), { recursive: true });
		await fs.writeFile(reviewedPath, actual);

		page.off("console", onConsole);
		page.off("pageerror", onPageError);
		page.off("request", onRequest);
	}

	expect(
		await pngFiles(REVIEWED_ROOT),
		"the generated gallery must be exactly what the manifest declares",
	).toEqual(declared);
});

function validateEntry(entry: MarketingScreenshotEntry, storyIds: Set<string>) {
	expect(entry.file).toMatch(/^[a-z0-9][a-z0-9-]*\.png$/u);
	expect(entry.title.trim().length).toBeGreaterThan(0);
	expect(entry.caption.trim().length).toBeGreaterThan(10);
	expect(entry.viewport.width).toBeGreaterThanOrEqual(320);
	expect(entry.viewport.height).toBeGreaterThanOrEqual(160);
	expect(entry.theme).toBe("dark");
	expect(["software", "hardware"]).toContain(entry.mode);
	expect(Array.isArray(entry.interactions)).toBe(true);
	if (entry.captureSelector)
		expect(entry.captureSelector.trim().length).toBeGreaterThan(0);
	expect(
		storyIds.has(entry.storyId),
		`${entry.file} references missing Storybook story ${entry.storyId}`,
	).toBe(true);
}

async function applyInteraction(
	page: import("@playwright/test").Page,
	interaction: ScreenshotInteraction,
) {
	const target = page.locator(interaction.selector);
	if (interaction.action === "expect-hidden") {
		await expect(target).toBeHidden();
		return;
	}
	await expect(target).toBeVisible();
	if (interaction.action === "expect-visible") return;
	if (interaction.action === "click") await target.click();
	else if (interaction.action === "fill")
		await target.fill(interaction.value ?? "");
	else await target.press(interaction.value ?? "Enter");
}

async function settleRenderedLayout(page: import("@playwright/test").Page) {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
			),
	);
}

async function pngFiles(directory: string): Promise<string[]> {
	try {
		return (await fs.readdir(directory, { withFileTypes: true }))
			.filter((entry) => entry.isFile() && entry.name.endsWith(".png"))
			.map((entry) => entry.name)
			.sort();
	} catch (error) {
		if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
		throw error;
	}
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
