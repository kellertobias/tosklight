import dgram, { type Socket } from "node:dgram";

export type DmxProtocol = "artnet" | "sacn";
export interface DmxPacket {
  protocol: DmxProtocol;
  universe: number;
  sequence: number;
  slots: Uint8Array;
  priority?: number;
  terminated?: boolean;
  receivedAt: number;
}

export class DmxReceiver {
  readonly packets: DmxPacket[] = [];
  private constructor(private readonly socket: Socket, readonly port: number) {
    socket.on("message", (message) => {
      const packet = parseDmxPacket(message);
      if (packet) {
        this.packets.push(packet);
        if (this.packets.length > 200) this.packets.shift();
      }
    });
  }

  static async bind(): Promise<DmxReceiver> {
    const socket = dgram.createSocket("udp4");
    await new Promise<void>((resolve, reject) => {
      socket.once("error", reject);
      socket.bind(0, "127.0.0.1", () => {
        socket.off("error", reject);
        resolve();
      });
    });
    return new DmxReceiver(socket, (socket.address() as dgram.AddressInfo).port);
  }

  mark(): number { return this.packets.length; }

  reset(): void { this.packets.length = 0; }

  async nextAfter(mark: number, protocol: DmxProtocol, universe: number, timeout = 2_000): Promise<DmxPacket> {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const packet = this.packets.slice(mark).find((candidate) => candidate.protocol === protocol && candidate.universe === universe);
      if (packet) return packet;
      await new Promise<void>((resolve) => setTimeout(resolve, 5));
    }
    throw new Error(`Timed out waiting for ${protocol} universe ${universe}; recent packets: ${JSON.stringify(this.packets.slice(-5).map(summarizePacket))}`);
  }

  close(): void { this.socket.close(); }
}

function parseDmxPacket(packet: Buffer): DmxPacket | null {
  if (packet.length >= 18 && packet.subarray(0, 8).toString("binary") === "Art-Net\0" && packet.readUInt16LE(8) === 0x5000) {
    const length = packet.readUInt16BE(16);
    return { protocol: "artnet", universe: packet.readUInt16LE(14), sequence: packet[12], slots: packet.subarray(18, 18 + length), receivedAt: Date.now() };
  }
  if (packet.length >= 126 && packet.subarray(4, 16).toString("binary") === "ASC-E1.17\0\0\0") {
    return { protocol: "sacn", universe: packet.readUInt16BE(113), sequence: packet[111], priority: packet[108], terminated: (packet[112] & 0x40) !== 0, slots: packet.subarray(126), receivedAt: Date.now() };
  }
  return null;
}

function summarizePacket(packet: DmxPacket) {
  return { protocol: packet.protocol, universe: packet.universe, sequence: packet.sequence, slots: Array.from(packet.slots.slice(0, 16)) };
}

export type OscArgument = string | number | boolean;
export interface OscMessage { address: string; arguments: OscArgument[] }

export class OscHardware {
  readonly messages: OscMessage[] = [];
  private constructor(
    private readonly command: Socket,
    private readonly feedback: Socket,
    private readonly serverPort: number,
    readonly feedbackPort: number,
  ) {
    feedback.on("message", (packet) => {
      const message = parseOscMessage(packet);
      if (message) {
        this.messages.push(message);
        if (this.messages.length > 5_000) this.messages.shift();
      }
    });
  }

  static async connect(serverPort: number): Promise<OscHardware> {
    const command = await bindUdp();
    const feedback = await bindUdp();
    return new OscHardware(command, feedback, serverPort, (feedback.address() as dgram.AddressInfo).port);
  }

  mark(): number { return this.messages.length; }

  async subscribe(clientId: string, deskAlias = "main"): Promise<void> {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      await this.send("/light/subscribe", [clientId, deskAlias, this.feedbackPort]);
      if (this.messages.some((message) => message.address === `/light/${deskAlias}/feedback/page`)) return;
      await new Promise<void>((resolve) => setTimeout(resolve, 25));
    }
  }

  async send(address: string, arguments_: OscArgument[] = []): Promise<void> {
    const packet = encodeOscMessage(address, arguments_);
    await new Promise<void>((resolve, reject) => this.command.send(packet, this.serverPort, "127.0.0.1", (error) => error ? reject(error) : resolve()));
  }

  async expectAfter(mark: number, address: string, timeout = 2_000): Promise<OscMessage> {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const message = this.messages.slice(mark).find((candidate) => candidate.address === address);
      if (message) return message;
      await new Promise<void>((resolve) => setTimeout(resolve, 5));
    }
    throw new Error(`Timed out waiting for OSC ${address}; recent messages: ${JSON.stringify(this.messages.slice(-10))}`);
  }

  close(): void { this.command.close(); this.feedback.close(); }
}

async function bindUdp(): Promise<Socket> {
  const socket = dgram.createSocket("udp4");
  await new Promise<void>((resolve, reject) => {
    socket.once("error", reject);
    socket.bind(0, "127.0.0.1", () => { socket.off("error", reject); resolve(); });
  });
  return socket;
}

export function encodeOscMessage(address: string, arguments_: OscArgument[]): Buffer {
  const tags = `,${arguments_.map((value) => typeof value === "string" ? "s" : typeof value === "boolean" ? (value ? "T" : "F") : Number.isInteger(value) ? "i" : "f").join("")}`;
  const parts = [oscString(address), oscString(tags)];
  for (const value of arguments_) {
    if (typeof value === "string") parts.push(oscString(value));
    else if (typeof value === "number") {
      const data = Buffer.alloc(4);
      if (Number.isInteger(value)) data.writeInt32BE(value); else data.writeFloatBE(value);
      parts.push(data);
    }
  }
  return Buffer.concat(parts);
}

function parseOscMessage(packet: Buffer): OscMessage | null {
  try {
    const address = readOscString(packet, 0);
    const tags = readOscString(packet, address.next);
    let offset = tags.next;
    const arguments_: OscArgument[] = [];
    for (const tag of tags.value.slice(1)) {
      if (tag === "s") { const value = readOscString(packet, offset); arguments_.push(value.value); offset = value.next; }
      else if (tag === "i") { arguments_.push(packet.readInt32BE(offset)); offset += 4; }
      else if (tag === "f") { arguments_.push(packet.readFloatBE(offset)); offset += 4; }
      else if (tag === "T" || tag === "F") arguments_.push(tag === "T");
      else return null;
    }
    return { address: address.value, arguments: arguments_ };
  } catch { return null; }
}

function oscString(value: string): Buffer {
  const bytes = Buffer.from(`${value}\0`);
  const result = Buffer.alloc(Math.ceil(bytes.length / 4) * 4);
  bytes.copy(result);
  return result;
}

function readOscString(packet: Buffer, offset: number): { value: string; next: number } {
  const end = packet.indexOf(0, offset);
  if (end < 0) throw new Error("unterminated OSC string");
  return { value: packet.subarray(offset, end).toString("utf8"), next: Math.ceil((end + 1) / 4) * 4 };
}
