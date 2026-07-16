import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import type { ApiDriver, Session } from "../apps/control-ui/e2e/bench/api";
import {
  activeShowId,
  command,
  expectProgrammer,
  fixtureIdsByNumber,
  groupNumbers,
  loadCanonicalCopy,
  object,
  objects,
  pressCommand,
  putObject,
} from "./support/catalog";

test.describe("docs/testing/04-osc-api-and-cross-surface.md", () => {
  pairedScenario<{}>({
    id: "OSC-001",
    title: "manual ticks produce deterministic current state without periodic mutation",
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `osc-001-${surface}`); return {}; },
    api: async () => {},
    ui: async ({ bench, desk }) => { await desk.open(bench.baseUrl); },
    assert: async ({ api, bench }) => {
      const before = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
      await bench.tick(0);
      const after = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
      expect(after.active_show.id).toBe(before.active_show.id);
      expect(after.hardware_connected).toBe(false);
    },
  });

  registerGroupOutputPair("OSC-002", 25, 64, "hardware-equivalent command reaches shared programmer and output");

  pairedScenario<{}>({
    id: "OSC-003",
    title: "independent desks retain isolated command and programmer state",
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `osc-003-${surface}`); return {}; },
    api: async ({ api }) => {
      const second = await createSession(api, crypto.randomUUID());
      const original = api.session;
      await api.command("programmer.command_line", { value: "GROUP 1 +" });
      api.session = second;
      await api.command("programmer.command_line", { value: "GROUP 2 +" });
      api.session = original;
    },
    ui: async ({ api, bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      for (const key of ["GRP", "1", "+"]) await page.getByRole("button", { name: key, exact: true }).click();
      const second = await createSession(api, crypto.randomUUID());
      const original = api.session;
      api.session = second;
      await api.command("programmer.command_line", { value: "GROUP 2 +" });
      api.session = original;
    },
    assert: async ({ api }) => {
      const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
      const commands = states.map((state) => normalizeCommand(state.command_line)).filter(Boolean).sort();
      expect(commands).toEqual(["GROUP 1 +", "GROUP 2 +"]);
      expect(states.every((state) => state.values.length === 0 && Object.keys(state.group_values).length === 0)).toBe(true);
    },
  });

  pairedScenario<{}>({
    id: "OSC-004",
    title: "invalid input is rejected without programmer or output mutation",
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `osc-004-${surface}`); return {}; },
    api: async ({ api }) => {
      await expect(api.command("not.a.command", {})).rejects.toThrow("unknown command");
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      for (const key of ["GRP", "9", "9", "9", "AT", "5", "0", "ENT"]) {
        await page.getByRole("button", { name: key, exact: true }).click();
      }
      await expect(page.getByLabel("Command line")).toHaveClass(/error/);
    },
    assert: async ({ api, bench }) => {
      const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
      expect(states.every((state) => state.values.length === 0 && Object.keys(state.group_values).length === 0)).toBe(true);
      expect((await bench.tick(0)).universes.find((entry: any) => entry.universe === 1)!.slots.slice(0, 12)).toEqual(Array(12).fill(0));
    },
  });

  registerGroupOutputPair("OSC-005", 50, 128, "UI and attached hardware share one authoritative desk programmer");

  pairedScenario<{ cueListId: string }>({
    id: "OSC-006",
    title: "current-page and explicit-page playback addresses resolve the intended Cuelist",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `osc-006-${surface}`);
      return { cueListId: await installPlayback(api) };
    },
    api: async ({ api }) => { await api.request("POST", "/api/v1/cuelists/1/go", {}); },
    ui: async ({ api, bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      await page.locator(".playback-fader-bank").getByRole("button", { name: "GO", exact: true }).first().click();
      await expect.poll(async () => activeCueIndex(api)).toBe(0);
    },
    assert: async ({ api }) => { expect(await activeCueIndex(api)).toBe(0); },
  });

  pairedScenario<{ originalRevision: number }>({
    id: "API-001",
    title: "authentication and stale revision conflicts are atomic",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `api-001-${surface}`);
      return { originalRevision: (await object(api, "group", "3")).revision };
    },
    api: async ({ api }, state) => { await exerciseRevisionConflict(api, state.originalRevision); },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: "BUILT-INS" }).click();
      await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
      await exerciseRevisionConflict(api, state.originalRevision);
      await expect(page.locator(".group-card").filter({ hasText: "Front Dimmers updated" })).toBeVisible();
    },
    assert: async ({ api }) => {
      expect((await object(api, "group", "3")).body.name).toBe("Front Dimmers updated");
    },
  });

  pairedScenario<{ auditBefore: number }>({
    id: "API-002",
    title: "Group CRUD produces ordered audit and object events",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `api-002-${surface}`);
      return { auditBefore: (await audit(api)).length };
    },
    api: async ({ api }) => {
      const fixtures = await fixtureIdsByNumber(api);
      await api.command("selection.set", { fixtures: [fixtures[1], fixtures[2]] });
      await command(api, "RECORD GROUP 90");
      await api.command("selection.set", { fixtures: [fixtures[3]] });
      await command(api, "RECORD + GROUP 90");
      await command(api, "DELETE GROUP 90");
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await pressCommand(page, "1 + 2");
      await pressCommand(page, "RECORD GROUP 90");
      await pressCommand(page, "3");
      await pressCommand(page, "RECORD + GROUP 90");
      await pressCommand(page, "DELETE GROUP 90");
    },
    assert: async ({ api }, state) => {
      expect((await objects(api, "group")).some((entry) => entry.id === "90")).toBe(false);
      const events = (await audit(api)).slice(state.auditBefore);
      expect(events.filter((event: any) => /group|show_object|command/.test(event.kind)).length).toBeGreaterThanOrEqual(3);
    },
  });

  registerGroupOutputPair("CROSS-001", 50, 128, "equivalent group value agrees across command surfaces");

  pairedScenario<{}>({
    id: "CROSS-002",
    title: "browser and API converge live after an external Group mutation",
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `cross-002-${surface}`); return {}; },
    api: async ({ api }) => { await appendFixtureFive(api); },
    ui: async ({ api, bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: "BUILT-INS" }).click();
      await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
      await appendFixtureFive(api);
      await expect(page.locator(".group-card").filter({ hasText: "Front Dimmers" })).toContainText("5 fixtures");
    },
    assert: async ({ api }) => { expect(await groupNumbers(api, "3")).toEqual([1, 2, 3, 4, 5]); },
  });

  test("OSC-001 @osc › subscription and one tick return a complete deterministic feedback cycle", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-001-wire");
    const hardware = await bench.osc();
    const alias = api.session!.desk.osc_alias;
    try {
      await hardware.subscribe(`osc-001-${crypto.randomUUID()}`, alias);
      const mark = hardware.mark();
      await bench.tick(0);
      for (const address of [
        `/light/${alias}/feedback/page`,
        `/light/${alias}/feedback/command-line`,
        `/light/${alias}/feedback/programmer/group`,
        `/light/${alias}/feedback/page-playback/1/fader`,
        `/light/${alias}/feedback/page-playback/1/button/1`,
        `/light/${alias}/feedback/speed-group/1`,
      ]) await hardware.expectAfter(mark, address);
      const quietMark = hardware.mark();
      await new Promise((resolve) => setTimeout(resolve, 75));
      expect(hardware.messages.slice(quietMark)).toHaveLength(0);
    } finally { hardware.close(); }
  });

  test("OSC-002 @osc › hardware keypad command reaches feedback and both network outputs", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-002-wire");
    const hardware = await bench.osc();
    const alias = api.session!.desk.osc_alias;
    try {
      await hardware.subscribe(`osc-002-${crypto.randomUUID()}`, alias);
      for (const action of ["grp", "digit-1", "at", "digit-2", "digit-5", "enter"]) await hardware.send(`/light/${alias}/programmer/${action}`, [true]);
      await expectProgrammer(api, (state) => expect(state.group_values["1"]?.intensity).toBeDefined());
      const art = bench.artnet.mark(); const sacn = bench.sacn.mark();
      await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(64));
      expect(Array.from((await bench.sacn.nextAfter(sacn, "sacn", 101)).slots.slice(0, 12))).toEqual(Array(12).fill(64));
    } finally { hardware.close(); }
  });

  test("OSC-003 @osc › subscribers on separate desk aliases stay isolated and unsubscribe is reference-counted", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-003-wire");
    const second = await createSession(api, crypto.randomUUID());
    const a = await bench.osc(); const b = await bench.osc();
    try {
      await a.subscribe("osc-003-a", api.session!.desk.osc_alias);
      await b.subscribe("osc-003-b", second.desk.osc_alias);
      const aMark = a.mark(); const bMark = b.mark();
      await a.send(`/light/${api.session!.desk.osc_alias}/programmer/digit-1`, [true]);
      await a.expectAfter(aMark, `/light/${api.session!.desk.osc_alias}/feedback/command-line`);
      expect(b.messages.slice(bMark).some((message) => message.address.includes(api.session!.desk.osc_alias))).toBe(false);
      await a.send("/light/unsubscribe", ["osc-003-a"]);
      await new Promise((resolve) => setTimeout(resolve, 100));
      const disconnected = a.mark();
      await bench.tick(0);
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(a.messages.slice(disconnected)).toHaveLength(0);
      expect((await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
      await b.send("/light/unsubscribe", ["osc-003-b"]);
      expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
    } finally { a.close(); b.close(); }
  });

  test("OSC-004 @osc › malformed and unsubscribed input leaves authoritative state unchanged", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-004-wire");
    const hardware = await bench.osc();
    try {
      await hardware.send("/light/subscribe", ["bad", "missing-desk", "wrong-port"]);
      await hardware.send("/light/main/programmer/unknown", [true]);
      await hardware.send("/light/main/programmer/digit-1", [true]);
      await bench.tick(0);
      const states = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
      expect(states.every((state) => !state.command_line && state.values.length === 0)).toBe(true);
    } finally { hardware.close(); }
  });

  test("OSC-005 @osc › UI and hardware interleave tokens on one desk command line", async ({ api, bench, desk, page }) => {
    await loadCanonicalCopy(api, bench, "osc-005-mixed");
    await desk.open(bench.baseUrl);
    const uiSession = await browserSession(page);
    const hardware = await bench.osc();
    try {
      for (const key of ["GRP", "1", "+"]) await page.getByRole("button", { name: key, exact: true }).click();
      await hardware.subscribe("osc-005-mixed", uiSession.desk.osc_alias);
      await hardware.send(`/light/${uiSession.desk.osc_alias}/programmer/digit-2`, [true]);
      await expect(page.getByLabel("Command line")).toHaveValue("G1 + G2");
      await hardware.send("/light/unsubscribe", ["osc-005-mixed"]);
      await expect(page.getByRole("button", { name: "AT", exact: true })).toBeVisible();
      for (const key of ["AT", "5", "0", "ENT"]) await page.getByRole("button", { name: key, exact: true }).click();
      const art = bench.artnet.mark(); await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(128));
    } finally { hardware.close(); }
  });

  test("OSC-006 @osc › current-page and explicit-page addresses execute the same assigned Cuelist", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "osc-006-wire");
    await installPlayback(api);
    const hardware = await bench.osc(); const alias = api.session!.desk.osc_alias;
    try {
      await hardware.subscribe("osc-006-wire", alias);
      await hardware.send(`/light/${alias}/page-playback/1/button/1`, [true]);
      await expect.poll(async () => activeCueIndex(api)).toBe(0);
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await hardware.send("/light/playback/1/1/button/1", [true]);
      await expect.poll(async () => activeCueIndex(api)).toBe(0);
      await hardware.send("/light/playback/1/1/button/1", [false]);
    } finally { hardware.close(); }
  });

  test("CROSS-001 @osc › OSC matches UI and API normalized Group output", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "cross-001-osc");
    const hardware = await bench.osc(); const alias = api.session!.desk.osc_alias;
    try {
      await hardware.subscribe("cross-001-osc", alias);
      for (const action of ["grp", "digit-1", "at", "digit-5", "digit-0", "enter"]) await hardware.send(`/light/${alias}/programmer/${action}`, [true]);
      const art = bench.artnet.mark(); await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(art, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(128));
    } finally { hardware.close(); }
  });
});

