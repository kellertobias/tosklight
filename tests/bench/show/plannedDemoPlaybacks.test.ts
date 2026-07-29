import { describe, expect, it } from "vitest";
import { PLANNED_DEMO_FIXTURES } from "../../support/plannedDemoManifest";
import { installPlannedDemoPlaybacks } from "../../support/plannedDemoPlaybacks";

describe("Plan 76 initial Playback topology", () => {
  it("creates Group Masters, individual ACLs, Hazer, Start, and the Speed D chase", async () => {
    const writes: Array<{ kind: string; id: string; body: any }> = [];
    const api = {
      seedShowObject: async (_showId: string, kind: string, id: string, body: any) => {
        writes.push({ kind, id, body });
      },
    } as any;
    const fixtures = PLANNED_DEMO_FIXTURES.map((fixture) => ({
      fixture_id: `fixture-${fixture.number}`,
      fixture_number: fixture.number,
      logical_heads: [],
    }));
    const result = await installPlannedDemoPlaybacks(api, "show", fixtures);
    expect(result.cuelists).toHaveLength(7);
    expect(result.playbacks).toHaveLength(13);
    expect(result.playbacks.filter((item) => item.target.type === "group")).toHaveLength(6);
    const chase = result.cuelists.find((item) => item.name === "ACL Chase")!;
    expect(chase).toMatchObject({
      mode: "chaser",
      wrap_mode: "reset",
      looped: true,
      speed_group: "D",
    });
    expect(chase.cues).toHaveLength(4);
    for (const [index, cue] of chase.cues.entries()) {
      const values = cue.changes.map((change: any) => change.value.value);
      expect(values.filter((value: number) => value === 1)).toHaveLength(1);
      expect(values[index]).toBe(1);
    }
    expect(writes.find((write) => write.kind === "playback_page")?.body.name).toBe("Busking");
  });
});
