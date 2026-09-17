import type { Locator, Page } from "@playwright/test";
import type {
	DiscoveredMediaOutput,
	MediaServerDiscovery,
} from "../apps/light-desktop/src/api/generated/light-wire";
import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

const DISCOVER = "/api/v2/media-servers/discover";

/** Enough discovered outputs that Media Servers is taller than a short window. */
const DISCOVERY: MediaServerDiscovery = {
	discoveryError: null,
	servers: Array.from({ length: 6 }, (_, index) => ({
		key: `192.0.2.${50 + index}:4809`,
		name: `ToskLight Pixel Media - Rack ${index + 1}`,
		host: `192.0.2.${50 + index}`,
		citpPort: 4809,
		status: "ready",
		instance: `Rack ${index + 1}`,
		error: null,
		outputs: [
			{
				id: `00000000-0000-4000-8000-0000000001${String(index).padStart(2, "0")}`,
				name: "Main",
				personality: "eight-layers",
				protocol: "sacn",
				universe: 60 + index,
				startAddress: 1,
				dmxPendingRestart: false,
				mode: "8 layers",
				tempoSource: "speed-group",
				speedGroup: 1,
				issue: null,
			} satisfies DiscoveredMediaOutput,
		],
	})),
};

const SIZES = [
	{ name: "wide", width: 1600, height: 1000 },
	{ name: "short", width: 1280, height: 560 },
	{ name: "small and short", width: 1024, height: 480 },
] as const;

type Box = { x: number; y: number; width: number; height: number };

async function box(locator: Locator): Promise<Box> {
	const found = await locator.boundingBox();
	if (!found) throw new Error("the control has no layout box");
	return found;
}

function patchHeader(page: Page): Locator {
	return page
		.locator("header.ui-window-header")
		.filter({ hasText: "Show Patch" })
		.first();
}

/** Where the view switch and the Settings button sit, rounded to whole pixels. */
async function headerPlaces(page: Page) {
	const header = patchHeader(page);
	const places: Record<string, Box> = {};
	for (const name of ["Fixtures", "Media Servers", "Tracking"])
		places[name] = await box(header.getByRole("tab", { name, exact: true }));
	places.Settings = await box(
		header.getByRole("button", { name: "Settings", exact: true }),
	);
	places.header = await box(header);
	return Object.fromEntries(
		Object.entries(places).map(([name, place]) => [
			name,
			{
				x: Math.round(place.x),
				y: Math.round(place.y),
				width: Math.round(place.width),
				height: Math.round(place.height),
			},
		]),
	);
}

async function openShowPatch(
	page: Page,
	baseUrl: string,
	desk: { open(url: string): Promise<void> },
) {
	await page.route(`**${DISCOVER}`, (route) =>
		route.fulfill({ json: DISCOVERY }),
	);
	await desk.open(baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await expect(
		patchHeader(page).getByRole("tab", { name: "Fixtures", exact: true }),
	).toHaveAttribute("aria-selected", "true");
}

/** Scroll the view's one scroller to its end and prove the last element's end is inside it. */
async function expectLastReachable(page: Page, last: Locator) {
	const scroller = page.locator(
		".patch-configuration-window .patch-configuration-scroll .ui-window-scroller",
	);
	await expect(scroller).toBeVisible();
	await scroller.evaluate((node) => {
		node.scrollTop = node.scrollHeight;
	});
	const frame = await box(scroller);
	const target = await box(last);
	expect(target.y + target.height).toBeLessThanOrEqual(
		frame.y + frame.height + 1,
	);
	expect(target.y + target.height).toBeGreaterThan(frame.y);
	expect(frame.y + frame.height).toBeLessThanOrEqual(
		(page.viewportSize()?.height ?? 0) + 1,
	);
	// The content keeps the Settings pages' 12px inner margin instead of touching the frame.
	const content = await box(
		page.locator(".patch-configuration-window .patch-configuration-content"),
	);
	const inner = await box(
		page.locator(".patch-configuration-content > *").first(),
	);
	expect(Math.round(inner.x - content.x)).toBe(12);
	expect(Math.round(content.x)).toBe(Math.round(frame.x));
}

for (const size of SIZES) {
	test(`TL-467 @ui › Show Patch views keep the header still and scroll to their end (${size.name})`, async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await page.setViewportSize({ width: size.width, height: size.height });
		await loadCanonicalCopy(api, bench, `tl-467-${size.name}`, "default-stage");
		await openShowPatch(page, api.baseUrl, desk);
		const header = patchHeader(page);
		const fixtures = await headerPlaces(page);

		await header
			.getByRole("tab", { name: "Media Servers", exact: true })
			.click();
		await expect(
			page.locator(".media-server-card").filter({ hasText: "Rack 6 · Main" }),
		).toBeVisible({ timeout: 10_000 });
		expect(await headerPlaces(page)).toEqual(fixtures);
		await expectLastReachable(
			page,
			page.locator(".media-server-setup > :last-child"),
		);

		await header.getByRole("tab", { name: "Tracking", exact: true }).click();
		await expect(
			page.getByRole("switch", { name: /Receive PosiStageNet/ }),
		).toBeVisible();
		expect(await headerPlaces(page)).toEqual(fixtures);
		await expect(page.getByLabel("Multicast group")).toHaveCount(0);
		await expectLastReachable(page, page.locator(".psn-setup > :last-child"));

		await header.getByRole("tab", { name: "Fixtures", exact: true }).click();
		expect(await headerPlaces(page)).toEqual(fixtures);
	});
}

