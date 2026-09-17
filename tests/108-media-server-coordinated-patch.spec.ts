import * as net from "node:net";
import type { Locator, Page } from "@playwright/test";
import type {
	DiscoveredMediaAddressUpdateRequest,
	DiscoveredMediaOutput,
	MediaServerDiscovery,
} from "../apps/light-desktop/src/api/generated/light-wire";
import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

test.use({ viewport: { width: 1600, height: 1100 } });

const DISCOVER = "/api/v2/media-servers/discover";
const ADDRESS = "/api/v2/media-servers/discovered/address";
const RACK_A_OUTPUT = "00000000-0000-4000-8000-000000000051";
const RACK_B_OUTPUT = "00000000-0000-4000-8000-000000000052";

type PatchedFixtureRecord = {
	fixture_id: string;
	universe: number | null;
	address: number | null;
	direct_control: { ip_address: string; port: number } | null;
	internal_bindings?: { output?: string | null } | null;
};

/** A local TCP port nothing listens on, so the desk's CITP connects are refused at once. */
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

function output(
	overrides: Partial<DiscoveredMediaOutput>,
): DiscoveredMediaOutput {
	return {
		id: RACK_A_OUTPUT,
		name: "Main",
		personality: "two-layers",
		protocol: "sacn",
		universe: 101,
		startAddress: 355,
		dmxPendingRestart: false,
		mode: "2 layers",
		tempoSource: "playback-bpm-channel",
		speedGroup: null,
		issue: null,
		...overrides,
	};
}

function card(page: Page, title: string): Locator {
	return page.locator(".media-server-card").filter({ hasText: title });
}

async function patchedFixtures(api: {
	patch: () => Promise<{ fixtures: unknown[] }>;
}): Promise<PatchedFixtureRecord[]> {
	return (await api.patch()).fixtures as PatchedFixtureRecord[];
}

async function chooseAddress(
	target: Locator,
	universe: number,
	address: number,
) {
	await target
		.getByRole("button", { name: "Patch address", exact: true })
		.click();
	await target.getByLabel("Universe").fill(String(universe));
	await target.getByLabel("Address").fill(String(address));
	await target.getByRole("button", { name: "Confirm patch address" }).click();
}

