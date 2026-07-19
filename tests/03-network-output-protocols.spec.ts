import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import {
  duplicatePatchedFixtures,
  readPatchSnapshot,
  setFixtureAddressThroughApi,
  setFixtureAddressThroughSoftware,
} from "./support/operator";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { DmxReceiver } from "../apps/control-ui/e2e/bench/protocols";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import {
  fixtureIdsByNumber,
  loadCanonicalCopy,
  object,
  objects,
  pressCommand,
  putObject,
} from "./support/catalog";

interface ConversionObservation {
  percent: number;
  normalized: number | null;
  logical: number;
  artnet: number;
  sacn: number;
}

interface PatchConflictState {
  firstId: string;
  secondId: string;
  firstRevision: number;
  secondRevision: number;
  rejected: boolean;
}

interface SixteenBitFixture {
  id: string;
  fixtureNumber: number;
  address: number;
  byteOrder: "msb_first" | "lsb_first";
  invert: boolean;
  defaultValue: number;
}

interface SixteenBitState {
  fixtures: {
    msb: SixteenBitFixture;
    lsb: SixteenBitFixture;
    inverted: SixteenBitFixture;
    defaulted: SixteenBitFixture;
  };
  logicalUniverse: number;
  destinationUniverse: number;
  sunstripDestinationUniverse: number;
}

interface MinimumRouteState {
  receiver: DmxReceiver;
  emptyArtSlots?: number[];
  emptySacnSlots?: number[];
  patchedArtSlots?: number[];
  patchedSacnSlots?: number[];
  artDisabled?: boolean;
}

interface DeliveryRouteState {
  receiver: DmxReceiver;
}