function registerGroupOutputPair(id: string, percent: number, byte: number, title: string) {
  pairedScenario<{}>({
    id, title,
    arrange: async ({ api, bench }, surface) => { await loadCanonicalCopy(api, bench, `${id.toLowerCase()}-${surface}`); return {}; },
    api: async ({ api }) => { await api.command("programmer.group.set", { group_id: "1", attribute: "intensity", value: percent / 100 }); },
    ui: async ({ bench, desk, page }) => { await desk.open(bench.baseUrl); await pressCommand(page, `GROUP 1 AT ${percent}`); },
    assert: async ({ bench }) => {
      const mark = bench.artnet.mark(); await bench.tick(3_000);
      expect(Array.from((await bench.artnet.nextAfter(mark, "artnet", 1)).slots.slice(0, 12))).toEqual(Array(12).fill(byte));
    },
  });
}

async function createSession(api: ApiDriver, clientId: string): Promise<Session> {
  return api.request<Session>("POST", "/api/v1/sessions", { username: "Operator", client_id: clientId }, false);
}

async function exerciseRevisionConflict(api: ApiDriver, originalRevision: number) {
  const url = `${api.baseUrl}/api/v1/shows/${await activeShowId(api)}/objects/group/3`;
  const unauthenticated = await fetch(url);
  expect(unauthenticated.status).toBe(401);
  const invalid = await fetch(url, { headers: { authorization: "Bearer invalid" } });
  expect(invalid.status).toBe(401);
  const group = await object(api, "group", "3");
  await putObject(api, "group", "3", { ...group.body, name: "Front Dimmers updated" }, originalRevision);
  const stale = await fetch(url, {
    method: "PUT",
    headers: { authorization: `Bearer ${api.session!.token}`, "content-type": "application/json", "if-match": String(originalRevision) },
    body: JSON.stringify({ ...group.body, name: "stale name" }),
  });
  expect(stale.status).toBe(409);
}

