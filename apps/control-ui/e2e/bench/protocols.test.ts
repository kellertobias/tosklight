import dgram from "node:dgram";
import { describe, expect, it } from "vitest";
import { DmxReceiver } from "./protocols";

describe("DmxReceiver packet cursors", () => {
	it("finds packets after a mark when the bounded history has truncated", async () => {
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
