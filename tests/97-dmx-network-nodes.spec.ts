import dgram from "node:dgram";
import type {
	NetworkEndpoint,
	NetworkEndpointsSnapshot,
} from "../apps/light-desktop/src/api/generated/light-wire";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

test.setTimeout(180_000);
test.use({ viewport: { width: 1600, height: 1100 } });

const ENDPOINTS = "/api/v2/output/network-endpoints";

function artPoll(): Buffer {
	const packet = Buffer.alloc(14);
	packet.write("Art-Net\0", 0, "latin1");
	packet.writeUInt16LE(0x2000, 8);
	packet[11] = 14;
	return packet;
}

function artDmx(universe: number): Buffer {
	const packet = Buffer.alloc(18 + 24);
	packet.write("Art-Net\0", 0, "latin1");
	packet.writeUInt16LE(0x5000, 8);
	packet.writeUInt16BE(14, 10);
	packet.writeUInt16LE(universe, 14);
	packet.writeUInt16BE(24, 16);
	return packet;
}

/** An E1.31 universe-discovery page from another console. */
function sacnDiscovery(name: string, universes: number[]): Buffer {
	const packet = Buffer.alloc(120 + universes.length * 2);
	packet.writeUInt16BE(0x0010, 0);
	packet.write("ASC-E1.17\0\0\0", 4, "latin1");
	packet.writeUInt16BE(0x7000 | (packet.length - 16), 16);
	packet.writeUInt32BE(8, 18);
	packet.fill(0xab, 22, 38);
	packet.writeUInt16BE(0x7000 | (packet.length - 38), 38);
	packet.writeUInt32BE(2, 40);
	packet.write(name, 44, "utf8");
	packet.writeUInt16BE(0x7000 | (packet.length - 112), 112);
	packet.writeUInt32BE(1, 114);
	for (const [index, universe] of universes.entries()) {
		packet.writeUInt16BE(universe, 120 + index * 2);
	}
	return packet;
}

async function send(socket: dgram.Socket, packet: Buffer, endpoint: string) {
	const [host, port] = endpoint.split(":");
	await new Promise<void>((resolve, reject) =>
		socket.send(packet, Number(port), host, (error) =>
			error ? reject(error) : resolve(),
		),
	);
}

async function readEndpoints(api: ApiDriver) {
	return api.request<NetworkEndpointsSnapshot>("GET", ENDPOINTS);
}

function find(
	snapshot: NetworkEndpointsSnapshot,
	predicate: (endpoint: NetworkEndpoint) => boolean,
): NetworkEndpoint {
	const found = snapshot.endpoints.find(predicate);
	expect(found, JSON.stringify(snapshot.endpoints, null, 2)).toBeDefined();
	return found as NetworkEndpoint;
}

