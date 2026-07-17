import fs from "node:fs/promises";
import path from "node:path";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import artifactResolver from "../tools/artifact-paths.cjs";

const { artifactPaths } = artifactResolver;

const VIDEO = path.join(artifactPaths.visual, "light-visual-inspection.webm");

test("records the complete desk with OSC and DMX observers", async ({ api, bench, desk, page }, testInfo) => {
  test.setTimeout(90_000);
  await desk.open(bench.baseUrl);
  await installRecordingOverlay(page);
  const hardware = await bench.osc();
  const browserSession = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session") ?? "null"));
  const alias = browserSession.desk.osc_alias as string;
  const sent: string[] = [];
  const video = page.video();
  try {
    await stage(page, "READY", "Application, OSC hardware, logical DMX, Art-Net, and sACN are connected.", sent, hardware.messages, await dmxState(api, bench));
    await pause(page, 1_200);

    await hardware.subscribe("visual-inspection", alias);
    sent.push(`→ /light/subscribe  visual-inspection, ${alias}, ${hardware.feedbackPort}`);
    await stage(page, "OSC SUBSCRIBED", `The external controller is now attached to desk alias “${alias}”.`, sent, hardware.messages, await dmxState(api, bench));
    await pause(page, 1_000);

    for (const [key, address, expected] of [
      ["GRP", "group", "GROUP"], ["1", "digit-1", "G1"], ["AT", "at", "G1 AT"], ["2", "digit-2", "G1 AT 2"], ["5", "digit-5", "G1 AT 25"],
    ] as const) {
      const oscAddress = `/light/${alias}/programmer/${address}`;
      await hardware.send(oscAddress, [true]);
      sent.push(`→ ${oscAddress}  true`);
      await expect(page.getByLabel("Command line")).toHaveValue(expected);
      await stage(page, `PHYSICAL ${key}`, `OSC pressed ${key}; the application command line changes exactly like its UI button.`, sent, hardware.messages, await dmxState(api, bench));
      await pause(page, 700);
    }
    const enter = `/light/${alias}/programmer/enter`;
    await hardware.send(enter, [true]);
    sent.push(`→ ${enter}  true`);
    await bench.waitForGroupProgrammer("1", 0.25);
    await stage(page, "PHYSICAL ENTER", "The completed OSC command has landed in the user’s shared programmer.", sent, hardware.messages, await dmxState(api, bench));
    await pause(page, 900);

    const artMark = bench.artnet.mark();
    const sacnMark = bench.sacn.mark();
    await bench.tick(3_000);
    await bench.artnet.nextAfter(artMark, "artnet", 1);
    await bench.sacn.nextAfter(sacnMark, "sacn", 101);
    await stage(page, "OUTPUT SETTLED", "The fade boundary is reached. Logical DMX and both real UDP protocols agree at 25%.", sent, hardware.messages, await dmxState(api, bench));
    await pause(page, 1_600);

    await page.getByRole("button", { name: "BUILT-INS" }).click();
    await page.locator(".dock-entry").filter({ hasText: "DMX" }).click();
    await stage(page, "APPLICATION DMX VIEW", "The application’s own DMX renderer is visible while the external packet observers remain alongside it.", sent, hardware.messages, await dmxState(api, bench));
    await pause(page, 2_500);

    await hardware.send("/light/unsubscribe", ["visual-inspection"]);
    sent.push("→ /light/unsubscribe  visual-inspection");
    await stage(page, "COMPLETE", "Visual walkthrough complete. Assertions are still checked by the normal test catalog.", sent, hardware.messages, await dmxState(api, bench));
    await pause(page, 1_500);
  } finally {
    await hardware.close();
    await fs.mkdir(path.dirname(VIDEO), { recursive: true });
    await page.context().close();
    if (video) {
      await video.saveAs(VIDEO);
      await testInfo.attach("visual-inspection-video", { path: VIDEO, contentType: "video/webm" });
    }
  }
});