test("TL-467 @ui › Tracking Settings keep, validate, and explain the source values", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await page.setViewportSize({ width: 1024, height: 480 });
	await loadCanonicalCopy(api, bench, "tl-467-settings", "default-stage");
	type Snapshot = {
		configuration: { group: string; port: number; stale_after_millis: number };
	};
	const before = await api.request<Snapshot>("GET", "/api/v2/psn");
	await openShowPatch(page, api.baseUrl, desk);
	const header = patchHeader(page);
	await header.getByRole("tab", { name: "Tracking", exact: true }).click();
	await header.getByRole("button", { name: "Settings", exact: true }).click();

	const settings = page.getByRole("dialog", { name: "Show Patch" });
	await expect(
		settings.getByRole("tab", { name: "Tracking", exact: true }),
	).toHaveAttribute("aria-selected", "true");
	const group = settings.getByLabel("Multicast group");
	const port = settings.getByLabel("Port", { exact: true });
	const stale = settings.getByLabel("Stale after (ms)");
	await expect(group).toHaveValue(before.configuration.group);
	await expect(port).toHaveValue(String(before.configuration.port));
	await expect(stale).toHaveValue(
		String(before.configuration.stale_after_millis),
	);
	// The Settings content keeps its inner margin on a short window too.
	const panel = await box(settings.locator(".ui-window-settings-content"));
	const section = await box(settings.locator(".tracking-settings"));
	expect(Math.round(section.x - panel.x)).toBe(18);

	await group.fill("10.0.0.1");
	const apply = settings.getByRole("button", {
		name: "Apply tracking settings",
	});
	await apply.scrollIntoViewIfNeeded();
	await apply.click();
	await expect(settings).toContainText("use 224.0.0.0 to 239.255.255.255");
	expect(
		(await api.request<Snapshot>("GET", "/api/v2/psn")).configuration,
	).toEqual(before.configuration);

	await group.fill("239.1.2.3");
	await port.fill("56570");
	await stale.fill("2500");
	await apply.scrollIntoViewIfNeeded();
	await expect(apply).toBeInViewport();
	await apply.click();
	await expect(settings.getByRole("status")).toHaveText(
		"Tracking settings saved.",
	);
	const stored = await api.request<Snapshot>("GET", "/api/v2/psn");
	expect(stored.configuration).toMatchObject({
		group: "239.1.2.3",
		port: 56570,
		stale_after_millis: 2500,
	});

	await settings.getByRole("button", { name: "Close settings" }).click();
	await expect(settings).toHaveCount(0);
	await header.getByRole("button", { name: "Settings", exact: true }).click();
	await expect(group).toHaveValue("239.1.2.3");
	await expect(port).toHaveValue("56570");
	await expect(stale).toHaveValue("2500");
});