test.describe("docs/testing/03-network-output-protocols.md", () => {
  pairedScenario<{ values: number[]; observations: ConversionObservation[] }>({
    id: "DMX-001",
    title: "exact byte conversion agrees in logical, Art-Net, and sACN output",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-001-${surface}`);
      return { values: [0, 25, 50, 75, 100], observations: [] };
    },
    api: async ({ api, bench }, state) => {
      const fixture = (await fixtureIdsByNumber(api))[1];
      for (const percent of state.values) {
        await api.command("programmer.set", { fixture_id: fixture, attribute: "intensity", value: percent / 100 });
        state.observations.push(await captureConversion(api, bench, fixture, percent));
      }
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      const fixture = (await fixtureIdsByNumber(api))[1];
      for (const percent of state.values) {
        await pressCommand(page, `1 AT ${percent}`);
        state.observations.push(await captureConversion(api, bench, fixture, percent));
      }
    },
    assert: async (_context, state) => {
      const expectedBytes = [0, 64, 128, 191, 255];
      expect(state.observations.map(({ percent, logical, artnet, sacn }) => ({ percent, logical, artnet, sacn }))).toEqual(
        state.values.map((percent, index) => ({
          percent,
          logical: expectedBytes[index],
          artnet: expectedBytes[index],
          sacn: expectedBytes[index],
        })),
      );
      for (const [index, observation] of state.observations.entries()) {
        expect(observation.normalized).toBeCloseTo(state.values[index] / 100, 6);
      }
    },
  });

  pairedScenario<{}>({
    id: "DMX-002",
    title: "ArtDMX headers and nonzero sequence increments are wire-correct",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-002-${surface}`);
      return {};
    },
    api: async ({ api }) => {
      await api.executeCommandLine("FIXTURE 1 AT 25 TIME 0");
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 AT 25 TIME 0");
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
      expect(packets.every((packet: any) => packet.slots.length === 512 && packet.slots[0] === 64)).toBe(true);
      expect(packets.every((packet: any) => packet.artnet?.id === "Art-Net\0"
        && packet.artnet.opcode === 0x5000
        && packet.artnet.protocolVersion === 14
        && packet.artnet.physical === 0
        && packet.artnet.payloadLength === 512)).toBe(true);
    },
  });

  pairedScenario<{}>({
    id: "DMX-003",
    title: "E1.31 fields, priority, sequence, and termination are wire-correct",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-003-${surface}`);
      return {};
    },
    api: async ({ api }) => {
      await api.executeCommandLine("FIXTURE 1 AT 50 TIME 0");
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 AT 50 TIME 0");
    },
    assert: async ({ api, bench }) => {
      bench.sacn.reset();
      for (let index = 0; index < 3; index += 1) await bench.tick(0);
      await expect.poll(() => bench.sacn.packets.length).toBe(3);
      const packets = bench.sacn.packets.slice(-3);
      expect(packets.map((packet: any) => packet.universe)).toEqual([101, 101, 101]);
      expect(packets.map((packet: any) => packet.priority)).toEqual([100, 100, 100]);
      expect(packets.map((packet: any) => packet.sequence)).toEqual([1, 2, 3]);
      expect(packets.every((packet: any) => packet.slots.length === 512 && packet.slots[0] === 128)).toBe(true);
      expect(new Set(packets.map((packet: any) => packet.sacn?.cid)).size).toBe(1);
      expect(new Set(packets.map((packet: any) => packet.sacn?.sourceName)).size).toBe(1);
      expect(packets[0].sacn?.sourceName).toBeTruthy();
      expect(packets.every((packet: any) => packet.sacn?.preambleSize === 0x10
        && packet.sacn.packetIdentifier === "ASC-E1.17\0\0\0"
        && packet.sacn.rootVector === 0x00000004
        && packet.sacn.framingVector === 0x00000002
        && packet.sacn.dmpVector === 0x02
        && packet.sacn.addressAndDataType === 0xa1
        && packet.sacn.firstPropertyAddress === 0
        && packet.sacn.addressIncrement === 1
        && packet.sacn.propertyValueCount === 513
        && packet.sacn.startCode === 0)).toBe(true);
      const route = (await objects(api, "route")).find((entry) => entry.body.protocol === "sacn")!;
      const mark = bench.sacn.mark();
      await putObject(api, "route", route.id, { ...route.body, enabled: false }, route.revision);
      await expect.poll(() => bench.sacn.packets.slice(mark).filter((packet) => packet.protocol === "sacn" && packet.universe === 101).length).toBe(3);
      const terminated = bench.sacn.packets.slice(mark).filter((packet) => packet.protocol === "sacn" && packet.universe === 101);
      expect(terminated).toHaveLength(3);
      expect(terminated.every((packet) => packet.terminated && packet.priority === 100 && packet.sequence !== 0)).toBe(true);
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
      await Promise.all([state.extraArt.close(), state.disabledSacn.close()]);
    },
  });

  pairedScenario<PatchConflictState>({
    id: "DMX-005",
    title: "a conflicting patch edit preserves both previous addresses atomically",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-005-${surface}`);
      return installPatchConflict(api);
    },
    api: async ({ api }, state) => {
      try {
        await setFixtureAddressThroughApi(api, state.secondId, "2.1");
      } catch (error) {
        expect(String(error)).toContain("returned 400");
        state.rejected = true;
      }
    },
    ui: async ({ bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: /Open show menu/ }).click();
      await page.getByRole("button", { name: "Show Patch", exact: true }).click();
      const candidate = page.locator(".patch-table tbody tr").filter({ hasText: "Atomic Candidate" });
      await setFixtureAddressThroughSoftware({
        page,
        addressCell: candidate.locator(".patch-address"),
        address: "2.1",
      });
      const conflict = page.getByRole("dialog", { name: "Patch conflict" });
      await expect(conflict).toContainText("Atomic Anchor");
      await conflict.getByRole("button", { name: "Keep old patch / mode", exact: true }).click();
      await expect(candidate.locator(".patch-address")).toHaveText("2.2");
      state.rejected = true;
    },
    assert: async ({ api }, state) => {
      expect(state.rejected).toBe(true);
      const snapshot = await readPatchSnapshot(api);
      const first = snapshot.fixtures.find((fixture) => fixture.fixture_id === state.firstId);
      const second = snapshot.fixtures.find((fixture) => fixture.fixture_id === state.secondId);
      expect(first).toMatchObject({
        fixture_revision: state.firstRevision,
        fixture_number: 901,
        split_patches: [{ universe: 2, address: 1 }],
      });
      expect(second).toMatchObject({
        fixture_revision: state.secondRevision,
        fixture_number: 902,
        split_patches: [{ universe: 2, address: 2 }],
      });
    },
  });

  pairedScenario<SixteenBitState>({
    id: "DMX-006",
    title: "16-bit metadata and virtual heads encode deterministic component bytes",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-006-${surface}`, "default-stage");
      return installSixteenBitMatrix(api, bench);
    },
    api: async ({ api }, state) => {
      await api.command("programmer.set", { fixture_id: state.fixtures.msb.id, attribute: "intensity", value: 0.5 });
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "900 AT 50");
    },
    assert: async ({ api, bench }, state) => {
      await expectFixtureBytes(bench, state.logicalUniverse, state.destinationUniverse, state.fixtures.msb.address, [128, 0], 3_000);

      const defaultEncoded = encodeSixteenBit(state.fixtures.defaulted.defaultValue, false);
      await expectFixtureBytes(
        bench,
        state.logicalUniverse,
        state.destinationUniverse,
        state.fixtures.defaulted.address,
        orderedBytes(defaultEncoded, "msb_first"),
        0,
      );

      const boundaries = [
        0,
        1 / 65_535,
        255.49 / 65_535,
        255.5 / 65_535,
        255.51 / 65_535,
        32_767 / 65_535,
        32_768 / 65_535,
        65_534 / 65_535,
        1,
      ];
      for (const fixture of [state.fixtures.msb, state.fixtures.lsb, state.fixtures.inverted]) {
        for (const value of boundaries) {
          await api.command("programmer.set", { fixture_id: fixture.id, attribute: "intensity", value });
          const encoded = encodeSixteenBit(value, fixture.invert);
          await expectFixtureBytes(
            bench,
            state.logicalUniverse,
            state.destinationUniverse,
            fixture.address,
            orderedBytes(encoded, fixture.byteOrder),
            3_000,
          );
        }
      }

      await api.command("programmer.set", { fixture_id: state.fixtures.defaulted.id, attribute: "intensity", value: 0.75 });
      await expectFixtureBytes(
        bench,
        state.logicalUniverse,
        state.destinationUniverse,
        state.fixtures.defaulted.address,
        orderedBytes(encodeSixteenBit(0.75, false), "msb_first"),
        3_000,
      );
      await api.command("programmer.release", { fixture_id: state.fixtures.defaulted.id, attribute: "intensity" });
      await expectFixtureBytes(
        bench,
        state.logicalUniverse,
        state.destinationUniverse,
        state.fixtures.defaulted.address,
        orderedBytes(defaultEncoded, "msb_first"),
        3_000,
      );

      await expectSunstripVirtualDimmers(api, bench, state.sunstripDestinationUniverse);
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
        delivery_mode: "unicast",
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
    assert: async ({ api, bench }, state, surface) => {
      const before = await api.request<any>("GET", "/api/v1/diagnostics");
      await expect.poll(async () => (await api.request<any[]>("GET", "/api/v1/audit?after=0")).at(-1)?.kind)
        .toBe(surface === "ui" ? "command_applied" : "programmer_changed");
      const auditBefore = await api.request<any[]>("GET", "/api/v1/audit?after=0");
      const auditRevision = Math.max(0, ...auditBefore.map((event) => event.revision));
      const failingErrorsBefore = routeSendErrors(before, state.destination);
      const healthyDestination = `127.0.0.1:${bench.artnet.port}`;
      const healthyErrorsBefore = routeSendErrors(before, healthyDestination);
      const healthyMark = bench.artnet.mark();
      const failedMark = state.failing.mark();
      await bench.tick(3_000);
      expect((await bench.artnet.nextAfter(healthyMark, "artnet", 1)).slots[0]).toBe(64);
      await new Promise((resolve) => setTimeout(resolve, 75));
      expect(state.failing.packets.slice(failedMark)).toHaveLength(0);
      const after = await api.request<any>("GET", "/api/v1/diagnostics");
      expect(after.output.send_errors).toBe(before.output.send_errors + 1);
      expect(routeSendErrors(after, state.destination)).toBe(failingErrorsBefore + 1);
      expect(routeSendErrors(after, healthyDestination)).toBe(healthyErrorsBefore);
      expect(await api.request<any[]>("GET", `/api/v1/audit?after=${auditRevision}`)).toEqual([]);

      await api.request("POST", "/api/v1/test/output/failure", { destination: state.destination, enabled: false }, false);
      const recoveryMark = state.failing.mark();
      await bench.tick(0);
      const recovered = await state.failing.nextAfter(recoveryMark, "artnet", 11);
      expect(recovered.slots[0]).toBe(64);
      expect(recovered.sequence).not.toBe(0);
      const recoveredDiagnostics = await api.request<any>("GET", "/api/v1/diagnostics");
      expect(routeSendErrors(recoveredDiagnostics, state.destination)).toBe(failingErrorsBefore + 1);
      await state.failing.close();
    },
  });

  pairedScenario<MinimumRouteState>({
    id: "DMX-008",
    title: "minimum universe size sends idle zeros, includes patched defaults, and disables without deletion",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-008-${surface}`);
      return { receiver: await DmxReceiver.bind() };
    },
    api: async ({ api, bench }, state) => {
      await putObject(api, "route", "dmx-008-artnet", route("art_net", 32, 32, state.receiver.port, true, 128));
      await putObject(api, "route", "dmx-008-sacn", route("sacn", 32, 132, state.receiver.port, true, 128));
      await exerciseMinimumRoute(api, bench, state, async () => {
        const artnet = await object<any>(api, "route", "dmx-008-artnet");
        await putObject(api, "route", artnet.id, { ...artnet.body, enabled: false }, artnet.revision);
      });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await page.locator(".dock-identity").click();
      await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).click();
      await page.locator(".setup-window nav").getByRole("button", { name: "Outputs", exact: true }).click();
      const routes = page.getByRole("region", { name: "Output routes" });
      await createRouteThroughUi(routes, page, "Art-Net", 32, 32, state.receiver.port, 128);
      await createRouteThroughUi(routes, page, "sACN", 32, 132, state.receiver.port, 128);
      await exerciseMinimumRoute(api, bench, state, async () => {
        const artnet = routes.locator("article").filter({ hasText: "Logical 32 → Art-Net 32" });
        await artnet.getByRole("button", { name: "Edit route", exact: true }).click();
        const editor = page.getByRole("dialog", { name: "Output route editor" });
        await editor.locator(".ui-switch-control").click();
        await editor.getByRole("button", { name: "Save route", exact: true }).click();
        await expect(artnet).toContainText("Disabled");
      });
    },
    assert: async ({ api }, state) => {
      expect(state.emptyArtSlots).toEqual(Array(128).fill(0));
      expect(state.emptySacnSlots).toEqual(Array(128).fill(0));
      expect(state.patchedArtSlots).toHaveLength(202);
      expect(state.patchedSacnSlots).toHaveLength(201);
      expect(state.patchedArtSlots?.slice(0, 199)).toEqual(Array(199).fill(0));
      expect(state.patchedSacnSlots?.slice(0, 199)).toEqual(Array(199).fill(0));
      expect(state.patchedArtSlots?.slice(199)).toEqual([102, 0, 0]);
      expect(state.patchedSacnSlots?.slice(199)).toEqual([102, 0]);
      expect(state.artDisabled).toBe(true);
      const routes = await objects<any>(api, "route");
      expect(routes.find((entry) => entry.body.logical_universe === 32 && entry.body.protocol === "art_net")?.body)
        .toMatchObject({ enabled: false, minimum_slots: 128 });
      expect(routes.find((entry) => entry.body.logical_universe === 32 && entry.body.protocol === "sacn")?.body)
        .toMatchObject({ enabled: true, minimum_slots: 128 });
      await state.receiver.close();
    },
  });

  pairedScenario<DeliveryRouteState>({
    id: "DMX-009",
    title: "protocol-correct delivery modes persist and resolve to the actual socket destinations",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `dmx-009-${surface}`);
      return { receiver: await DmxReceiver.bind() };
    },
    api: async ({ api }, state) => {
      await putObject(api, "route", "dmx-009-art-broadcast", deliveryRoute("art_net", 201, "broadcast"));
      await putObject(api, "route", "dmx-009-art-unicast", deliveryRoute("art_net", 202, "unicast", state.receiver.port));
      await putObject(api, "route", "dmx-009-sacn-multicast", deliveryRoute("sacn", 301, "multicast"));
      await putObject(api, "route", "dmx-009-sacn-unicast", deliveryRoute("sacn", 302, "unicast", state.receiver.port));
      const fixture = (await fixtureIdsByNumber(api))[1];
      await api.command("programmer.set", { fixture_id: fixture, attribute: "intensity", value: 0.5 });
    },
    ui: async ({ bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 AT 50");
      await page.locator(".dock-identity").click();
      await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).click();
      await page.locator(".setup-window nav").getByRole("button", { name: "Outputs", exact: true }).click();
      const routes = page.getByRole("region", { name: "Output routes" });
      await createDeliveryRouteThroughUi(routes, page, "Art-Net", 201, "Broadcast");
      await createDeliveryRouteThroughUi(routes, page, "Art-Net", 202, "Unicast", state.receiver.port);
      await createDeliveryRouteThroughUi(routes, page, "sACN", 301, "Multicast");
      await createDeliveryRouteThroughUi(routes, page, "sACN", 302, "Unicast", state.receiver.port);
    },
    assert: async ({ api, bench }, state) => {
      const stored = (await objects<any>(api, "route"))
        .filter((entry) => [201, 202, 301, 302].includes(entry.body.destination_universe))
        .sort((left, right) => left.body.destination_universe - right.body.destination_universe);
      expect(stored.map((entry) => [entry.body.protocol, entry.body.delivery_mode, entry.body.destination])).toEqual([
        ["art_net", "broadcast", null],
        ["art_net", "unicast", `127.0.0.1:${state.receiver.port}`],
        ["sacn", "multicast", null],
        ["sacn", "unicast", `127.0.0.1:${state.receiver.port}`],
      ]);
      const diagnostics = await api.request<any>("GET", "/api/v1/diagnostics");
      expect(diagnostics.output_routes).toEqual(expect.arrayContaining([
        expect.objectContaining({ protocol: "art_net", universe: 201, delivery_mode: "broadcast", destination: "255.255.255.255:6454", enabled: true }),
        expect.objectContaining({ protocol: "art_net", universe: 202, delivery_mode: "unicast", destination: `127.0.0.1:${state.receiver.port}`, enabled: true }),
        expect.objectContaining({ protocol: "sacn", universe: 301, delivery_mode: "multicast", destination: "239.255.1.45:5568", enabled: true }),
        expect.objectContaining({ protocol: "sacn", universe: 302, delivery_mode: "unicast", destination: `127.0.0.1:${state.receiver.port}`, enabled: true }),
      ]));
      const mark = state.receiver.mark();
      await bench.tick(3_000);
      const artnet = await state.receiver.nextAfter(mark, "artnet", 202);
      const sacn = await state.receiver.nextAfter(mark, "sacn", 302);
      expect(artnet.slots[0]).toBe(128);
      expect(sacn.slots[0]).toBe(128);
      expect(Array.from(artnet.slots)).toEqual(Array.from(sacn.slots));
      await state.receiver.close();
    },
  });
});

