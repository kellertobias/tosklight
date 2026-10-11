import dgram, { type AddressInfo } from "node:dgram";
import { describe, expect, it } from "vitest";
import { DmxReceiver, OscHardware, encodeOscMessage } from "./protocols";

const hasUsableUdpBinding = await DmxReceiver.canBind();

describe("DmxReceiver packet cursors", () => {
	it.runIf(hasUsableUdpBinding)("finds packets after a mark when the bounded history has truncated", async () => {
		const receiver = await DmxReceiver.bind();
		const sender = dgram.createSocket("udp4");
		try {
			for (let index = 0; index < 205; index += 1) {
				await send(
					sender,
					artDmxPacket(index % 256, index % 256),
					receiver.port,
				);
			}
			await waitFor(
				() =>
					receiver.packets.length === 200 &&
					receiver.packets.at(-1)?.slots[0] === 204,
			);

			const mark = receiver.mark();
			await send(sender, artDmxPacket(206, 205), receiver.port);

			const packet = await receiver.nextAfter(mark, "artnet", 1, 500);
			expect(receiver.packetsAfter(mark)).toEqual([packet]);
			expect(packet.sequence).toBe(206);
			expect(packet.slots[0]).toBe(205);
			expect(receiver.packets).toHaveLength(200);
		} finally {
			sender.close();
			await receiver.close();
		}
	});
});

function artDmxPacket(sequence: number, value: number): Buffer {
	const packet = Buffer.alloc(19);
	packet.write("Art-Net\0", 0, "binary");
	packet.writeUInt16LE(0x5000, 8);
	packet.writeUInt16BE(14, 10);
	packet[12] = sequence;
	packet.writeUInt16LE(1, 14);
	packet.writeUInt16BE(1, 16);
	packet[18] = value;
	return packet;
}

function send(
	socket: dgram.Socket,
	packet: Buffer,
	port: number,
): Promise<void> {
	return new Promise((resolve, reject) => {
		socket.send(packet, port, "127.0.0.1", (error) =>
			error ? reject(error) : resolve(),
		);
	});
}

async function waitFor(predicate: () => boolean): Promise<void> {
	const deadline = Date.now() + 2_000;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((resolve) => setTimeout(resolve, 5));
	}
	throw new Error("Timed out waiting for local DMX packets");
}

async function subscriptionFixture() {
	const server = dgram.createSocket("udp4");
	await new Promise<void>((resolve) => server.bind(0, "127.0.0.1", resolve));
	const hardware = await OscHardware.connect((server.address() as AddressInfo).port);
	return { server, hardware, close: async () => { server.close(); await hardware.close(); } };
}

describe("OscHardware subscription acknowledgement", () => {
	it.runIf(hasUsableUdpBinding)("rejects old page feedback and a different desk's acknowledgement", async () => {
		const fixture = await subscriptionFixture();
		fixture.hardware.messages.push({ address: "/light/desk/feedback/page", arguments: [1] });
		fixture.server.on("message", () => {
			void send(fixture.server, encodeOscMessage("/light/other/feedback/page", [99]), fixture.hardware.feedbackPort);
		});
		try {
			await expect(fixture.hardware.subscribe("unacknowledged", "desk")).rejects.toThrow("Timed out subscribing");
			expect(fixture.hardware.trace.filter(message => message.direction === "sent")).toHaveLength(5);
		} finally {
			await fixture.close();
		}
	});

	it.runIf(hasUsableUdpBinding)("accepts page feedback received during the final retry wait", async () => {
		const fixture = await subscriptionFixture();
		let requests = 0;
		let reply: ReturnType<typeof setTimeout> | undefined;
		fixture.server.on("message", () => {
			requests += 1;
			if (requests === 5) reply = setTimeout(() => {
				void send(fixture.server, encodeOscMessage("/light/desk/feedback/page", [1]), fixture.hardware.feedbackPort);
			}, 10);
		});
		try {
			await expect(fixture.hardware.subscribe("last-wait", "desk")).resolves.toBeUndefined();
			expect(requests).toBe(5);
		} finally {
			clearTimeout(reply);
			await fixture.close();
		}
	});

	it.runIf(hasUsableUdpBinding)("waits for delayed matching feedback without accepting another desk's page", async () => {
		const fixture = await subscriptionFixture();
		let reply: ReturnType<typeof setTimeout> | undefined;
		let scheduled = false;
		fixture.server.on("message", () => {
			if (scheduled) return;
			scheduled = true;
			void send(fixture.server, encodeOscMessage("/light/other/feedback/page", [99]), fixture.hardware.feedbackPort);
			reply = setTimeout(() => {
				void send(fixture.server, encodeOscMessage("/light/desk/feedback/page", [2]), fixture.hardware.feedbackPort);
			}, 200);
		});
		try {
			await expect(fixture.hardware.subscribe("delayed", "desk")).resolves.toBeUndefined();
			expect(fixture.hardware.messages.some(message => message.address === "/light/desk/feedback/page")).toBe(true);
		} finally {
			clearTimeout(reply);
			await fixture.close();
		}
	});
});