test("DMX-NODES-001 @api network endpoints report Art-Net and sACN sends and receives with actionable status", async ({
	api,
	bench,
}) => {
	await loadCanonicalCopy(api, bench, "dmx-nodes-001");
	await bench.tick(25);
	const initial = await readEndpoints(api);
	expect(initial.network_output_available).toBe(true);

	const artNetSend = find(
		initial,
		(endpoint) =>
			endpoint.protocol === "art_net" &&
			endpoint.direction === "send" &&
			endpoint.role === "DMX output",
	);
	const sacnSend = find(
		initial,
		(endpoint) =>
			endpoint.protocol === "sacn" &&
			endpoint.direction === "send" &&
			endpoint.role === "DMX output",
	);
	for (const route of [artNetSend, sacnSend]) {
		expect(route.origin).toBe("configured");
		expect(route.endpoint).toMatch(/^127\.0\.0\.1:\d+$/);
		expect(route.delivery_mode).toBe("unicast");
		expect(route.logical_universe).not.toBeNull();
		expect(route.universes).toHaveLength(1);
		expect(route.status).toBe("active");
	}
	expect(artNetSend.endpoint).toBe(`127.0.0.1:${bench.artnet.port}`);
	expect(sacnSend.endpoint).toBe(`127.0.0.1:${bench.sacn.port}`);
	const sacnAnnouncement = find(
		initial,
		(endpoint) => endpoint.id === "send:sacn:announce",
	);
	expect(sacnAnnouncement.universes).toContain(sacnSend.universes[0]);

	// The bench listens on loopback, where the desk hears peers without broadcast or multicast.
	const pollListener = find(
		initial,
		(endpoint) =>
			endpoint.protocol === "art_net" &&
			endpoint.direction === "receive" &&
			endpoint.role === "ArtPoll listener" &&
			endpoint.endpoint.startsWith("127.0.0.1:"),
	);
	expect(pollListener.status).toBe("listening");
	const sacnListener = find(
		initial,
		(endpoint) => endpoint.id === "receive:sacn:discovery",
	);
	expect(sacnListener.direction).toBe("receive");
	expect(sacnListener.status).toBe("listening");
	expect(sacnListener.endpoint).toMatch(/^127\.0\.0\.1:\d+$/);

	const peer = dgram.createSocket("udp4");
	await new Promise<void>((resolve) => peer.bind(0, "127.0.0.1", resolve));
	try {
		await send(peer, artPoll(), pollListener.endpoint);
		await send(peer, artDmx(artNetSend.universes[0]), pollListener.endpoint);
		await send(
			peer,
			sacnDiscovery("Backup console", [sacnSend.universes[0], 4000]),
			sacnListener.endpoint,
		);
		const peerPort = (peer.address() as dgram.AddressInfo).port;

		let heard = initial;
		await expect
			.poll(
				async () => {
					await bench.tick(25);
					heard = await readEndpoints(api);
					const ids = heard.endpoints.map((endpoint) => endpoint.id);
					return [
						`receive:artnet:poller:127.0.0.1:${peerPort}`,
						"receive:artnet:sender:127.0.0.1",
						`receive:sacn:source:${"ab".repeat(16)}`,
					].filter((id) => ids.includes(id)).length;
				},
				{ timeout: 10_000 },
			)
			.toBe(3);

		const poller = find(
			heard,
			(endpoint) =>
				endpoint.id === `receive:artnet:poller:127.0.0.1:${peerPort}`,
		);
		expect(poller.direction).toBe("receive");
		expect(poller.status).toBe("active");
		const artNetPeer = find(
			heard,
			(endpoint) => endpoint.id === "receive:artnet:sender:127.0.0.1",
		);
		expect(artNetPeer.universes).toEqual([artNetSend.universes[0]]);
		expect(artNetPeer.status).toBe("conflict");
		const sacnPeer = find(
			heard,
			(endpoint) => endpoint.id === `receive:sacn:source:${"ab".repeat(16)}`,
		);
		expect(sacnPeer.direction).toBe("receive");
		expect(sacnPeer.name).toBe("Backup console");
		expect(sacnPeer.universes).toEqual(
			[sacnSend.universes[0], 4000].sort((left, right) => left - right),
		);
		expect(sacnPeer.status).toBe("conflict");

		// Each send route names the competing source so the operator knows what to move.
		expect(
			find(heard, (endpoint) => endpoint.id === artNetSend.id),
		).toMatchObject({
			status: "conflict",
			detail: expect.stringContaining("127.0.0.1"),
		});
		expect(
			find(heard, (endpoint) => endpoint.id === sacnSend.id),
		).toMatchObject({
			status: "conflict",
			detail: expect.stringContaining("Backup console"),
		});
	} finally {
		peer.close();
	}

	// A failing destination is an error with the reason and where to look.
	await api.request(
		"POST",
		"/api/v2/test/output/failure",
		{ destination: artNetSend.endpoint, enabled: true },
		false,
	);
	try {
		await bench.tick(25);
		const failing = find(
			await readEndpoints(api),
			(endpoint) => endpoint.id === artNetSend.id,
		);
		expect(failing.status).toBe("error");
		expect(failing.errors).toBeGreaterThan(0);
		expect(failing.detail).toContain("injected output failure");
		expect(failing.detail).toContain("destination address");
	} finally {
		await api.request(
			"POST",
			"/api/v2/test/output/failure",
			{ destination: artNetSend.endpoint, enabled: false },
			false,
		);
	}
});

test("DMX-NODES-002 @ui the DMX window's Nodes tab lists Art-Net and sACN send and receive endpoints", async ({
	page,
	desk,
	api,
	bench,
}) => {
	page.setDefaultTimeout(12_000);
	await loadCanonicalCopy(api, bench, "dmx-nodes-002");
	await desk.open(api.baseUrl);
	await expect(page.locator(".connection-cover")).toBeHidden();
	// The full DMX built-in carries the view tabs; a desktop pane is the compact monitor.
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "DMX", exact: true }).click();
	const pane = page.locator(".dmx-window");
	await expect(pane).toBeVisible();
	const tabs = pane.getByRole("tab");
	await expect(tabs).toHaveText(["Values", "Sources", "Nodes"]);
	await pane.getByRole("tab", { name: "Nodes" }).click();
	await bench.tick(25);

	const table = pane.getByRole("table", { name: "Network nodes" });
	await expect(table.getByRole("columnheader")).toHaveText([
		"Protocol",
		"Direction",
		"Endpoint",
		"Universe",
		"Status",
	]);
	const artNetSend = table
		.locator("tr[data-endpoint-id^='send:artnet:']")
		.filter({ hasText: `127.0.0.1:${bench.artnet.port}` });
	await expect(artNetSend).toContainText("Art-Net");
	await expect(artNetSend).toContainText("Send");
	await expect(artNetSend).toContainText("Unicast");
	await expect(artNetSend).toContainText(/\d+ → \d+/);
	const sacnSend = table
		.locator("tr[data-endpoint-id^='send:sacn:']")
		.filter({ hasText: `127.0.0.1:${bench.sacn.port}` });
	await expect(sacnSend).toContainText("sACN");
	await expect(sacnSend).toContainText("Send");
	for (const row of [artNetSend, sacnSend]) {
		await expect(row.locator(".dmx-nodes-status")).toHaveText(/Active|Idle/);
	}
	await expect(
		table.locator("tr[data-endpoint-id^='receive:artnet:poll']").first(),
	).toContainText("Receive");
	await expect(
		table.locator("tr[data-endpoint-id='receive:sacn:discovery']"),
	).toContainText("Listening");

	const aside = pane.locator(".dmx-nodes-pane");
	await expect(aside).toContainText("Network summary");
	await expect(aside).toContainText("Art-Net send");
	await expect(aside).toContainText("sACN receive");

	await sacnSend.getByRole("button").click();
	await expect(aside).toContainText("Selected endpoint");
	await expect(aside).toContainText(`127.0.0.1:${bench.sacn.port}`);
});