function routeSendErrors(diagnostics: any, destination: string): number {
  return diagnostics.route_send_errors?.find((entry: any) => entry.destination === destination)?.errors ?? 0;
}

async function captureConversion(api: ApiDriver, bench: any, fixtureId: string, percent: number): Promise<ConversionObservation> {
  const fixture = (await objects<any>(api, "patched_fixture")).find((entry) => entry.body.fixture_id === fixtureId)!;
  const artMark = bench.artnet.mark();
  const sacnMark = bench.sacn.mark();
  const tick = await bench.tick(3_000);
  const offset = fixture.body.address - 1;
  const programmer = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
  const stored = programmer.flatMap((state) => state.values).find((entry: any) => entry.fixture_id === fixtureId && entry.attribute === "intensity");
  return {
    percent,
    normalized: unwrapNormalized(stored),
    logical: tick.universes.find((entry: any) => entry.universe === fixture.body.universe)!.slots[offset],
    artnet: (await bench.artnet.nextAfter(artMark, "artnet", 1)).slots[offset],
    sacn: (await bench.sacn.nextAfter(sacnMark, "sacn", 101)).slots[offset],
  };
}

function route(protocol: "art_net" | "sacn", logical: number, destination: number, port: number, enabled: boolean, minimumSlots = 512) {
  return { protocol, logical_universe: logical, destination_universe: destination, delivery_mode: "unicast", destination: `127.0.0.1:${port}`, enabled, minimum_slots: minimumSlots };
}

