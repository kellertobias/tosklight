import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { DmxReceiver } from "../apps/control-ui/e2e/bench/protocols";
import {
  activeShowId,
  command,
  deleteObject,
  fixtureIdsByNumber,
  loadCanonicalCopy,
  object,
  objects,
  pressCommand,
  putObject,
} from "./support/catalog";

test.describe("docs/testing/03-network-output-protocols.md", () => {
  pairedScenario<{ values: number[] }>({
    id: "DMX-001",
    title: "exact byte conversion agrees in logical, Art-Net, and sACN output",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-001-${surface}`);
      return { values: [0, 25, 50, 75, 100] };
    },
    api: async ({ api }, state) => {
      const fixture = (await fixtureIdsByNumber(api))[1];
      for (const percent of state.values) {
        await api.command("programmer.set", { fixture_id: fixture, attribute: "intensity", value: percent / 100 });
      }
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      const expectedBytes = [0, 64, 128, 191, 255];
      for (const [index, percent] of state.values.entries()) {
        await pressCommand(page, `1 AT ${percent}`);
        await expectFrame(api, bench, 1, expectedBytes[index]);
      }
    },
    assert: async ({ api, bench }, state, surface) => {
      if (surface === "ui") return;
      await expectFrame(api, bench, 1, 255);
      const fixture = (await fixtureIdsByNumber(api))[1];
      for (const [value, expected] of [[0.25, 64], [0.5, 128], [0.75, 191]] as const) {
        await api.command("programmer.set", { fixture_id: fixture, attribute: "intensity", value });
        await expectFrame(api, bench, 1, expected);
      }
      expect(state.values).toEqual([0, 25, 50, 75, 100]);
    },
  });

  pairedScenario<{}>({
    id: "DMX-002",
    title: "ArtDMX headers and nonzero sequence increments are wire-correct",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-002-${surface}`);
      return {};
    },
    api: async () => {},
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 AT 25");
    },
    assert: async ({ bench }) => {
      bench.artnet.reset();
      for (let index = 0; index < 3; index += 1) await bench.tick(0);
      await expect.poll(() => bench.artnet.packets.length).toBe(3);
      const packets = bench.artnet.packets.slice(-3);
      expect(packets.map((packet: any) => packet.protocol)).toEqual(["artnet", "artnet", "artnet"]);
      expect(packets.map((packet: any) => packet.universe)).toEqual([1, 1, 1]);
      expect(packets.map((packet: any) => packet.sequence)).toEqual([1, 2, 3]);
      expect(packets.every((packet: any) => packet.sequence !== 0)).toBe(true);
    },
  });

  pairedScenario<{}>({
    id: "DMX-003",
    title: "E1.31 fields, priority, sequence, and termination are wire-correct",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-003-${surface}`);
      return {};
    },
    api: async () => {},
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 AT 50");
    },
    assert: async ({ api, bench }) => {
      bench.sacn.reset();
      for (let index = 0; index < 3; index += 1) await bench.tick(0);
      await expect.poll(() => bench.sacn.packets.length).toBe(3);
      const packets = bench.sacn.packets.slice(-3);
      expect(packets.map((packet: any) => packet.universe)).toEqual([101, 101, 101]);
      expect(packets.map((packet: any) => packet.priority)).toEqual([100, 100, 100]);
      expect(packets.map((packet: any) => packet.sequence)).toEqual([1, 2, 3]);
      const route = (await objects(api, "route")).find((entry) => entry.body.protocol === "sacn")!;
      const mark = bench.sacn.mark();
      await putObject(api, "route", route.id, { ...route.body, enabled: false }, route.revision);
      const terminated = await bench.sacn.nextAfter(mark, "sacn", 101);
      expect(terminated.terminated).toBe(true);
    },
  });

  pairedScenario<{ extraArt: DmxReceiver; disabledSacn: DmxReceiver }>({
    id: "DMX-004",
    title: "remapped fan-out reaches every enabled destination and no disabled route",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-004-${surface}`);
      const extraArt = await DmxReceiver.bind();
      const disabledSacn = await DmxReceiver.bind();
      await putObject(api, "route", "artnet-11", route("art_net", 1, 11, extraArt.port, true));
      await putObject(api, "route", "sacn-102", route("sacn", 1, 102, disabledSacn.port, false));
      return { extraArt, disabledSacn };
    },
    api: async ({ api }) => {
      const fixtures = await fixtureIdsByNumber(api);
      for (const [number, value] of [[1, 0.25], [2, 0.5], [3, 0.75]] as const) {
        await api.command("programmer.set", { fixture_id: fixtures[number], attribute: "intensity", value });
      }
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      for (const [number, percent] of [[1, 25], [2, 50], [3, 75]] as const) await pressCommand(page, `${number} AT ${percent}`);
    },
    assert: async ({ bench }, state) => {
      const mark = state.extraArt.mark();
      const disabledMark = state.disabledSacn.mark();
      await bench.tick(3_000);
      const primary = await bench.artnet.nextAfter(0, "artnet", 1);
      const extra = await state.extraArt.nextAfter(mark, "artnet", 11);
      expect(Array.from(primary.slots.slice(0, 3))).toEqual([64, 128, 191]);
      expect(Array.from(extra.slots.slice(0, 3))).toEqual([64, 128, 191]);
      await new Promise((resolve) => setTimeout(resolve, 75));
      expect(state.disabledSacn.packets.slice(disabledMark)).toHaveLength(0);
      state.extraArt.close();
      state.disabledSacn.close();
    },
  });

  pairedScenario<{}>({
    id: "DMX-005",
    title: "patch overlap and universe boundary validation is atomic",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-005-${surface}`);
      return {};
    },
    api: async ({ api }) => validatePatchAtomicity(api),
    ui: async ({ api, bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: /Open show menu/ }).click();
      await page.getByRole("button", { name: "Show Patch", exact: true }).click();
      await expect(page.getByText("Dimmer 1", { exact: true }).first()).toBeVisible();
      await validatePatchAtomicity(api);
    },
    assert: async ({ api }) => {
      const fixtures = await objects(api, "patched_fixture");
      expect(fixtures.some((entry) => entry.body.fixture_number === 901)).toBe(true);
      expect(fixtures.some((entry) => entry.body.fixture_number === 902)).toBe(false);
    },
  });

  pairedScenario<{}>({
    id: "DMX-006",
    title: "16-bit MSB component order encodes coarse and fine bytes",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-006-${surface}`);
      await installSixteenBitFixture(api);
      return {};
    },
    api: async ({ api }) => {
      const fixture = (await fixtureIdsByNumber(api))[900];
      await api.command("programmer.set", { fixture_id: fixture, attribute: "intensity", value: 0.5 });
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "900 AT 50");
    },
    assert: async ({ bench }) => {
      const tick = await bench.tick(3_000);
      const slots = tick.universes.find((entry: any) => entry.universe === 1)!.slots;
      expect(slots.slice(99, 101)).toEqual([128, 0]);
    },
  });

  pairedScenario<{ failing: DmxReceiver; destination: string }>({
    id: "DMX-007",
    title: "one route failure is isolated and recovery sends current state",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-007-${surface}`);
      const failing = await DmxReceiver.bind();
      const destination = `127.0.0.1:${failing.port}`;
      await putObject(api, "route", "dmx-007-failing", {
        protocol: "art_net",
        logical_universe: 1,
        destination_universe: 11,
        destination,
        enabled: true,
      });
      await api.request("POST", "/api/v1/test/output/failure", { destination, enabled: true }, false);
      return { failing, destination };
    },
    api: async ({ api }) => {
      const fixture = (await fixtureIdsByNumber(api))[1];
      await api.command("programmer.set", { fixture_id: fixture, attribute: "intensity", value: 0.25 });
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 AT 25");
    },
    assert: async ({ api, bench }, state) => {
      const before = await api.request<any>("GET", "/api/v1/diagnostics");
      const healthyMark = bench.artnet.mark();
      const failedMark = state.failing.mark();
      await bench.tick(3_000);
      expect((await bench.artnet.nextAfter(healthyMark, "artnet", 1)).slots[0]).toBe(64);
      await new Promise((resolve) => setTimeout(resolve, 75));
      expect(state.failing.packets.slice(failedMark)).toHaveLength(0);
      const after = await api.request<any>("GET", "/api/v1/diagnostics");
      expect(after.output.send_errors).toBe(before.output.send_errors + 1);

      await api.request("POST", "/api/v1/test/output/failure", { destination: state.destination, enabled: false }, false);
      const recoveryMark = state.failing.mark();
      await bench.tick(0);
      const recovered = await state.failing.nextAfter(recoveryMark, "artnet", 11);
      expect(recovered.slots[0]).toBe(64);
      expect(recovered.sequence).not.toBe(0);
      state.failing.close();
    },
  });
});

async function expectFrame(api: any, bench: any, fixtureNumber: number, expected: number) {
  const fixture = (await objects(api, "patched_fixture")).find((entry) => entry.body.fixture_number === fixtureNumber)!;
  const artMark = bench.artnet.mark();
  const sacnMark = bench.sacn.mark();
  const tick = await bench.tick(3_000);
  const offset = fixture.body.address - 1;
  expect(tick.universes.find((entry: any) => entry.universe === fixture.body.universe)!.slots[offset]).toBe(expected);
  expect((await bench.artnet.nextAfter(artMark, "artnet", 1)).slots[offset]).toBe(expected);
  expect((await bench.sacn.nextAfter(sacnMark, "sacn", 101)).slots[offset]).toBe(expected);
}

function route(protocol: "art_net" | "sacn", logical: number, destination: number, port: number, enabled: boolean) {
  return { protocol, logical_universe: logical, destination_universe: destination, destination: `127.0.0.1:${port}`, enabled };
}

async function validatePatchAtomicity(api: any) {
  const source = (await objects(api, "patched_fixture")).find((entry) => entry.body.fixture_number === 1)!;
  await putObject(api, "patched_fixture", "dmx-005-valid", {
    ...source.body,
    fixture_id: crypto.randomUUID(),
    fixture_number: 901,
    name: "Boundary Dimmer",
    universe: 2,
    address: 1,
  });
  let error = "";
  try {
    await putObject(api, "patched_fixture", "dmx-005-overlap", {
      ...source.body,
      fixture_id: crypto.randomUUID(),
      fixture_number: 902,
      name: "Overlapping Dimmer",
      universe: 2,
      address: 1,
    });
  } catch (candidate) {
    error = String(candidate);
  }
  expect(error).toContain("returned 400");
}

async function installSixteenBitFixture(api: any) {
  const source = (await objects(api, "patched_fixture")).find((entry) => entry.body.fixture_number === 1)!;
  const definition = structuredClone(source.body.definition);
  definition.id = crypto.randomUUID();
  definition.footprint = 2;
  definition.heads[0].parameters[0].components = [
    { offset: 0, byte_order: "msb_first" },
    { offset: 1, byte_order: "msb_first" },
  ];
  await putObject(api, "patched_fixture", "dmx-006-16bit", {
    ...source.body,
    fixture_id: crypto.randomUUID(),
    fixture_number: 900,
    name: "16-bit Dimmer",
    definition,
    universe: 1,
    address: 100,
  });
}
