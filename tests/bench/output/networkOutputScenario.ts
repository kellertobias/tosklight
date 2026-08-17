import { expect, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { type DmxProtocol, DmxReceiver } from "../core/protocols";
import { CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION } from "../../support/fixtureSchema";

export type NetworkProtocol = "art_net" | "sacn";
export type NetworkDeliveryMode = "broadcast" | "multicast" | "unicast";

export interface NetworkRouteIntent {
	protocol: NetworkProtocol;
	logicalUniverse: number;
	destinationUniverse: number;
	deliveryMode?: NetworkDeliveryMode;
	enabled?: boolean;
	minimumSlots?: number;
	capture?: boolean;
}

interface CapturedRoute {
	id: string;
	intent: Required<Omit<NetworkRouteIntent, "capture">>;
	receiver?: DmxReceiver;
	mark: number;
}

/**
 * Visible network-output route setup plus decoded UDP observations for semantic
 * browser scenarios. Raw datagram/header permutations remain in the low-level
 * protocol specs.
 */
export class BrowserNetworkOutput {
	private readonly routes = new Map<string, CapturedRoute>();

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	async add(label: string, input: NetworkRouteIntent): Promise<void> {
		assertLabel(label);
		if (this.routes.has(label))
			throw new Error(`Network output route "${label}" already exists`);
		const intent = normalizedIntent(input);
		const receiver = input.capture ? await DmxReceiver.bind() : undefined;
		if (input.capture && intent.deliveryMode !== "unicast")
			throw new Error("Captured network output routes must use unicast");
		const destination = receiver ? `127.0.0.1:${receiver.port}` : undefined;
		await this.desk.recordStep(
			"OUTPUT ROUTE",
			`Add ${protocolLabel(intent.protocol)} route ${intent.logicalUniverse} to ${intent.destinationUniverse} through the visible Outputs editor.`,
		);
		const region = await this.openRoutes();
		await this.desk.click(
			region.getByRole("button", { name: "Add route", exact: true }),
		);
		const editor = this.page.getByRole("dialog", {
			name: "Output route editor",
		});
		if (intent.protocol === "sacn") {
			await this.desk.click(
				editor.getByRole("button", { name: "Art-Net", exact: true }),
			);
			await this.desk.click(
				this.page.getByRole("option", { name: "sACN", exact: true }),
			);
		}
		const defaultMode =
			intent.protocol === "art_net" ? "broadcast" : "multicast";
		if (intent.deliveryMode !== defaultMode) {
			await this.desk.click(
				editor.getByRole("button", {
					name: deliveryLabel(defaultMode),
					exact: true,
				}),
			);
			await this.desk.click(
				this.page.getByRole("option", {
					name: deliveryLabel(intent.deliveryMode),
					exact: true,
				}),
			);
		}
		await editor
			.getByLabel("Logical universe")
			.fill(String(intent.logicalUniverse));
		await editor
			.getByLabel("Destination universe")
			.fill(String(intent.destinationUniverse));
		if (intent.deliveryMode === "unicast")
			await editor
				.getByLabel("Destination", { exact: true })
				.fill(destination ?? "127.0.0.1:6454");
		await editor
			.getByLabel("Minimum universe size")
			.fill(String(intent.minimumSlots));
		if (!intent.enabled)
			await this.desk.click(editor.locator(".ui-switch-control"));
		await this.desk.click(
			editor.getByRole("button", { name: "Save route", exact: true }),
		);
		const card = region
			.locator("article")
			.filter({ hasText: routeDescription(intent) });
		await expect(card).toBeVisible();
		if (!intent.enabled) await expect(card).toContainText("Disabled");
		const stored = (await this.objects<any>("route")).find(
			(entry) =>
				entry.body.protocol === intent.protocol &&
				entry.body.logical_universe === intent.logicalUniverse &&
				entry.body.destination_universe === intent.destinationUniverse,
		);
		if (!stored) throw new Error(`Output route "${label}" was not persisted`);
		this.routes.set(label, {
			id: stored.id,
			intent,
			receiver,
			mark: receiver?.mark() ?? 0,
		});
	}

	async installSixteenBitFixture(fixtureNumber = 900): Promise<number> {
		if (!Number.isSafeInteger(fixtureNumber) || fixtureNumber < 1)
			throw new Error("Fixture numbers start at 1");
		const fixtureId = crypto.randomUUID();
		const profileId = crypto.randomUUID();
		const modeId = crypto.randomUUID();
		const headId = crypto.randomUUID();
		await this.api.seedShowObject(
			await this.activeShowId(),
			"patched_fixture",
			`semantic-u16-${fixtureNumber}`,
			sixteenBitFixture({
				fixtureId,
				fixtureNumber,
				profileId,
				modeId,
				headId,
			}),
			0,
		);
		return fixtureNumber;
	}

	mark(label: string): void {
		const route = this.required(label);
		route.mark = route.receiver?.mark() ?? 0;
	}

	async expectPacket(
		label: string,
		expected: {
			slots?: Readonly<Record<number, number>>;
			sequenceNonZero?: boolean;
		},
	): Promise<void> {
		const route = this.requiredCapture(label);
		const protocol = wireProtocol(route.intent.protocol);
		const deadline = Date.now() + 2_000;
		let candidates = route.receiver
			.packetsAfter(route.mark)
			.filter(
				(packet) =>
					packet.protocol === protocol &&
					packet.universe === route.intent.destinationUniverse,
			);
		let packet = candidates.find((candidate) =>
			packetMatches(candidate, expected),
		);
		while (!packet && Date.now() < deadline) {
			await new Promise((resolve) => setTimeout(resolve, 5));
			candidates = route.receiver
				.packetsAfter(route.mark)
				.filter(
					(candidate) =>
						candidate.protocol === protocol &&
						candidate.universe === route.intent.destinationUniverse,
				);
			packet = candidates.find((candidate) =>
				packetMatches(candidate, expected),
			);
		}
		if (!packet)
			throw new Error(
				`Output route "${label}" received no matching packet; observed ${JSON.stringify(
					candidates.map((candidate) => ({
						sequence: candidate.sequence,
						slots: Object.fromEntries(
							Object.keys(expected.slots ?? {}).map((address) => [
								address,
								candidate.slots[Number(address) - 1],
							]),
						),
					})),
				)}`,
			);
		for (const [address, value] of Object.entries(expected.slots ?? {}))
			expect(packet.slots[Number(address) - 1]).toBe(value);
		if (expected.sequenceNonZero) expect(packet.sequence).not.toBe(0);
	}

	async expectNoPacket(label: string): Promise<void> {
		const route = this.requiredCapture(label);
		await new Promise((resolve) => setTimeout(resolve, 75));
		expect(
			route.receiver
				.packetsAfter(route.mark)
				.filter(
					(packet) =>
						packet.protocol === wireProtocol(route.intent.protocol) &&
						packet.universe === route.intent.destinationUniverse,
				),
		).toHaveLength(0);
	}

	async expectSamePayload(
		firstLabel: string,
		secondLabel: string,
	): Promise<void> {
		const first = this.requiredCapture(firstLabel);
		const second = this.requiredCapture(secondLabel);
		const [firstPacket, secondPacket] = await Promise.all([
			first.receiver.nextAfter(
				first.mark,
				wireProtocol(first.intent.protocol),
				first.intent.destinationUniverse,
			),
			second.receiver.nextAfter(
				second.mark,
				wireProtocol(second.intent.protocol),
				second.intent.destinationUniverse,
			),
		]);
		expect(Array.from(firstPacket.slots)).toEqual(
			Array.from(secondPacket.slots),
		);
	}

	async failure(label: string, enabled: boolean): Promise<void> {
		const route = this.requiredCapture(label);
		const destination = `127.0.0.1:${route.receiver.port}`;
		await this.api.request(
			"POST",
			"/api/v2/test/output/failure",
			{ destination, enabled },
			false,
		);
	}

	async expectStored(
		label: string,
		expected: Record<string, unknown>,
	): Promise<void> {
		const route = this.required(label);
		const stored = await this.api.showObject<any>(
			await this.activeShowId(),
			"route",
			route.id,
		);
		expect(stored?.body).toMatchObject(expected);
	}

	async expectDiagnostic(
		label: string,
		expected: Record<string, unknown>,
	): Promise<void> {
		const route = this.required(label);
		const diagnostics = await this.api.request<any>(
			"GET",
			"/api/v2/diagnostics",
		);
		expect(diagnostics.output_routes).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					protocol: route.intent.protocol,
					universe: route.intent.destinationUniverse,
					...expected,
				}),
			]),
		);
	}

	async close(): Promise<void> {
		await Promise.all(
			[...this.routes.values()]
				.map((route) => route.receiver)
				.filter((receiver): receiver is DmxReceiver => receiver !== undefined)
				.map((receiver) => receiver.close()),
		);
		this.routes.clear();
	}

	private async openRoutes() {
		await this.desk.click(this.page.locator(".dock-identity"));
		const modal = this.page.locator(".show-modal");
		await this.desk.click(
			modal.getByRole("button", { name: "Enter Setup", exact: true }),
		);
		await this.desk.click(
			this.page
				.locator(".setup-window nav")
				.getByRole("button", { name: "Outputs", exact: true }),
		);
		// Outputs is a tabbed section: the Output Engine opens first and the routes live behind
		// their own tab.
		await this.desk.click(
			this.page.getByRole("tab", { name: "Routes", exact: true }),
		);
		const region = this.page.getByRole("region", { name: "Output routes" });
		await expect(region).toBeVisible();
		return region;
	}

	private required(label: string): CapturedRoute {
		const route = this.routes.get(label);
		if (!route) throw new Error(`Network output route "${label}" is absent`);
		return route;
	}

	private requiredCapture(label: string): CapturedRoute & {
		receiver: DmxReceiver;
	} {
		const route = this.required(label);
		if (!route.receiver)
			throw new Error(
				`Network output route "${label}" has no capture receiver`,
			);
		return { ...route, receiver: route.receiver };
	}

	private async objects<T>(kind: string) {
		return this.api.showObjects<T>(await this.activeShowId(), kind);
	}

	private async activeShowId() {
		const bootstrap = await this.api.request<{
			active_show: { id: string } | null;
		}>("GET", "/api/v2/bootstrap", undefined, false);
		if (!bootstrap.active_show) throw new Error("No active show");
		return bootstrap.active_show.id;
	}
}