function deliveryRoute(protocol: "art_net" | "sacn", destinationUniverse: number, deliveryMode: "broadcast" | "multicast" | "unicast", port?: number) {
  return {
    protocol,
    logical_universe: 1,
    destination_universe: destinationUniverse,
    delivery_mode: deliveryMode,
    destination: deliveryMode === "unicast" ? `127.0.0.1:${port}` : null,
    enabled: true,
    minimum_slots: 128,
  };
}

async function createDeliveryRouteThroughUi(routes: any, page: any, protocol: "Art-Net" | "sACN", destinationUniverse: number, mode: "Broadcast" | "Multicast" | "Unicast", port?: number) {
  await routes.getByRole("button", { name: "Add route", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "Output route editor" });
  if (protocol === "sACN") {
    await editor.getByRole("button", { name: "Art-Net", exact: true }).click();
    await page.getByRole("option", { name: "sACN", exact: true }).click();
  }
  const defaultMode = protocol === "Art-Net" ? "Broadcast" : "Multicast";
  if (mode !== defaultMode) {
    await editor.getByRole("button", { name: defaultMode, exact: true }).click();
    await page.getByRole("option", { name: mode, exact: true }).click();
  }
  await editor.getByLabel("Destination universe").fill(String(destinationUniverse));
  if (mode === "Unicast") await editor.getByLabel("Destination", { exact: true }).fill(`127.0.0.1:${port}`);
  await editor.getByRole("button", { name: "Save route", exact: true }).click();
  await expect(routes.locator("article").filter({ hasText: `Logical 1 → ${protocol} ${destinationUniverse}` })).toBeVisible();
}

