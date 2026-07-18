import { spawn, type ChildProcess } from "node:child_process";
import dgram from "node:dgram";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ApiDriver, type Session } from "./api";
import { DmxReceiver, OscHardware } from "./protocols";
import artifactResolver from "../../../../tools/artifact-paths.cjs";

const { artifactPaths } = artifactResolver;

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const SERVER = path.join(artifactPaths.cargo, "debug", process.platform === "win32" ? "light-server.exe" : "light-server");

export interface TestShow { id: string; fixtureIds: string[]; session: Session }

export class LightBench {
  private process?: ChildProcess;
  private readonly log: string[] = [];
  private readonly oscHardware: OscHardware[] = [];
  private lastVirtualNow = "2020-01-01T00:00:00Z";
  dataDir = "";
  baseUrl = "";
  oscPort = 0;
  artnet!: DmxReceiver;
  sacn!: DmxReceiver;

  async start(workerIndex: number): Promise<void> {
    this.dataDir = await fs.mkdtemp(path.join(os.tmpdir(), `light-e2e-${workerIndex}-`));
    const httpPort = await freeTcpPort();
    this.oscPort = await freeUdpPort();
    this.artnet = await DmxReceiver.bind();
    this.sacn = await DmxReceiver.bind();
    this.baseUrl = `http://127.0.0.1:${httpPort}`;
    await this.spawnServer();
  }

  async restart(): Promise<void> {
    await this.stopServerAbruptly();
    await this.spawnServer();
  }

  serverPid(): number | undefined { return this.process?.pid; }

  async stopServerGracefully(token: string): Promise<number> {
    const process = this.process;
    if (!process?.pid || process.exitCode !== null) throw new Error("light-server is not running");
    const pid = process.pid;
    const response = await fetch(`${this.baseUrl}/api/v1/shutdown`, {
      method: "POST",
      headers: { authorization: `Bearer ${token}` },
      signal: AbortSignal.timeout(5_000),
    });
    if (!response.ok) throw new Error(`Server shutdown failed: ${response.status} ${await response.text()}`);
    await this.waitForServerExit(process, 5_000);
    this.process = undefined;
    return pid;
  }

  async stopServerAbruptly(): Promise<number | undefined> {
    const process = this.process;
    const pid = process?.pid;
    if (process && process.exitCode === null) {
      process.kill("SIGKILL");
      await this.waitForServerExit(process, 2_000);
    }
    this.process = undefined;
    return pid;
  }

  async startServer(): Promise<number> {
    if (this.process && this.process.exitCode === null) throw new Error("light-server is already running");
    await this.spawnServer();
    if (!this.process?.pid) throw new Error("light-server started without a PID");
    return this.process.pid;
  }

  private async spawnServer(): Promise<void> {
    const httpPort = Number(new URL(this.baseUrl).port);
    this.process = spawn(SERVER, [
      "--data-dir", this.dataDir,
      "--fixture-package-dir", path.join(ROOT, "assets", "fixture-library"),
      "--bind", `127.0.0.1:${httpPort}`,
      "--test-bench",
      "--osc-bind", `127.0.0.1:${this.oscPort}`,
      "--output-bind-ip", "127.0.0.1",
    ], { cwd: ROOT, stdio: ["ignore", "pipe", "pipe"] });
    const collect = (chunk: Buffer) => {
      this.log.push(chunk.toString());
      if (this.log.length > 500) this.log.shift();
    };
    this.process.stdout?.on("data", collect);
    this.process.stderr?.on("data", collect);
    await this.waitUntilReady();
  }

