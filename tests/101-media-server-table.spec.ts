import * as net from "node:net";
import type { Locator, Page } from "@playwright/test";
import type { MediaServerDiscovery } from "../apps/light-desktop/src/api/generated/light-wire";
import { expect, test } from "./bench/core/fixtures";
import { CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION } from "./support/fixtureSchema";

test.use({ viewport: { width: 1600, height: 1000 } });

const DISCOVER = "/api/v2/media-servers/discover";

type MediaServersSnapshot = {
	fixtures: Array<{
		fixture_id: string;
		endpoint: PatchedFixtureRecord["direct_control"];
		status: { online: boolean; last_error: string | null };
	}>;
};

type PatchedFixtureRecord = {
	fixture_id: string;
	direct_control: {
		protocol: string;
		ip_address: string;
		port: number;
	} | null;
};

/** A local TCP port nothing listens on, so CITP connects are refused at once. */
async function closedPort(): Promise<number> {
	const server = net.createServer();
	await new Promise<void>((resolve, reject) => {
		server.once("error", reject);
		server.listen(0, "127.0.0.1", resolve);
	});
	const port = (server.address() as net.AddressInfo).port;
	await new Promise<void>((resolve) => server.close(() => resolve()));
	return port;
}

/** A CITP-capable media fixture with its profile embedded, as a show file stores it. */
function mediaFixture(input: {
	number: number;
	name: string;
	manufacturer: string;
	model: string;
	address: number;
	endpoint: PatchedFixtureRecord["direct_control"];
}) {
	const fixtureId = crypto.randomUUID();
	const profileId = crypto.randomUUID();
	const modeId = crypto.randomUUID();
	const headId = crypto.randomUUID();
	const profile = {
		schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
		id: profileId,
		revision: 1,
		manufacturer: input.manufacturer,
		name: input.model,
		short_name: input.model,
		fixture_type: "media_server",
		patch_policy: "dmx",
		direct_control_protocols: ["citp"],
		modes: [
			{
				id: modeId,
				name: "1ch",
				splits: [{ number: 1, footprint: 1 }],
				heads: [{ id: headId, name: "Main", master_shared: true }],
				channels: [
					{
						id: crypto.randomUUID(),
						head_id: headId,
						split: 1,
						fixture_attribute: "intensity",
						attribute: "intensity",
						canonical_transform: "identity",
						resolution: "u8",
						secondary_slots: [],
						default_raw: 0,
						highlight_raw: 255,
						physical_min: 0,
						physical_max: 1,
						unit: null,
						invert: false,
						snap: false,
						reacts_to_virtual_intensity: false,
						reacts_to_sequence_master: true,
						reacts_to_group_master: true,
						reacts_to_grand_master: true,
						behavior: "controlled",
						functions: [],
					},
				],
			},
		],
	};
	return {
		fixture_id: fixtureId,
		fixture_number: input.number,
		name: input.name,
		definition: {
			schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
			id: profileId,
			revision: 1,
			manufacturer: input.manufacturer,
			device_type: "media_server",
			name: input.model,
			model: input.model,
			mode: "1ch",
			footprint: 1,
			heads: [
				{
					index: 0,
					name: "Main",
					shared: true,
					parameters: [
						{
							attribute: "intensity",
							components: [{ offset: 0, byte_order: "msb_first" }],
							default: 0,
							virtual_dimmer: false,
							metadata: {
								physical_min: 0,
								physical_max: 1,
								unit: null,
								invert: false,
								wrap: false,
								curve: "linear",
							},
							capabilities: [],
						},
					],
				},
			],
			color_calibration: null,
			physical: {},
			model_asset: null,
			icon_asset: null,
			hazardous: false,
			direct_control_protocols: ["citp"],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
			profile_id: profileId,
			mode_id: modeId,
			profile_snapshot: profile,
		},
		universe: 9,
		address: input.address,
		layer_id: "default",
		direct_control: input.endpoint,
		location: { x: input.number * 100, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
	};
}

function patchHeader(page: Page): Locator {
	return page
		.locator("header.ui-window-header")
		.filter({ hasText: "Show Patch" })
		.first();
}

function row(page: Page, name: string): Locator {
	return page
		.getByRole("table", { name: "Patched Media Servers" })
		.getByRole("row")
		.filter({ has: page.getByRole("rowheader", { name, exact: true }) });
}

async function chooseProtocol(page: Page, name: string, protocol: string) {
	await page.getByRole("button", { name: `${name} protocol` }).click();
	await page.getByRole("option", { name: protocol, exact: true }).click();
}

test.describe("docs/testing/14-media-and-running-panes.md", () => {
	test("MEDIA-007 @ui › Show Patch manages patched Media Servers in one live table", async ({
		api,
		desk,
		page,
		show,
	}) => {
		const refused = await closedPort();
		const generic = mediaFixture({
			number: 701,
			name: "Stage Media",
			manufacturer: "E2E",
			model: "Generic CITP Server",
			address: 1,
			endpoint: { protocol: "citp", ip_address: "127.0.0.1", port: refused },
		});
		const tosk = mediaFixture({
			number: 702,
			name: "Pixel Rack",
			manufacturer: "ToskLight",
			model: "Media Server",
			address: 2,
			endpoint: null,
		});
		for (const fixture of [generic, tosk])
			await api.seedShowObject(
				show.id,
				"patched_fixture",
				fixture.fixture_id,
				fixture,
			);
		await api.openShow(show.id, { transition: "hold_current" });

		const discovery: MediaServerDiscovery = {
			discoveryError: null,
			servers: [
				{
					key: `127.0.0.1:${refused}`,
					name: "Stage Media",
					host: "127.0.0.1",
					citpPort: refused,
					status: "Unavailable",
					instance: null,
					error:
						"The discovered Media Server did not answer its configuration API. Check that it is running and reachable on port 8080, then refresh discovery.",
					outputs: [],
				},
			],
		};
		let discoveries = 0;
		await page.route(`**${DISCOVER}`, (route) => {
			discoveries += 1;
			return route.fulfill({ json: discovery });
		});
		await desk.open(api.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page.getByRole("button", { name: "Show Patch", exact: true }).click();
		const header = patchHeader(page);
		await header
			.getByRole("tab", { name: "Media Servers", exact: true })
			.click();

		// Refresh Discovery is a title button in a group of its own, before the view switch.
		const refresh = header.getByRole("button", { name: "Refresh Discovery" });
		await expect(refresh).toBeVisible();
		const group = header.locator(".ui-title-chrome-group").filter({
			has: page.getByRole("button", { name: "Refresh Discovery" }),
		});
		await expect(group.getByRole("button")).toHaveCount(1);
		const refreshBox = await refresh.boundingBox();
		const tabBox = await header
			.getByRole("tab", { name: "Fixtures", exact: true })
			.boundingBox();
		expect(refreshBox && tabBox && refreshBox.x < tabBox.x).toBe(true);
		await expect.poll(() => discoveries).toBe(1);
		await refresh.click();
		await expect.poll(() => discoveries).toBe(2);
		await expect(
			page
				.locator(".media-discovery")
				.getByRole("button", { name: /discovery/i }),
		).toHaveCount(0);

		// One row per patched server, with its type.
		const genericRow = row(page, "Stage Media");
		const toskRow = row(page, "Pixel Rack");
		await expect(genericRow).toContainText("CITP media server");
		await expect(toskRow).toContainText("ToskLight Media");
		await expect(toskRow).toContainText("● Off");
		await expect(genericRow).toContainText("Found, needs attention");

		// The desk's own connection attempt fails, and the row says what to do about it.
		await expect(genericRow).toContainText("● Offline", { timeout: 10_000 });
		await expect(genericRow.getByRole("alert")).toContainText(
			"Check the IP address, port, and that the server is running",
		);
		const statuses = await api.request<MediaServersSnapshot>(
			"GET",
			"/api/v2/media-servers",
		);
		const genericStatus = statuses.fixtures.find(
			(entry) => entry.fixture_id === generic.fixture_id,
		);
		expect(genericStatus?.status.online).toBe(false);
		expect(genericStatus?.status.last_error).toBeTruthy();

		// Refreshing one row reports on that row only.
		await genericRow
			.getByRole("button", { name: "Refresh Thumbnails" })
			.click();
		await expect(genericRow).toContainText("Thumbnails were not refreshed.");
		await expect(toskRow).not.toContainText("Thumbnails were not refreshed.");
		await expect(
			toskRow.getByRole("button", { name: "Pixel Rack protocol" }),
		).toBeEnabled();

		// The ToskLight row validates before it applies, then follows its new endpoint.
		await chooseProtocol(page, "Pixel Rack", "CITP");
		const toskApply = toskRow.getByRole("button", { name: "Apply" });
		await expect(toskRow.getByRole("alert")).toContainText(
			"Enter the server's IP address",
		);
		await expect(toskApply).toBeDisabled();
		await toskRow.getByLabel("Pixel Rack IP address").fill("999.1.1.1");
		await expect(toskRow.getByRole("alert")).toContainText(
			"Enter an IPv4 or IPv6 address",
		);
		await expect(toskApply).toBeDisabled();
		await toskRow.getByLabel("Pixel Rack IP address").fill("127.0.0.1");
		await toskRow.getByLabel("Pixel Rack port").fill(String(refused));
		await expect(toskApply).toBeEnabled();
		await toskApply.click();
		await expect(toskRow).toContainText(`Now using 127.0.0.1:${refused}.`);
		await expect(toskRow).toContainText("● Offline", { timeout: 10_000 });
		const applied = await api.request<MediaServersSnapshot>(
			"GET",
			"/api/v2/media-servers",
		);
		expect(
			applied.fixtures.find((entry) => entry.fixture_id === tosk.fixture_id)
				?.endpoint,
		).toEqual({ protocol: "citp", ip_address: "127.0.0.1", port: refused });

		// Clear Thumbnail Cache lives in the Media Servers page of Show Patch Settings.
		await header.getByRole("button", { name: "Settings", exact: true }).click();
		const settings = page.getByRole("dialog", { name: "Show Patch" });
		await expect(
			settings.getByRole("tab", { name: "Media Servers", exact: true }),
		).toHaveAttribute("aria-selected", "true");
		await settings
			.getByRole("button", { name: "Clear Thumbnail Cache" })
			.click();
		await expect(settings.getByRole("status")).toContainText(
			"cached thumbnail",
		);
		await settings.getByRole("button", { name: "Close settings" }).click();

		// Turning the generic server off leaves it patched and in the table.
		await chooseProtocol(page, "Stage Media", "Off");
		await genericRow.getByRole("button", { name: "Apply" }).click();
		await expect(genericRow).toContainText(
			"Network control is off for this server.",
		);
		await expect(genericRow).toContainText("● Off");
		const after = await api.request<MediaServersSnapshot>(
			"GET",
			"/api/v2/media-servers",
		);
		expect(after.fixtures.map((entry) => entry.fixture_id)).toContain(
			generic.fixture_id,
		);
	});

	test("MEDIA-007 @api › clearing the thumbnail cache is repeatable", async ({
		api,
	}) => {
		for (let attempt = 0; attempt < 2; attempt += 1) {
			const cleared = await api.request<{ cleared: number }>(
				"POST",
				"/api/v2/media-servers/thumbnail-cache/clear",
			);
			expect(Number.isInteger(cleared.cleared)).toBe(true);
			if (attempt === 1) expect(cleared.cleared).toBe(0);
		}
	});
});