async function createRouteThroughUi(routes: any, page: any, protocol: "Art-Net" | "sACN", logical: number, destination: number, port: number, minimumSlots: number) {
  await routes.getByRole("button", { name: "Add route", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "Output route editor" });
  if (protocol === "sACN") {
    await editor.getByRole("button", { name: "Art-Net", exact: true }).click();
    await page.getByRole("option", { name: "sACN", exact: true }).click();
  }
  await editor.getByRole("button", { name: protocol === "Art-Net" ? "Broadcast" : "Multicast", exact: true }).click();
  await page.getByRole("option", { name: "Unicast", exact: true }).click();
  await editor.getByLabel("Logical universe").fill(String(logical));
  await editor.getByLabel("Destination universe").fill(String(destination));
  await editor.getByLabel("Destination", { exact: true }).fill(`127.0.0.1:${port}`);
  await editor.getByLabel("Minimum universe size").fill(String(minimumSlots));
  await editor.getByRole("button", { name: "Save route", exact: true }).click();
  await expect(routes.locator("article").filter({ hasText: `Logical ${logical} → ${protocol} ${destination}` })).toBeVisible();
}

async function exerciseMinimumRoute(api: ApiDriver, bench: any, state: MinimumRouteState, disableArtNet: () => Promise<void>) {
  let mark = state.receiver.mark();
  await bench.tick(0);
  state.emptyArtSlots = Array.from((await state.receiver.nextAfter(mark, "artnet", 32)).slots);
  state.emptySacnSlots = Array.from((await state.receiver.nextAfter(mark, "sacn", 132)).slots);

  const source = (await objects<any>(api, "patched_fixture")).find((entry) => entry.body.fixture_number === 1)!;
  const firstParameter = source.body.definition.heads[0].parameters[0];
  await putObject(api, "patched_fixture", "dmx-008-defaults", {
    ...source.body,
    fixture_id: crypto.randomUUID(),
    fixture_number: 932,
    name: "Minimum Size Defaults",
    universe: 32,
    address: 200,
    definition: {
      ...source.body.definition,
      id: crypto.randomUUID(),
      revision: 1,
      footprint: 2,
      heads: [{
        ...source.body.definition.heads[0],
        parameters: [
          { ...firstParameter, default: 0.4, components: [{ ...firstParameter.components[0], offset: 0 }] },
          { ...firstParameter, attribute: "minimum_size_zero", components: [{ ...firstParameter.components[0], offset: 1 }], default: undefined },
        ],
      }],
    },
  });
  mark = state.receiver.mark();
  await bench.tick(0);
  state.patchedArtSlots = Array.from((await state.receiver.nextAfter(mark, "artnet", 32)).slots);
  state.patchedSacnSlots = Array.from((await state.receiver.nextAfter(mark, "sacn", 132)).slots);

  await disableArtNet();
  mark = state.receiver.mark();
  await bench.tick(0);
  await state.receiver.nextAfter(mark, "sacn", 132);
  await new Promise((resolve) => setTimeout(resolve, 75));
  state.artDisabled = !state.receiver.packets.slice(mark).some((packet) => packet.protocol === "artnet" && packet.universe === 32);
}

