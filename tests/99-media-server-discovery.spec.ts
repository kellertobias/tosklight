import type {
	DiscoveredMediaOutput,
	MediaServerDiscovery,
} from "../apps/light-desktop/src/api/generated/light-wire";
import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

test.use({ viewport: { width: 1600, height: 1100 } });

const DISCOVER = "/api/v2/media-servers/discover";

function output(
	overrides: Partial<DiscoveredMediaOutput>,
): DiscoveredMediaOutput {
	return {
		id: "00000000-0000-4000-8000-000000000041",
		name: "Main",
		personality: "eight-layers",
		protocol: "sacn",
		universe: 42,
		startAddress: 1,
		dmxPendingRestart: false,
		mode: "8 layers",
		tempoSource: "speed-group",
		speedGroup: 3,
		issue: null,
		...overrides,
	};
}

/**
 * A configured server, one that predates the current personalities, and one that answers
 * discovery but not its configuration API — the three states an operator has to tell apart.
 */
const DISCOVERY: MediaServerDiscovery = {
	discoveryError: null,
	servers: [
		{
			key: "192.0.2.41:4809",
			name: "ToskLight Pixel Media - Rack A",
			host: "192.0.2.41",
			citpPort: 4809,
			status: "ready",
			instance: "Rack A",
			error: null,
			outputs: [output({})],
		},
		{
			key: "192.0.2.42:4809",
			name: "ToskLight Pixel Media - Rack B",
			host: "192.0.2.42",
			citpPort: 4809,
			status: "ready",
			instance: "Rack B",
			error:
				"This Media Server needs an update before the desk can patch it. Update ToskLight Media to the current version, then refresh discovery.",
			outputs: [
				output({
					id: "00000000-0000-4000-8000-000000000042",
					name: "Side",
					personality: "two-layers",
					universe: 43,
					mode: null,
					tempoSource: null,
					speedGroup: null,
					issue:
						"This Media Server needs an update before the desk can patch it. Update ToskLight Media to the current version, then refresh discovery.",
				}),
			],
		},
		{
			key: "192.0.2.43:4809",
			name: "Pixel Spare",
			host: "192.0.2.43",
			citpPort: 4809,
			status: "Unavailable",
			instance: null,
			error:
				"The discovered Media Server did not answer its configuration API. Check that it is running and reachable on port 8080, then refresh discovery.",
			outputs: [],
		},
	],
};

test("MEDIA-006 @api discovery describes every server by the current Media contract", async ({
	api,
	bench,
}) => {
	await loadCanonicalCopy(api, bench, "media-006-api", "default-stage");
	const discovery = await api.request<MediaServerDiscovery>("GET", DISCOVER);
	expect(Array.isArray(discovery.servers)).toBe(true);
	for (const server of discovery.servers) {
		expect(server.outputs.length > 0 || Boolean(server.error)).toBe(true);
		for (const discovered of server.outputs) {
			// A patchable output names a shipped profile mode; anything else says why not.
			if (discovered.mode) {
				expect(["2 layers", "8 layers"]).toContain(discovered.mode);
				expect(discovered.issue).toBeNull();
			} else {
				expect(discovered.issue).toBeTruthy();
			}
		}
	}
});

test("MEDIA-006 @ui Show Patch tells a configured, outdated, and unavailable Media Server apart", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await loadCanonicalCopy(api, bench, "media-006-ui", "default-stage");
	await page.route(`**${DISCOVER}`, (route) =>
		route.fulfill({ json: DISCOVERY }),
	);
	await desk.open(api.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await page.getByRole("tab", { name: "Media Servers", exact: true }).click();

	const configured = page
		.locator(".media-server-card")
		.filter({ hasText: "Rack A · Main" });
	await expect(configured).toContainText(
		"Suggested DMX 42.1 · 8 layers · sACN · Tempo from Speed Group 3",
	);
	await expect(
		configured.getByText("Not patched", { exact: true }),
	).toBeVisible();
	await expect(
		configured.getByRole("button", { name: "Patch suggested" }),
	).toBeEnabled();
	await expect(configured).not.toContainText("eight-layers");

	const outdated = page
		.locator(".media-server-card")
		.filter({ hasText: "Rack B · Side" });
	await expect(
		outdated.getByText("Needs update", { exact: true }),
	).toBeVisible();
	await expect(outdated.getByRole("alert")).toContainText(
		"Update ToskLight Media",
	);
	await expect(
		outdated.getByRole("button", { name: "Patch suggested" }),
	).toBeDisabled();
	await expect(
		outdated.getByRole("button", { name: "Patch address" }),
	).toBeDisabled();

	const unavailable = page
		.locator(".media-server-card")
		.filter({ hasText: "Pixel Spare" });
	await expect(
		unavailable.getByText("Unavailable", { exact: true }),
	).toBeVisible();
	await expect(unavailable.getByRole("alert")).toContainText(
		"reachable on port 8080",
	);

	await configured.getByRole("button", { name: "Patch suggested" }).click();
	await expect(configured.getByRole("status")).toContainText(
		"Patched at DMX 42.1.",
	);
	await expect(configured.getByText("Patched", { exact: true })).toBeVisible();
	const patched = await api.request<{
		fixtures: Array<{ kind: string; layers: unknown[] }>;
	}>("GET", "/api/v2/media-servers");
	expect(
		patched.fixtures.filter(
			(fixture) =>
				fixture.kind === "media_server" && fixture.layers.length === 8,
		),
	).toHaveLength(1);
});