function packetMatches(
	packet: Readonly<{
		sequence: number;
		slots: Uint8Array;
	}>,
	expected: {
		slots?: Readonly<Record<number, number>>;
		sequenceNonZero?: boolean;
	},
) {
	return (
		Object.entries(expected.slots ?? {}).every(
			([address, value]) => packet.slots[Number(address) - 1] === value,
		) &&
		(!expected.sequenceNonZero || packet.sequence !== 0)
	);
}

function normalizedIntent(
	input: NetworkRouteIntent,
): Required<Omit<NetworkRouteIntent, "capture">> {
	const deliveryMode =
		input.deliveryMode ??
		(input.protocol === "art_net" ? "broadcast" : "multicast");
	for (const [label, value] of [
		["Logical universe", input.logicalUniverse],
		["Destination universe", input.destinationUniverse],
		["Minimum slots", input.minimumSlots ?? 512],
	] as const)
		if (!Number.isSafeInteger(value) || value < 1)
			throw new Error(`${label} must be a positive integer`);
	return {
		protocol: input.protocol,
		logicalUniverse: input.logicalUniverse,
		destinationUniverse: input.destinationUniverse,
		deliveryMode,
		enabled: input.enabled ?? true,
		minimumSlots: input.minimumSlots ?? 512,
	};
}