async function audit(api: ApiDriver): Promise<any[]> {
  return api.request<any[]>("GET", "/api/v1/audit?after=0");
}

async function appendFixtureFive(api: ApiDriver) {
  const group = await object(api, "group", "3");
  const fixture = (await fixtureIdsByNumber(api))[5];
  await putObject(api, "group", "3", { ...group.body, fixtures: [...group.body.fixtures, fixture] }, group.revision);
}

async function installPlayback(api: ApiDriver): Promise<string> {
  const fixture = (await fixtureIdsByNumber(api))[1];
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, {
    id: cueListId, name: "OSC Sequence", priority: 0, mode: "sequence", looped: false,
    chaser_step_millis: 1000, speed_group: null,
    cues: [{
      number: 1, name: "First", changes: [{ fixture_id: fixture, attribute: "intensity", value: { kind: "normalized", value: 0.5 }, fade_millis: 0 }],
      group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [],
    }],
  });
  await putObject(api, "playback", "1", {
    number: 1, name: "OSC Sequence", target: { type: "cue_list", cue_list_id: cueListId },
    buttons: ["go", "go_minus", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0,
  });
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1 } });
  return cueListId;
}

async function activeCueIndex(api: ApiDriver): Promise<number | null> {
  const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
  return playbacks.active[0]?.cue_index ?? null;
}

async function browserSession(page: Page): Promise<Session> {
  return page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session") ?? "null"));
}

function normalizeCommand(value: string): string {
  return value.trim().replace(/^G(\d+)/, "GROUP $1");
}
