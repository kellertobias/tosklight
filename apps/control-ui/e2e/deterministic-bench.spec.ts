import { test, expect } from "./bench/fixtures";

test("Lightning Desk command line reaches exact Art-Net and sACN output", async ({ bench, desk, show }) => {
  await desk.open(bench.baseUrl);
  const artnetMark = bench.artnet.mark();
  const sacnMark = bench.sacn.mark();
  await desk.command("GROUP 1 AT 50");
  await bench.waitForGroupProgrammer("1", 0.5);
  const tick = await bench.tick(3_000);
  expect(tick.now).toBe("2020-01-01T00:00:03Z");
  expect(tick.packets_sent).toBe(2);
  const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
  const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
  expect(Array.from(artnet.slots.slice(0, show.fixtureIds.length))).toEqual(Array(12).fill(128));
  expect(Array.from(sacn.slots.slice(0, show.fixtureIds.length))).toEqual(Array(12).fill(128));
  expect(artnet.sequence).toBe(1);
  expect(sacn.sequence).toBe(1);
  expect(sacn.priority).toBe(100);
  expect(sacn.terminated).toBe(false);
});

test("REST programmer changes use the same real protocol path", async ({ bench, api, show }) => {
  const mark = bench.artnet.mark();
  await api.request("POST", "/api/v1/programmer/set", { fixture_id: show.fixtureIds[0], attribute: "intensity", value: 0.75 });
  const tick = await bench.tick(30_000);
  expect(tick.now).toBe("2020-01-01T00:00:30Z");
  const packet = await bench.artnet.nextAfter(mark, "artnet", 1);
  expect(packet.slots[0]).toBe(191);
  expect(Array.from(packet.slots.slice(1, 12))).toEqual(Array(11).fill(0));
});

test("OSC hardware commands receive feedback and drive output", async ({ bench, show }) => {
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

test("test clock rejects invalid advances", async ({ bench }) => {
  const response = await fetch(`${bench.baseUrl}/api/v1/test/clock/advance`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ millis: -1 }),
  });
  expect(response.status).toBe(400);
});