async function installPatchConflict(api: ApiDriver): Promise<PatchConflictState> {
  const source = (await readPatchSnapshot(api)).fixtures.find(
    (fixture) => fixture.fixture_number === 1,
  );
  if (!source) throw new Error("Canonical show is missing Fixture 1");
  const firstId = crypto.randomUUID();
  const secondId = crypto.randomUUID();
  const outcome = await duplicatePatchedFixtures(api, source.fixture_id, [
    { fixtureId: firstId, fixtureNumber: 901, name: "Atomic Anchor", address: "2.1" },
    { fixtureId: secondId, fixtureNumber: 902, name: "Atomic Candidate", address: "2.2" },
  ]);
  const first = outcome.fixtures.find((fixture) => fixture.fixture_id === firstId);
  const second = outcome.fixtures.find((fixture) => fixture.fixture_id === secondId);
  if (!first || !second) throw new Error("Patch did not return both conflict fixtures");
  return {
    firstId,
    secondId,
    firstRevision: first.fixture_revision,
    secondRevision: second.fixture_revision,
    rejected: false,
  };
}

async function installSixteenBitMatrix(api: any, bench: any): Promise<SixteenBitState> {
  const source = (await objects<any>(api, "patched_fixture")).find((entry) => entry.body.fixture_number === 1)!;
  const logicalUniverse = 10;
  const destinationUniverse = 210;
  const sunstripDestinationUniverse = 203;
  await putObject(api, "route", "dmx-006-16bit-route", route("art_net", logicalUniverse, destinationUniverse, bench.artnet.port, true));
  await putObject(api, "route", "dmx-006-sunstrip-route", route("art_net", 3, sunstripDestinationUniverse, bench.artnet.port, true));

  const installed: SixteenBitFixture[] = [];
  for (const [key, fixtureNumber, address, byteOrder, invert, defaultValue] of [
    ["msb", 900, 1, "msb_first", false, 0],
    ["lsb", 901, 5, "lsb_first", false, 0],
    ["inverted", 902, 9, "msb_first", true, 0],
    ["defaulted", 903, 13, "msb_first", false, 0.25],
  ] as const) {
    const definition = structuredClone(source.body.definition);
    definition.id = crypto.randomUUID();
    definition.name = `DMX-006 ${key}`;
    definition.model = `DMX-006 ${key}`;
    definition.mode = "16-bit";
    definition.footprint = 2;
    const parameter = definition.heads[0].parameters.find((candidate: any) => candidate.attribute === "intensity") ?? definition.heads[0].parameters[0];
    parameter.components = [
      { offset: 0, byte_order: byteOrder },
      { offset: 1, byte_order: byteOrder },
    ];
    parameter.default = defaultValue;
    parameter.metadata = { ...(parameter.metadata ?? {}), invert };
    const id = crypto.randomUUID();
    await putObject(api, "patched_fixture", `dmx-006-${key}`, {
      ...source.body,
      fixture_id: id,
      fixture_number: fixtureNumber,
      name: `DMX-006 ${key}`,
      definition,
      logical_heads: [],
      multipatch: [],
      universe: logicalUniverse,
      address,
    });
    installed.push({ id, fixtureNumber, address, byteOrder, invert, defaultValue });
  }
  return {
    fixtures: {
      msb: installed[0],
      lsb: installed[1],
      inverted: installed[2],
      defaulted: installed[3],
    },
    logicalUniverse,
    destinationUniverse,
    sunstripDestinationUniverse,
  };
}

