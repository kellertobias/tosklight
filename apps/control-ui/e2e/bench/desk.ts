import { expect, type Page } from "@playwright/test";

export class DeskDriver {
  private recordingStep = { title: "STARTING", description: "Preparing the test application." };
  private recordingInstalled = false;

  constructor(readonly page: Page, private readonly testTitle = "Light UI test") {}

  async open(baseUrl: string): Promise<void> {
    await this.page.goto(baseUrl);
    await expect(this.page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    await expect(this.page.locator(".connection-banner")).toBeHidden({ timeout: 10_000 });
    if (
      process.env.LIGHT_VISUAL_RECORDING === "1"
      && !this.testTitle.includes("records the complete desk")
    ) {
      await this.installRecordingOverlay();
      await this.recordStep(
        this.recordingStep.title === "STARTING" ? "APPLICATION READY" : this.recordingStep.title,
        this.recordingStep.title === "STARTING"
          ? "The complete production desk is connected and ready for the scenario."
          : this.recordingStep.description,
      );
    }
  }

  /** Adds a human-readable chapter card to visual recordings without affecting fast test runs. */
  async recordStep(title: string, description: string): Promise<void> {
    this.recordingStep = { title, description };
    if (process.env.LIGHT_VISUAL_RECORDING !== "1" || !this.recordingInstalled || this.page.isClosed()) return;
    await this.page.evaluate(({ title, description }) => {
      document.querySelector("#light-catalog-step")!.textContent = title;
      document.querySelector("#light-catalog-description")!.textContent = description;
    }, { title, description });
    const pause = Number(process.env.LIGHT_VISUAL_STEP_PAUSE ?? 900);
    if (Number.isFinite(pause) && pause > 0) await this.page.waitForTimeout(pause);
  }

  private async installRecordingOverlay(): Promise<void> {
    if (this.recordingInstalled) return;
    await this.page.addStyleTag({ content: `
      #light-catalog-recording{position:fixed;z-index:999998;left:14px;right:14px;top:12px;min-height:74px;display:grid;grid-template-columns:minmax(0,1fr) 510px;gap:12px;padding:10px 13px;border:2px solid #1bd6ec;border-radius:10px;background:#071017ed;color:#edf7ff;box-shadow:0 10px 35px #000b;font:14px/1.25 ui-monospace,SFMono-Regular,Menlo,monospace;pointer-events:none}
      #light-catalog-recording .catalog-copy{min-width:0}#light-catalog-recording small{display:block;color:#74ddea;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}#light-catalog-recording strong{display:block;margin:3px 0;color:#fff;font:800 19px/1.05 system-ui}#light-catalog-recording p{margin:0;color:#c5d0d8;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
      #light-catalog-recording .catalog-state{display:grid;grid-template-columns:1fr 1fr;gap:9px}#light-catalog-recording section{min-width:0;padding:5px 8px;border:1px solid #344552;border-radius:6px;background:#101922}#light-catalog-recording b{display:block;color:#ffc44d;font-size:11px}#light-catalog-recording output{display:block;overflow:hidden;color:#d9eef9;white-space:nowrap;text-overflow:ellipsis;font-size:12px}
    ` });
    await this.page.evaluate(({ testTitle }) => {
      const previous = document.querySelector("#light-catalog-recording");
      previous?.remove();
      const overlay = document.createElement("aside");
      overlay.id = "light-catalog-recording";
      overlay.innerHTML = `<div class="catalog-copy"><small id="light-catalog-test"></small><strong id="light-catalog-step">APPLICATION READY</strong><p id="light-catalog-description"></p></div><div class="catalog-state"><section><b>DESK / OSC EVENTS</b><output id="light-catalog-events">Waiting for events…</output></section><section><b>ACTUAL DMX U1</b><output id="light-catalog-dmx">Waiting for output…</output></section></div>`;
      overlay.querySelector("#light-catalog-test")!.textContent = testTitle;
      document.body.append(overlay);
      let revision = 0;
      const update = async () => {
        try {
          const session = JSON.parse(localStorage.getItem("light.primary-session") ?? "null");
          if (!session?.token) return;
          const headers = { Authorization: `Bearer ${session.token}` };
          const [dmxResponse, eventsResponse] = await Promise.all([
            fetch("/api/v1/dmx", { headers }),
            fetch(`/api/v1/audit?after=${revision}`, { headers }),
          ]);
          if (dmxResponse.ok) {
            const dmx = await dmxResponse.json();
            const slots = dmx.universes?.find((universe: { universe: number }) => universe.universe === 1)?.slots?.slice(0, 12) ?? [];
            document.querySelector("#light-catalog-dmx")!.textContent = slots.map((value: number, index: number) => `${index + 1}:${value}`).join(" · ") || "No Universe 1 frame";
          }
          if (eventsResponse.ok) {
            const events = await eventsResponse.json() as Array<{ revision: number; kind: string; payload: Record<string, unknown> }>;
            if (events.length) {
              revision = Math.max(revision, ...events.map((event) => event.revision));
              const visible = events.filter((event) => event.kind === "desk_action" || event.payload?.source === "osc" || event.kind.includes("playback") || event.kind.includes("speed_group"));
              const latest = (visible.at(-1) ?? events.at(-1))!;
              const action = latest.payload?.action ?? latest.payload?.command ?? latest.payload?.source ?? "state changed";
              document.querySelector("#light-catalog-events")!.textContent = `${latest.kind} · ${String(action)}`;
            }
          }
        } catch {
          // The overlay is evidence only; a transient refresh error must not alter the test.
        }
      };
      void update();
      window.setInterval(() => void update(), 250);
    }, { testTitle: this.testTitle });
    this.recordingInstalled = true;
  }

  async command(value: string, visibleValue = formatVisibleCommand(value)): Promise<void> {
    const command = this.page.getByLabel("Command line");
    await this.page.getByRole("button", { name: "ESC", exact: true }).click();
    const keys = value.trim().split(/\s+/).flatMap((token) => {
      if (token === "GROUP") return ["GRP"];
      if (/^\d+$/.test(token)) return [...token];
      return [token];
    });
    for (const key of keys) {
      await this.page.getByRole("button", { name: key, exact: true }).click();
    }
    await expect(command).toHaveValue(visibleValue);
    await this.page.getByRole("button", { name: "ENT", exact: true }).click();
  }

  async openFixtures(): Promise<void> {
    await this.page.getByRole("button", { name: "BUILT-INS" }).click();
    await this.page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  }
}

function formatVisibleCommand(value: string): string {
  const tokens = value.trim().split(/\s+/);
  let scope: "fixture" | "group" = "fixture";
  let expectsSelectionNumber = true;
  return tokens.map((token) => {
    if (token === "GROUP") {
      scope = "group";
      expectsSelectionNumber = true;
      return null;
    }
    if (token === "+" || token === "-") {
      expectsSelectionNumber = true;
      return token;
    }
    if (token === "AT" || token === "THRU") {
      expectsSelectionNumber = token === "THRU";
      return token;
    }
    if (/^\d+$/.test(token) && expectsSelectionNumber) {
      expectsSelectionNumber = false;
      return `${scope === "group" ? "G" : "F"}${token}`;
    }
    return token;
  }).filter((token): token is string => token !== null).join(" ");
}
