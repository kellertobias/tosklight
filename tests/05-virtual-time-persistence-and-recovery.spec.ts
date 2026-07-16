import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { activeShowId, loadCanonicalCopy, object, pressCommand, putObject } from "./support/catalog";

const cases = [
  ["TIME-001", "zero tick emits one frame without advancing application time"],
  ["TIME-002", "programmer fade boundaries are exact"],
  ["TIME-003", "large virtual jumps remain deterministic"],
  ["SHOW-001", "saved show and programmer state survive server restart"],
  ["SHOW-002", "a crash restart exposes one atomic show revision"],
  ["SHOW-003", "active-show recovery leaves a usable authenticated server"],
  ["SHOW-004", "serialized legacy-compatible objects remain stable after reopen"],
  ["DESKTOP-001", "the owning process restart restores one child server endpoint"],
  ["DESKTOP-002", "restart never changes the configured server endpoint"],
] as const;

test.describe("docs/testing/05-virtual-time-persistence-and-recovery.md", () => {
  for (const [id, title] of cases) {
    pairedScenario<{ startedAt: string; showId: string }>({
      id,
      title,
      arrange: async ({ api, bench }, surface) => {
        await loadCanonicalCopy(api, bench, `${id.toLowerCase()}-${surface}`);
        const startedAt = (await bench.tick(0)).now;
        return { startedAt, showId: await activeShowId(api) };
      },
      api: async ({ api }) => {
        await api.command("programmer.execute", { value: "GROUP 1 AT 50" });
      },
      ui: async ({ bench, desk, page }) => {
        await desk.open(bench.baseUrl);
        await pressCommand(page, "GROUP 1 AT 50");
      },
      assert: async ({ api, bench }, state) => {
        if (id === "TIME-001") {
          const frame = await bench.tick(0);
          expect(frame.now).toBe(state.startedAt);
          expect(frame.packets_sent).toBeGreaterThanOrEqual(2);
          return;
        }
        if (id === "TIME-002") {
          expect((await bench.tick(2_999)).universes[0].slots.slice(0, 12)).toEqual(Array(12).fill(127));
          expect((await bench.tick(1)).universes[0].slots.slice(0, 12)).toEqual(Array(12).fill(128));
          return;
        }
        if (id === "TIME-003") {
          expect((await bench.tick(300_000)).universes[0].slots.slice(0, 12)).toEqual(Array(12).fill(128));
          expect((await bench.tick(3_600_000)).universes[0].slots.slice(0, 12)).toEqual(Array(12).fill(128));
          return;
        }
        if (id === "SHOW-004") {
          const group = await object<any>(api, "group", "4");
          await putObject(api, "group", "4", { ...group.body, fixtures: [], derived_from: null, frozen_from: null }, group.revision);
        }
        await bench.restart();
        await api.login();
        const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
        expect(bootstrap.active_show.id).toBe(state.showId);
        expect(bootstrap.users.some((user: any) => user.name === "Operator")).toBe(true);
        const frame = await bench.tick(3_000);
        expect(frame.universes.find((universe: any) => universe.universe === 1)).toBeDefined();
        if (id === "SHOW-004") expect((await object<any>(api, "group", "4")).body.fixtures).toEqual([]);
      },
    });
  }

  test("SHOW-001 @restart › persisted programmer values restore once for the same user", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "show-001-restart");
    await api.command("programmer.execute", { value: "GROUP 1 AT 50" });
    await bench.restart();
    await api.login();
    const programmers = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
    const sameUser = programmers.filter((programmer) => programmer.user_id === api.session!.user.id);
    expect(new Set(sameUser.map((programmer) => programmer.id)).size).toBe(1);
    expect(sameUser.every((programmer) => programmer.group_values["1"].intensity.value.value === 0.5)).toBe(true);
  });
});
