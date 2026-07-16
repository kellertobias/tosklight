import fs from "node:fs/promises";
import type { Page } from "../../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";

export interface VersionedObject<T = Record<string, any>> {
  kind: string;
  id: string;
  revision: number;
  body: T;
}

export interface ProgrammerState {
  session_id?: string;
  selected: string[];
  selection_expression: any;
  values: Array<{ fixture_id: string; attribute: string; value: { value?: number } | number }>;
  group_values: Record<string, Record<string, any>>;
  command_line: string;
  [key: string]: any;
}

export async function loadCanonicalCopy(api: ApiDriver, bench: any, name: string, fixture: "compact-rig" | "default-stage" = "compact-rig") {
  await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
  bench.artnet.reset();
  bench.sacn.reset();
  await api.login();
  const bytes = await fs.readFile(new URL(`../fixtures/${fixture}.show`, import.meta.url));
  const canonical = await api.request<{ id: string }>("POST", "/api/v1/shows", {
    name: `${fixture}-canonical-${crypto.randomUUID()}`,
    data_base64: bytes.toString("base64"),
    overwrite: false,
  });
  const response = await fetch(`${api.baseUrl}/api/v1/shows/${canonical.id}/download`, {
    headers: { authorization: `Bearer ${api.session?.token}` },
  });
  expect(response.ok).toBe(true);
  const copy = await api.request<{ id: string }>("POST", "/api/v1/shows", {
    name: `${name}-${crypto.randomUUID()}`,
    data_base64: Buffer.from(await response.arrayBuffer()).toString("base64"),
    overwrite: false,
  });
  await api.request("POST", `/api/v1/shows/${copy.id}/open`, { transition: "hold_current" });
  const routes = await api.request<Array<VersionedObject>>("GET", `/api/v1/shows/${copy.id}/objects/route`, undefined, false);
  for (const route of routes) {
    const protocol = route.body.protocol;
    await api.request("PUT", `/api/v1/shows/${copy.id}/objects/route/${route.id}`, {
      ...route.body,
      destination: `127.0.0.1:${protocol === "art_net" ? bench.artnet.port : bench.sacn.port}`,
    }, true, route.revision);
  }
  await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
  bench.artnet.reset();
  bench.sacn.reset();
  await api.login();
  return copy;
}

export async function command(api: ApiDriver, value: string): Promise<void> {
  await api.command("programmer.execute", { value });
}

export async function pressCommand(page: Page, value: string, visibleValue?: string): Promise<void> {
  const commandLine = page.getByLabel("Command line");
  await page.getByRole("button", { name: "ESC", exact: true }).click();
  for (const key of commandKeys(value)) {
    await page.getByRole("button", { name: key, exact: true }).click();
  }
  if (visibleValue !== undefined) await expect(commandLine).toHaveValue(visibleValue);
  await page.getByRole("button", { name: "ENT", exact: true }).click();
}

export function commandKeys(value: string): string[] {
  return value.trim().split(/\s+/).flatMap((token) => {
    if (token === "GROUP") return ["GRP"];
    if (token === "DEGRP") return ["GRP", "GRP"];
    if (token === "THRU") return ["TRU"];
    if (token === "GO-") return ["GO −"];
    if (/^\d+$/.test(token)) return [...token];
    return [token];
  });
}

export async function activeShowId(api: ApiDriver): Promise<string> {
  const bootstrap = await api.request<{ active_show: { id: string } | null }>("GET", "/api/v1/bootstrap", undefined, false);
  expect(bootstrap.active_show).toBeTruthy();
  return bootstrap.active_show!.id;
}

export async function objects<T = Record<string, any>>(api: ApiDriver, kind: string): Promise<Array<VersionedObject<T>>> {
  const showId = await activeShowId(api);
  const result = await api.request<Array<VersionedObject<T>>>("GET", `/api/v1/shows/${showId}/objects/${kind}`, undefined, false);
  return result.sort((left, right) => left.id.localeCompare(right.id, undefined, { numeric: true }));
}

export async function object<T = Record<string, any>>(api: ApiDriver, kind: string, id: string): Promise<VersionedObject<T>> {
  const found = (await objects<T>(api, kind)).find((entry) => entry.id === id);
  expect(found).toBeDefined();
  return found!;
}

export async function putObject(api: ApiDriver, kind: string, id: string, body: unknown, revision = 0): Promise<void> {
  const showId = await activeShowId(api);
  await api.request("PUT", `/api/v1/shows/${showId}/objects/${kind}/${id}`, body, true, revision);
}

export async function deleteObject(api: ApiDriver, kind: string, id: string, revision: number): Promise<void> {
  const showId = await activeShowId(api);
  await api.request("DELETE", `/api/v1/shows/${showId}/objects/${kind}/${id}`, undefined, true, revision);
}

export async function programmer(api: ApiDriver): Promise<ProgrammerState> {
  const programmers = await api.request<ProgrammerState[]>("GET", "/api/v1/programmers", undefined, false);
  const current = programmers.find((item) => item.session_id === api.session?.session_id) ?? programmers[0];
  expect(current).toBeDefined();
  return current;
}

export async function expectProgrammer(api: ApiDriver, assertion: (state: ProgrammerState) => void | Promise<void>): Promise<void> {
  await expect.poll(async () => {
    const programmers = await api.request<ProgrammerState[]>("GET", "/api/v1/programmers", undefined, false);
    let error: unknown;
    for (const state of programmers) {
      try {
        await assertion(state);
        return true;
      } catch (candidate) {
        error = candidate;
      }
    }
    if (error) throw error;
    throw new Error("No programmer matched assertion");
  }, { timeout: 3_000 }).toBe(true);
}

export async function fixtureIdsByNumber(api: ApiDriver): Promise<Record<number, string>> {
  const fixtures = await objects(api, "patched_fixture");
  return Object.fromEntries(fixtures.map((fixture) => [fixture.body.fixture_number, fixture.body.fixture_id]));
}

export async function fixtureNumbersById(api: ApiDriver): Promise<Record<string, number>> {
  const fixtures = await objects(api, "patched_fixture");
  return Object.fromEntries(fixtures.map((fixture) => [fixture.body.fixture_id, fixture.body.fixture_number]));
}

export async function selectedNumbers(api: ApiDriver): Promise<number[]> {
  const byId = await fixtureNumbersById(api);
  return (await programmer(api)).selected.map((id) => byId[id]);
}

export async function groupNumbers(api: ApiDriver, id: string): Promise<number[]> {
  const byId = await fixtureNumbersById(api);
  return (await object(api, "group", id)).body.fixtures.map((fixture: string) => byId[fixture]);
}

export async function expectSlotsAfterTick(bench: any, millis: number, expected: number[], universe = 1): Promise<void> {
  const artnetMark = bench.artnet.mark();
  const sacnMark = bench.sacn.mark();
  const tick = await bench.tick(millis);
  const slots = tick.universes.find((entry: any) => entry.universe === universe)?.slots ?? [];
  expect(slots.slice(0, expected.length)).toEqual(expected);
  const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", universe);
  const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", universe === 1 ? 101 : universe);
  expect(Array.from(artnet.slots.slice(0, expected.length))).toEqual(expected);
  expect(Array.from(sacn.slots.slice(0, expected.length))).toEqual(expected);
}

export function normalized(value: any): number | undefined {
  if (typeof value === "number") return value;
  if (Array.isArray(value?.values)) return value.values[0]?.value ?? value.values[0];
  return value?.value;
}