async function installRecordingOverlay(page: import("../apps/control-ui/node_modules/@playwright/test/index.js").Page) {
  await page.addStyleTag({ content: `
    #root{position:fixed!important;left:0;top:0;width:1440px;height:1080px;transform:scale(.75);transform-origin:top left}
    #light-recording-overlay{position:fixed;z-index:999999;inset:0 0 0 1080px;background:#080b10;color:#e9f0f7;font:18px/1.35 ui-monospace,SFMono-Regular,Menlo,monospace;display:grid;grid-template-rows:auto auto 1fr 1fr;gap:14px;padding:22px;box-sizing:border-box;border-left:3px solid #1bd6ec}
    #light-recording-overlay h1{font:700 28px/1.1 system-ui;margin:0;color:#1bd6ec}#light-recording-overlay h2{font:700 17px system-ui;margin:0 0 8px;color:#ffc44d}
    #light-recording-overlay .record-stage{font:700 22px system-ui;color:#fff}.record-copy{color:#aeb8bf;min-height:48px}.record-panel{background:#111720;border:1px solid #334154;border-radius:10px;padding:14px;min-height:0;overflow:hidden}.record-feed{white-space:pre-wrap;font-size:15px;color:#b8d9f5}.record-dmx{display:grid;grid-template-columns:repeat(6,1fr);gap:8px}.record-slot{display:grid;grid-template-rows:1fr auto;height:110px;text-align:center}.record-bar{align-self:end;background:linear-gradient(#1bd6ec,#315b82);min-height:2px}.record-protocol{display:grid;grid-template-columns:1fr 1fr;gap:10px;margin-top:12px}.record-protocol b{color:#62d78a}.record-command{background:#091f29;border:1px solid #1bd6ec;padding:9px;border-radius:6px;color:#fff;margin-top:8px}
  ` });
  await page.evaluate(() => {
    const overlay = document.createElement("aside"); overlay.id = "light-recording-overlay";
    overlay.innerHTML = `<h1>LIGHT TEST · VISUAL INSPECTION</h1><div><div class="record-stage" id="record-stage"></div><div class="record-copy" id="record-copy"></div><div class="record-command" id="record-command">Command: —</div></div><section class="record-panel"><h2>EXTERNAL OSC · SENT AND FEEDBACK</h2><div class="record-feed" id="record-osc"></div></section><section class="record-panel"><h2>ACTUAL DMX OUTPUT</h2><div class="record-dmx" id="record-dmx"></div><div class="record-protocol" id="record-protocol"></div></section>`;
    document.body.append(overlay);
  });
}

async function stage(page: import("../apps/control-ui/node_modules/@playwright/test/index.js").Page, title: string, copy: string, sent: string[], feedback: Array<{ address: string; arguments: unknown[] }>, dmx: DmxState) {
  const command = await page.getByLabel("Command line").inputValue().catch(() => "—");
  await page.evaluate(({ title, copy, sent, feedback, dmx, command }) => {
    document.querySelector("#record-stage")!.textContent = title; document.querySelector("#record-copy")!.textContent = copy; document.querySelector("#record-command")!.textContent = `Desk command: ${command || "—"}`;
    const osc = [...sent.slice(-5), ...feedback.slice(-5).map((item) => `← ${item.address}  ${item.arguments.join(", ")}`)]; document.querySelector("#record-osc")!.textContent = osc.join("\n") || "Waiting for OSC…";
    document.querySelector("#record-dmx")!.innerHTML = dmx.logical.map((value, index) => `<div class="record-slot"><div class="record-bar" style="height:${Math.max(2, value / 255 * 82)}px"></div><span>${index + 1}<br><b>${value}</b></span></div>`).join("");
    document.querySelector("#record-protocol")!.innerHTML = `<div>Art-Net U1 · seq ${dmx.artnet.sequence}<br><b>${dmx.artnet.slots.join(" · ")}</b></div><div>sACN U101 · seq ${dmx.sacn.sequence}<br><b>${dmx.sacn.slots.join(" · ")}</b></div>`;
  }, { title, copy, sent, feedback, dmx, command });
}

interface DmxState { logical: number[]; artnet: { sequence: number; slots: number[] }; sacn: { sequence: number; slots: number[] } }
async function dmxState(api: any, bench: any): Promise<DmxState> {
  const snapshot = await api.request<any>("GET", "/api/v1/dmx", undefined, false);
  const logical = snapshot.universes.find((item: any) => item.universe === 1)?.slots.slice(0, 12) ?? Array(12).fill(0);
  const art = bench.artnet.packets.at(-1); const sacn = bench.sacn.packets.at(-1);
  return { logical, artnet: { sequence: art?.sequence ?? 0, slots: Array.from(art?.slots.slice(0, 6) ?? Array(6).fill(0)) }, sacn: { sequence: sacn?.sequence ?? 0, slots: Array.from(sacn?.slots.slice(0, 6) ?? Array(6).fill(0)) } };
}
async function pause(page: import("../apps/control-ui/node_modules/@playwright/test/index.js").Page, millis: number) { await page.waitForTimeout(millis); }