test.describe("docs/testing/14-media-and-running-panes.md", () => {
	test("MEDIA-008 @ui › Show Patch coordinates discovered Media Servers with the desk routes", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		// default-stage sends desk universe 1 as Art-Net 1 and as sACN 101.
		await loadCanonicalCopy(api, bench, "media-008-ui", "default-stage");
		const portA = await closedPort();
		const portB = await closedPort();
		const discovery: MediaServerDiscovery = {
			discoveryError: null,
			servers: [
				{
					key: `127.0.0.1:${portA}`,
					name: "ToskLight Pixel Media - Rack A",
					host: "127.0.0.1",
					citpPort: portA,
					status: "ready",
					instance: "Rack A",
					error: null,
					outputs: [output({})],
				},
				{
					key: `127.0.0.1:${portB}`,
					name: "ToskLight Pixel Media - Rack B",
					host: "127.0.0.1",
					citpPort: portB,
					status: "ready",
					instance: "Rack B",
					error: null,
					outputs: [
						output({
							id: RACK_B_OUTPUT,
							name: "Side",
							protocol: "art-net",
							universe: 30,
							startAddress: 1,
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
		await page.route(`**${DISCOVER}`, (route) =>
			route.fulfill({ json: discovery }),
		);
		// The server side of a coordinated update: the first attempt fails, later ones apply.
		const updates: DiscoveredMediaAddressUpdateRequest[] = [];
		await page.route(`**${ADDRESS}`, async (route) => {
			const body = route
				.request()
				.postDataJSON() as DiscoveredMediaAddressUpdateRequest;
			updates.push(body);
			if (updates.length === 1)
				return route.fulfill({
					status: 503,
					json: { error: "The Media Server is unreachable." },
				});
			return route.fulfill({
				json: output({
					id: body.outputId,
					name: "Side",
					protocol: body.protocol ?? "art-net",
					universe: body.universe,
					startAddress: body.startAddress,
				}),
			});
		});
		await desk.open(api.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page.getByRole("button", { name: "Show Patch", exact: true }).click();
		await page.getByRole("tab", { name: "Media Servers", exact: true }).click();

		// Every server is named by identity, address, type, and state; unpatched is explicit.
		const rackA = card(page, "Rack A · Main");
		const rackB = card(page, "Rack B · Side");
		const spare = card(page, "Pixel Spare");
		await expect(rackA).toContainText(
			`127.0.0.1 · ToskLight Media · CITP ${portA} · Online · Desk connection: Not connected (not patched)`,
		);
		await expect(rackA).toContainText(
			"Suggested DMX 1.355 · 2 layers · listens to sACN 101",
		);
		await expect(rackA.getByText("Not patched", { exact: true })).toBeVisible();
		await expect(rackB).toContainText(
			"listens to Art-Net 30, which no desk route sends",
		);
		await expect(spare).toContainText(
			"192.0.2.43 · ToskLight Media · CITP 4809 · Offline",
		);
		await expect(spare.getByText("Unavailable", { exact: true })).toBeVisible();
		await expect(spare.getByRole("alert")).toContainText("port 8080");

		// Desk validation (collision, footprint, missing route) runs before the server is asked.
		const rackBStatus = rackB.getByRole("status");
		await chooseAddress(rackB, 1, 1);
		await expect(rackBStatus).toContainText(
			/overlap in the patch on universe 1, addresses \d+-\d+\..*The Media Server was not changed\./,
		);
		// 2 layers occupies 158 slots, so 1.400 would run past slot 512.
		await chooseAddress(rackB, 1, 400);
		await expect
			.poll(() => rackBStatus.textContent())
			.toMatch(/exceeds universe 1\. The Media Server was not changed\.$/);
		await chooseAddress(rackB, 20, 1);
		await expect(rackB.getByRole("status")).toContainText(
			"The desk sends no network output for universe 20",
		);
		expect(updates).toHaveLength(0);
		expect(
			(await patchedFixtures(api)).filter(
				(fixture) => fixture.internal_bindings?.output === RACK_B_OUTPUT,
			),
		).toHaveLength(0);

		// A failed coordinated update restores the desk.
		await chooseAddress(rackB, 1, 180);
		await expect(rackB.getByRole("status")).toContainText(
			"The desk patch was restored",
		);
		expect(updates).toHaveLength(1);
		expect(
			(await patchedFixtures(api)).filter(
				(fixture) => fixture.internal_bindings?.output === RACK_B_OUTPUT,
			),
		).toHaveLength(0);
		await expect(rackB.getByText("Not patched", { exact: true })).toBeVisible();

		// The retry moves the server onto the route the desk sends: Art-Net 1, address 180.
		await chooseAddress(rackB, 1, 180);
		await expect(rackB.getByRole("status")).toContainText(
			"Desk and Media Server now use DMX 1.180; the server listens to Art-Net 1.",
		);
		expect(updates[1]).toMatchObject({
			host: "127.0.0.1",
			outputId: RACK_B_OUTPUT,
			universe: 1,
			startAddress: 180,
			protocol: "art-net",
		});
		await expect(rackB.getByText("Patched", { exact: true })).toBeVisible();
		const side = (await patchedFixtures(api)).find(
			(fixture) => fixture.internal_bindings?.output === RACK_B_OUTPUT,
		);
		expect(side).toMatchObject({
			universe: 1,
			address: 180,
			direct_control: { ip_address: "127.0.0.1", port: portB },
		});

		// Patch suggested maps sACN 101 back to desk universe 1 and never asks the server.
		await rackA.getByRole("button", { name: "Patch suggested" }).click();
		await expect(rackA.getByRole("status")).toContainText(
			"Patched at DMX 1.355.",
		);
		await expect(rackA.getByText("Patched", { exact: true })).toBeVisible();
		expect(updates).toHaveLength(2);

		// Each patched row refreshes its own connection on request.
		const table = page.getByRole("table", { name: "Patched Media Servers" });
		const rackARow = table
			.getByRole("row")
			.filter({ hasText: "Rack A Main" })
			.first();
		await rackARow.getByRole("button", { name: "Check connection" }).click();
		await expect(rackARow).toContainText("● Offline", { timeout: 10_000 });
		await expect(rackARow.getByRole("alert")).toContainText(
			"then Check connection to retry",
		);
		await expect(rackA).toContainText("Desk connection: Offline");
	});

	test("MEDIA-008 @api › a coordinated address update validates the protocol universe", async ({
		api,
	}) => {
		await api.login();
		const bootstrap = await api.request<{ active_show: { id: string } | null }>(
			"GET",
			"/api/v2/bootstrap",
		);
		const context = {
			showId: bootstrap.active_show?.id,
			deskId: api.session?.desk.id,
		};
		const base = {
			host: "127.0.0.1",
			outputId: RACK_A_OUTPUT,
			startAddress: 1,
		};
		for (const [body, reason] of [
			[
				{ ...base, universe: 0, protocol: "sacn" },
				"sACN universe must be from 1",
			],
			[
				{ ...base, universe: 32_768, protocol: "art-net" },
				"Art-Net universe must be from 0",
			],
			[{ ...base, universe: 1, protocol: "kinet" }, "use art-net or sacn"],
		] as const) {
			await expect(
				api.request(
					"POST",
					ADDRESS,
					{ requestId: crypto.randomUUID(), ...body },
					true,
					undefined,
					context,
				),
			).rejects.toThrow(reason);
		}
	});
});