  async createTwelveDimmerShow(name = `E2E-${crypto.randomUUID()}`): Promise<TestShow> {
    const api = new ApiDriver(this.baseUrl);
    await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
    const session = await api.login();
    this.lastVirtualNow = "2020-01-01T00:00:00Z";
    this.artnet.reset();
    this.sacn.reset();
    for (const hardware of this.oscHardware) hardware.resetTrace();
    const show = await api.request<{ id: string }>("POST", "/api/v1/shows", { name, data_base64: null, overwrite: false });
    const fixtureIds = Array.from({ length: 12 }, () => crypto.randomUUID());
    await Promise.all(fixtureIds.map((fixtureId, index) => api.request("PUT", `/api/v1/shows/${show.id}/objects/patched_fixture/${fixtureId}`, dimmer(fixtureId, index + 1), true, 0)));
    await api.request("PUT", `/api/v1/shows/${show.id}/objects/group/1`, {
      id: "1", name: "All Dimmers", fixtures: fixtureIds, derived_from: null, frozen_from: null,
      programming: {}, master: 1, playback_fader: 1,
    }, true, 0);
    await api.request("PUT", `/api/v1/shows/${show.id}/objects/group/2`, {
      id: "2", name: "Odd Dimmers", fixtures: fixtureIds.filter((_, index) => index % 2 === 0),
      derived_from: null, frozen_from: null, programming: {}, master: 1, playback_fader: 2,
    }, true, 0);
    await api.request("PUT", `/api/v1/shows/${show.id}/objects/group/3`, {
      id: "3", name: "Front Dimmers", fixtures: fixtureIds.slice(0, 4),
      derived_from: null, frozen_from: null, programming: {}, master: 1, playback_fader: 3,
    }, true, 0);
    await api.request("PUT", `/api/v1/shows/${show.id}/objects/route/artnet`, {
      protocol: "art_net", logical_universe: 1, destination_universe: 1,
      destination: `127.0.0.1:${this.artnet.port}`, enabled: true, minimum_slots: 512,
    }, true, 0);
    await api.request("PUT", `/api/v1/shows/${show.id}/objects/route/sacn`, {
      protocol: "sacn", logical_universe: 1, destination_universe: 101,
      destination: `127.0.0.1:${this.sacn.port}`, enabled: true, minimum_slots: 512,
    }, true, 0);
    await api.request("POST", `/api/v1/shows/${show.id}/open`, { transition: "hold_current" });
    return { id: show.id, fixtureIds, session };
  }

  async tick(millis = 0): Promise<{ now: string; packets_sent: number; universes: Array<{ universe: number; slots: number[] }> }> {
    const response = await fetch(`${this.baseUrl}/api/v1/test/clock/advance`, {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ millis }), signal: AbortSignal.timeout(5_000),
    });
    if (!response.ok) throw new Error(`Test clock advance failed: ${await response.text()}`);
    const result = await response.json() as { now: string; packets_sent: number; universes: Array<{ universe: number; slots: number[] }> };
    this.lastVirtualNow = result.now;
    return result;
  }

  async waitForDmx(address: number, expected: number, timeout = 2_000): Promise<void> {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const response = await fetch(`${this.baseUrl}/api/v1/dmx`);
      if (response.ok) {
        const snapshot = await response.json() as { universes: Array<{ universe: number; slots: number[] }> };
        if (snapshot.universes.find((universe) => universe.universe === 1)?.slots[address - 1] === expected) return;
      }
      await new Promise<void>((resolve) => setTimeout(resolve, 5));
    }
    throw new Error(`Timed out waiting for logical DMX U1.${address} = ${expected}`);
  }

  async waitForGroupProgrammer(groupId: string, expected: number, timeout = 2_000): Promise<void> {
    const deadline = Date.now() + timeout;
    let last: unknown = null;
    while (Date.now() < deadline) {
      const response = await fetch(`${this.baseUrl}/api/v1/programmers`, { signal: AbortSignal.timeout(1_000) });
      if (response.ok) {
        const programmers = await response.json() as Array<{ group_values: Record<string, Record<string, { value: { value?: number } | number }>> }>;
        last = programmers;
        const stored = programmers.find((programmer) => programmer.group_values[groupId])?.group_values[groupId]?.intensity?.value;
        const normalized = typeof stored === "number" ? stored : stored?.value;
        if (normalized === expected) return;
      }
      await new Promise<void>((resolve) => setTimeout(resolve, 5));
    }
    throw new Error(`Timed out waiting for programmer group ${groupId} = ${expected}; programmers: ${JSON.stringify(last)}`);
  }

  async osc(): Promise<OscHardware> {
    const hardware = await OscHardware.connect(this.oscPort);
    this.oscHardware.push(hardware);
    return hardware;
  }
  visualOscSummary(): string {
    return this.oscHardware
      .flatMap((hardware) => hardware.trace)
      .sort((left, right) => left.recordedAt - right.recordedAt)
      .slice(-2)
      .map((message) => {
        const values = message.arguments.map((value) => typeof value === "number" ? Number(value.toFixed(3)) : value).join(", ");
        return `${message.direction === "sent" ? "TX" : "RX"} ${message.address}${values ? ` ${values}` : ""}`;
      })
      .join(" · ");
  }
  recentLog(): string { return this.log.slice(-100).join(""); }

  async failureArtifacts(token: string): Promise<Record<string, string>> {
    let audit: unknown = [];
    try {
      const response = await fetch(`${this.baseUrl}/api/v1/audit?after=0`, {
        headers: { authorization: `Bearer ${token}` },
        signal: AbortSignal.timeout(2_000),
      });
      if (response.ok) audit = (await response.json() as unknown[]).slice(-100);
    } catch { /* server failure is represented by its log */ }
    const packets = [...this.artnet.packets, ...this.sacn.packets].slice(-40).map((packet) => ({
      protocol: packet.protocol, universe: packet.universe, sequence: packet.sequence,
      priority: packet.priority, terminated: packet.terminated, slots: Array.from(packet.slots.slice(0, 64)),
    }));
    return {
      "light-server.log": this.recentLog(),
      "virtual-time.json": JSON.stringify({ now: this.lastVirtualNow }, null, 2),
      "audit-tail.json": JSON.stringify(audit, null, 2),
      "osc-tail.json": JSON.stringify(this.oscHardware.flatMap((hardware) => hardware.messages).slice(-200), null, 2),
      "osc-trace.json": JSON.stringify(this.oscHardware.flatMap((hardware) => hardware.trace).sort((left, right) => left.recordedAt - right.recordedAt).slice(-200), null, 2),
      "dmx-packets.json": JSON.stringify(packets, null, 2),
    };
  }

  async stop(): Promise<void> {
    const udpClosures: Promise<void>[] = this.oscHardware.map((hardware) => hardware.close());
    this.oscHardware.length = 0;
    if (this.artnet) udpClosures.push(this.artnet.close());
    if (this.sacn) udpClosures.push(this.sacn.close());
    await Promise.all(udpClosures);
    await this.stopServerAbruptly();
    if (this.dataDir) await fs.rm(this.dataDir, { recursive: true, force: true });
  }

  private async waitForServerExit(process: ChildProcess, timeout: number): Promise<void> {
    if (process.exitCode !== null) return;
    await new Promise<void>((resolve, reject) => {
      const onExit = () => {
        clearTimeout(timer);
        process.off("exit", onExit);
        resolve();
      };
      const timer = setTimeout(() => {
        process.off("exit", onExit);
        reject(new Error(`Timed out waiting for light-server PID ${process.pid} to exit`));
      }, timeout);
      timer.unref();
      process.once("exit", onExit);
      // Avoid missing an exit that lands between the initial check and listener setup.
      if (process.exitCode !== null) onExit();
    });
  }

  private async waitUntilReady(): Promise<void> {
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
      if (this.process?.exitCode !== null) throw new Error(`light-server exited during startup:\n${this.recentLog()}`);
      try {
        const response = await fetch(`${this.baseUrl}/api/v1/readiness`);
        if (response.ok) return;
      } catch { /* startup */ }
      await new Promise<void>((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`Timed out starting light-server:\n${this.recentLog()}`);
  }
}

