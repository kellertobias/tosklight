// @bench-semantic-world

import { fixture } from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { expect } from "../apps/control-ui/e2e/bench/core/fixtures";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"DMX-001",
	"exact byte conversion agrees in logical, Art-Net, and sACN output",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.command.execute("FIXTURE 1 AT 0 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
		await t.command.execute("FIXTURE 1 AT 25 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 64 });
		await t.command.execute("FIXTURE 1 AT 50 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 128 });
		await t.command.execute("FIXTURE 1 AT 75 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 191 });
		await t.command.execute("FIXTURE 1 AT 100 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 255 });
		await t.expect.outputPacket("artnet", 1, (packet) =>
			expect(packet.slots[0]).toBe(255),
		);
		await t.expect.outputPacket("sacn", 101, (packet) =>
			expect(packet.slots[0]).toBe(255),
		);
	},
);

scenario(
	"DMX-002",
	"visible programming reaches a wire-correct ArtDMX packet",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 1 AT 25 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expect.outputPacket("artnet", 1, (packet) => {
			expect(packet.sequence).not.toBe(0);
			expect(packet.slots).toHaveLength(512);
			expect(packet.slots[0]).toBe(64);
			expect(packet.artnet).toMatchObject({
				id: "Art-Net\0",
				opcode: 0x5000,
				protocolVersion: 14,
				payloadLength: 512,
			});
		});
	},
);

scenario(
	"DMX-003",
	"visible programming reaches a wire-correct E1.31 packet",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("FIXTURE 1 AT 50 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.expect.outputPacket("sacn", 101, (packet) => {
			expect(packet.sequence).not.toBe(0);
			expect(packet.priority).toBe(100);
			expect(packet.slots).toHaveLength(512);
			expect(packet.slots[0]).toBe(128);
			expect(packet.sacn).toMatchObject({
				preambleSize: 0x10,
				rootVector: 0x00000004,
				framingVector: 0x00000002,
				propertyValueCount: 513,
				startCode: 0,
			});
		});
	},
);

scenario(
	"DMX-004",
	"remapped fan-out reaches every enabled destination and no disabled route",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.output.network.add("extra Art-Net", {
			protocol: "art_net",
			logicalUniverse: 1,
			destinationUniverse: 11,
			deliveryMode: "unicast",
			capture: true,
		});
		await t.output.network.add("disabled sACN", {
			protocol: "sacn",
			logicalUniverse: 1,
			destinationUniverse: 102,
			deliveryMode: "unicast",
			enabled: false,
			capture: true,
		});
		t.output.network.mark("extra Art-Net");
		t.output.network.mark("disabled sACN");
		await t.command.execute("FIXTURE 1 AT 25 TIME 0");
		await t.command.execute("FIXTURE 2 AT 50 TIME 0");
		await t.command.execute("FIXTURE 3 AT 75 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.output.network.expectPacket("extra Art-Net", {
			slots: { 1: 64, 2: 128, 3: 191 },
		});
		await t.output.network.expectNoPacket("disabled sACN");
		await t.expectFixtureDMX(fixture(1), { Intensity: 64 });
		await t.expectFixtureDMX(fixture(2), { Intensity: 128 });
		await t.expectFixtureDMX(fixture(3), { Intensity: 191 });
	},
);

scenario(
	"DMX-005",
	"a conflicting visible patch edit preserves both previous addresses atomically",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		const conflict = await t.patch.prepareAddressConflict();
		await t.patch.via.ui.address(conflict.candidate, "2.1");
		await t.patch.keepOldAddressAfterConflict();
		await t.patch.expect(conflict.anchor).address("2.1");
		await t.patch.expect(conflict.candidate).address("2.2");
	},
);

scenario(
	"DMX-006",
	"visible programming encodes deterministic 16-bit coarse and fine bytes",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		const fixtureNumber = await t.output.network.installSixteenBitFixture();
		await t.command.execute(`FIXTURE ${fixtureNumber} AT 50 TIME 0`);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(fixtureNumber), {
			"Intensity coarse": 128,
			"Intensity fine": 0,
		});
	},
);

scenario(
	"DMX-007",
	"one route failure is isolated and recovery sends current state",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.output.network.add("recovering Art-Net", {
			protocol: "art_net",
			logicalUniverse: 1,
			destinationUniverse: 11,
			deliveryMode: "unicast",
			capture: true,
		});
		await t.output.network.failure("recovering Art-Net", true);
		t.output.network.mark("recovering Art-Net");
		await t.command.execute("FIXTURE 1 AT 25 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.output.network.expectNoPacket("recovering Art-Net");
		await t.expect.outputPacket("artnet", 1, (packet) =>
			expect(packet.slots[0]).toBe(64),
		);
		await t.output.network.failure("recovering Art-Net", false);
		t.output.network.mark("recovering Art-Net");
		await t.clock.advanceBy("0ms");
		await t.output.network.expectPacket("recovering Art-Net", {
			slots: { 1: 64 },
			sequenceNonZero: true,
		});
	},
);

scenario(
	"DMX-009",
	"protocol-correct delivery modes persist and resolve to actual destinations",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.output.network.add("Art-Net broadcast", {
			protocol: "art_net",
			logicalUniverse: 1,
			destinationUniverse: 201,
			deliveryMode: "broadcast",
		});
		await t.output.network.add("Art-Net unicast", {
			protocol: "art_net",
			logicalUniverse: 1,
			destinationUniverse: 202,
			deliveryMode: "unicast",
			capture: true,
		});
		await t.output.network.add("sACN multicast", {
			protocol: "sacn",
			logicalUniverse: 1,
			destinationUniverse: 301,
			deliveryMode: "multicast",
		});
		await t.output.network.add("sACN unicast", {
			protocol: "sacn",
			logicalUniverse: 1,
			destinationUniverse: 302,
			deliveryMode: "unicast",
			capture: true,
		});
		t.output.network.mark("Art-Net unicast");
		t.output.network.mark("sACN unicast");
		await t.command.execute("FIXTURE 1 AT 50 TIME 0");
		await t.clock.advanceBy("0ms");
		await t.output.network.expectStored("Art-Net broadcast", {
			delivery_mode: "broadcast",
			destination: null,
		});
		await t.output.network.expectStored("sACN multicast", {
			delivery_mode: "multicast",
			destination: null,
		});
		await t.output.network.expectDiagnostic("Art-Net broadcast", {
			delivery_mode: "broadcast",
			destination: "255.255.255.255:6454",
			enabled: true,
		});
		await t.output.network.expectDiagnostic("sACN multicast", {
			delivery_mode: "multicast",
			destination: "239.255.1.45:5568",
			enabled: true,
		});
		await t.output.network.expectSamePayload("Art-Net unicast", "sACN unicast");
	},
);
