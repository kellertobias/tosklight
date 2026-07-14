import { expect, test } from "./bench/fixtures";
import { pairedScenario } from "./bench/pairedScenario";

interface OutputMarks { artnet: number; sacn: number }

function pairedGroupOutput(id: string, percent: number, expectedByte: number) {
  pairedScenario<OutputMarks>({
    id,
    title: `group programming at ${percent}% reaches identical application and wire output`,
    arrange: ({ bench }) => ({ artnet: bench.artnet.mark(), sacn: bench.sacn.mark() }),
    api: async ({ api }, _marks) => {
      await api.command("programmer.group.set", { group_id: "1", attribute: "intensity", value: percent / 100 });
    },
    ui: async ({ bench, desk }, _marks) => {
      await desk.open(bench.baseUrl);
      await desk.command(`GROUP 1 AT ${percent}`);
    },
    assert: async ({ bench, show }, marks) => {
      await bench.waitForGroupProgrammer("1", percent / 100);
      const tick = await bench.tick(3_000);
      expect(tick.now).toBe("2020-01-01T00:00:03Z");
      expect(tick.packets_sent).toBe(2);
      const artnet = await bench.artnet.nextAfter(marks.artnet, "artnet", 1);
      const sacn = await bench.sacn.nextAfter(marks.sacn, "sacn", 101);
      expect(Array.from(artnet.slots.slice(0, show.fixtureIds.length))).toEqual(Array(12).fill(expectedByte));
      expect(Array.from(sacn.slots.slice(0, show.fixtureIds.length))).toEqual(Array(12).fill(expectedByte));
      expect(artnet.sequence).toBe(1);
      expect(sacn.sequence).toBe(1);
      expect(sacn.priority).toBe(100);
      expect(sacn.terminated).toBe(false);
    },
  });
}

pairedGroupOutput("DIM-002", 50, 128);
pairedGroupOutput("OSC-002", 25, 64);

test("OSC-002 @osc › hardware command matches the paired API and UI contract", async ({ bench, show }) => {
  const hardware = await bench.osc();
  try {
    const alias = show.session.desk.osc_alias;
    const feedbackMark = hardware.mark();
    await hardware.subscribe(`e2e-${crypto.randomUUID()}`, alias);
    await hardware.expectAfter(feedbackMark, `/light/${alias}/feedback/page`);
    await hardware.send(`/light/${alias}/programmer/grp`, [true]);
    await hardware.send(`/light/${alias}/programmer/digit-1`, [true]);
    await hardware.send(`/light/${alias}/programmer/at`, [true]);
    await hardware.send(`/light/${alias}/programmer/digit-2`, [true]);
    await hardware.send(`/light/${alias}/programmer/digit-5`, [true]);
    await hardware.send(`/light/${alias}/programmer/enter`, [true]);
    await bench.waitForGroupProgrammer("1", 0.25);
    const mark = bench.artnet.mark();
    const nextFeedback = hardware.mark();
    await bench.tick(3_000);
    const packet = await bench.artnet.nextAfter(mark, "artnet", 1);
    expect(Array.from(packet.slots.slice(0, 12))).toEqual(Array(12).fill(64));
    await hardware.expectAfter(nextFeedback, `/light/${alias}/feedback/page`);
  } finally {
    hardware.close();
  }
});

test("BENCH-001 @bench › test clock rejects invalid advances", async ({ bench }) => {
  const response = await fetch(`${bench.baseUrl}/api/v1/test/clock/advance`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ millis: -1 }),
  });
  expect(response.status).toBe(400);
});