function dimmer(fixtureId: string, number: number) {
  return {
    fixture_id: fixtureId, fixture_number: number, name: `Dimmer ${number}`,
    definition: {
      schema_version: 1, id: crypto.randomUUID(), revision: 1, manufacturer: "E2E",
      device_type: "dimmer", name: "Generic Dimmer", model: "Generic Dimmer", mode: "1ch", footprint: 1,
      heads: [{ index: 0, name: "Main", shared: true, parameters: [{ attribute: "intensity", components: [{ offset: 0, byte_order: "msb_first" }], default: 0, virtual_dimmer: false, metadata: { physical_min: 0, physical_max: 1, unit: null, invert: false, wrap: false, curve: "linear" }, capabilities: [] }] }],
      color_calibration: null, physical: {}, model_asset: null, icon_asset: null, hazardous: false,
      direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {},
    },
    universe: 1, address: number, layer_id: "default", direct_control: null,
    location: { x: number * 500, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 }, logical_heads: [], multipatch: [],
  };
}

async function freeTcpPort(): Promise<number> {
  const server = net.createServer();
  await new Promise<void>((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = (server.address() as net.AddressInfo).port;
  await new Promise<void>((resolve) => server.close(() => resolve()));
  return port;
}

async function freeUdpPort(): Promise<number> {
  const socket = dgram.createSocket("udp4");
  await new Promise<void>((resolve, reject) => { socket.once("error", reject); socket.bind(0, "127.0.0.1", resolve); });
  const port = (socket.address() as dgram.AddressInfo).port;
  await new Promise<void>((resolve) => socket.close(() => resolve()));
  return port;
}