function routeDescription(
	intent: Required<Omit<NetworkRouteIntent, "capture">>,
) {
	return `Logical ${intent.logicalUniverse} · ${protocolLabel(intent.protocol)} ${intent.destinationUniverse}`;
}

function protocolLabel(protocol: NetworkProtocol) {
	return protocol === "art_net" ? "Art-Net" : "sACN";
}

function wireProtocol(protocol: NetworkProtocol): DmxProtocol {
	return protocol === "art_net" ? "artnet" : "sacn";
}

function deliveryLabel(mode: NetworkDeliveryMode) {
	return mode[0].toUpperCase() + mode.slice(1);
}

function assertLabel(label: string) {
	if (!label.trim())
		throw new Error("Network output route label must not be empty");
}

function sixteenBitFixture(input: {
	fixtureId: string;
	fixtureNumber: number;
	profileId: string;
	modeId: string;
	headId: string;
}) {
	const channelId = crypto.randomUUID();
	const profile = {
		schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
		id: input.profileId,
		revision: 1,
		manufacturer: "ToskLight Test",
		name: "Semantic 16-bit Dimmer",
		short_name: "Semantic u16",
		fixture_type: "dimmer",
		modes: [
			{
				id: input.modeId,
				name: "16-bit",
				splits: [{ number: 1, footprint: 2 }],
				heads: [{ id: input.headId, name: "Main", master_shared: true }],
				channels: [
					{
						id: channelId,
						head_id: input.headId,
						split: 1,
						attribute: "intensity",
						resolution: "u16",
						secondary_slots: [2],
						default_raw: 0,
						highlight_raw: 65_535,
						invert: false,
					},
				],
			},
		],
	};
	return {
		fixture_id: input.fixtureId,
		fixture_number: input.fixtureNumber,
		name: "Semantic 16-bit Dimmer",
		universe: 10,
		address: 1,
		split_patches: [{ split: 1, universe: 10, address: 1 }],
		logical_heads: [],
		multipatch: [],
		definition: {
			schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
			id: input.profileId,
			revision: 1,
			manufacturer: "ToskLight Test",
			device_type: "dimmer",
			name: "Semantic 16-bit Dimmer",
			model: "Semantic 16-bit Dimmer",
			mode: "16-bit",
			footprint: 2,
			heads: [
				{
					index: 0,
					name: "Main",
					shared: true,
					parameters: [
						{
							attribute: "intensity",
							components: [
								{ offset: 0, byte_order: "msb_first" },
								{ offset: 1, byte_order: "msb_first" },
							],
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
			hazardous: false,
			safe_values: {},
			profile_id: input.profileId,
			mode_id: input.modeId,
			profile_snapshot: profile,
		},
	};
}