async function expectFixtureBytes(
  bench: any,
  logicalUniverse: number,
  destinationUniverse: number,
  address: number,
  expected: number[],
  millis: number,
): Promise<void> {
  const mark = bench.artnet.mark();
  const tick = await bench.tick(millis);
  const logical = tick.universes.find((entry: any) => entry.universe === logicalUniverse)!.slots.slice(address - 1, address - 1 + expected.length);
  const wire = Array.from((await bench.artnet.nextAfter(mark, "artnet", destinationUniverse)).slots.slice(address - 1, address - 1 + expected.length));
  expect(logical).toEqual(expected);
  expect(wire).toEqual(expected);
}

async function expectSunstripVirtualDimmers(api: any, bench: any, destinationUniverse: number): Promise<void> {
  const sunstrip = (await objects<any>(api, "patched_fixture")).find((entry) => entry.body.fixture_number === 501)!;
  const heads = [...sunstrip.body.logical_heads].sort((left: any, right: any) => left.head_index - right.head_index);
  expect(heads).toHaveLength(10);
  for (const [index, head] of heads.entries()) {
    await api.command("programmer.set", { fixture_id: head.fixture_id, attribute: "color.red", value: 1 });
    await api.command("programmer.set", { fixture_id: head.fixture_id, attribute: "intensity", value: (index + 1) / 10 });
  }
  const mark = bench.artnet.mark();
  const tick = await bench.tick(3_000);
  const logical = tick.universes.find((entry: any) => entry.universe === 3)!.slots;
  const wire = (await bench.artnet.nextAfter(mark, "artnet", destinationUniverse)).slots;
  const start = sunstrip.body.address - 1;
  for (let index = 0; index < 10; index += 1) {
    const expected = Math.round(Math.fround(Math.fround((index + 1) / 10) * 255));
    expect(logical.slice(start + index * 3, start + index * 3 + 3)).toEqual([expected, 0, 0]);
    expect(Array.from(wire.slice(start + index * 3, start + index * 3 + 3))).toEqual([expected, 0, 0]);
  }
}

function encodeSixteenBit(value: number, invert: boolean): number {
  let normalized = Math.fround(Math.max(0, Math.min(1, value)));
  if (invert) normalized = Math.fround(1 - normalized);
  return Math.round(Math.fround(normalized * 65_535));
}

function orderedBytes(encoded: number, order: "msb_first" | "lsb_first"): number[] {
  const coarse = (encoded >> 8) & 0xff;
  const fine = encoded & 0xff;
  return order === "msb_first" ? [coarse, fine] : [fine, coarse];
}

function unwrapNormalized(entry: any): number | null {
  let current = entry;
  while (current && typeof current === "object" && "value" in current) current = current.value;
  return typeof current === "number" ? current : null;
}
