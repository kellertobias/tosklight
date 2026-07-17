import { expect, type Page } from "@playwright/test";

export class DeskDriver {
  private recordingStep = { title: "STARTING", description: "Preparing the test application." };
  private recordingInstalled = false;
  private recordingNavigationHandler?: () => void;

  constructor(
    readonly page: Page,
    private readonly testTitle = "Light UI test",
    private readonly controlDeskId: string | null = null,
    private readonly externalOscSummary: () => string = () => "",
  ) {}

  async dispose(): Promise<void> {
    if (this.recordingNavigationHandler) this.page.off("domcontentloaded", this.recordingNavigationHandler);
    this.recordingNavigationHandler = undefined;
  }

  async open(baseUrl: string): Promise<void> {
    if (this.controlDeskId) {
      await this.page.addInitScript((deskId) => {
        localStorage.setItem("light.control-desk", deskId);
      }, this.controlDeskId);
    }
    await this.page.goto(baseUrl);
    await expect(this.page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    await expect(this.page.locator(".connection-banner")).toBeHidden({ timeout: 10_000 });
    if (this.controlDeskId) {
      await expect.poll(() => this.page.evaluate(() => {
        const session = JSON.parse(localStorage.getItem("light.primary-session") ?? "null");
        return session?.desk?.id ?? null;
      })).toBe(this.controlDeskId);
    }
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
    if (await this.page.locator("#light-catalog-recording").count() === 0) {
      await this.renderRecordingOverlay();
    }
    await this.page.evaluate(({ title, description }) => {
      document.querySelector("#light-catalog-step")!.textContent = title;
      document.querySelector("#light-catalog-description")!.textContent = description;
    }, { title, description });
    const pause = Number(process.env.LIGHT_VISUAL_STEP_PAUSE ?? 1_200);
    if (Number.isFinite(pause) && pause > 0) await this.page.waitForTimeout(pause);
  }

  private async installRecordingOverlay(): Promise<void> {
    if (this.recordingInstalled) return;
    await this.page.exposeFunction("__lightVisualOscSummary", () => this.externalOscSummary());
    this.recordingNavigationHandler = () => {
      void this.renderRecordingOverlay().catch(() => {
        // A second navigation or teardown can supersede this document.
      });
    };
    this.page.on("domcontentloaded", this.recordingNavigationHandler);
    await this.renderRecordingOverlay();
    this.recordingInstalled = true;
  }

  private async renderRecordingOverlay(): Promise<void> {
    await this.page.addStyleTag({ content: `
      body{position:fixed!important;inset:0 640px 0 0!important;width:auto!important;height:100%!important;transform:translateZ(0)}
      #light-catalog-recording{position:fixed;z-index:999998;inset:0 0 0 auto;width:640px;display:grid;grid-template-rows:auto minmax(0,1fr);gap:14px;padding:18px;border-left:3px solid #1bd6ec;background:#071017;color:#edf7ff;box-shadow:-10px 0 35px #000b;font:16px/1.35 ui-monospace,SFMono-Regular,Menlo,monospace;pointer-events:none}
      #light-catalog-recording .catalog-copy{min-width:0;padding:14px;border:1px solid #344552;border-radius:10px;background:#101922}#light-catalog-recording small{display:block;color:#74ddea;overflow-wrap:anywhere}#light-catalog-recording strong{display:block;margin:8px 0;color:#fff;font:800 25px/1.08 system-ui}#light-catalog-recording p{margin:0;color:#c5d0d8;overflow-wrap:anywhere}
      #light-catalog-recording .catalog-state{min-height:0;display:grid;grid-template-rows:repeat(3,minmax(0,1fr));gap:12px}#light-catalog-recording section{min-width:0;min-height:0;padding:14px;border:1px solid #344552;border-radius:10px;background:#101922;overflow:hidden}#light-catalog-recording b{display:block;margin-bottom:8px;color:#ffc44d;font-size:13px}#light-catalog-recording output{display:block;max-height:100%;overflow:auto;color:#d9eef9;white-space:pre-wrap;overflow-wrap:anywhere;font-size:14px}
    ` });
    await this.page.evaluate(({ testTitle, recordingStep }) => {
      const previous = document.querySelector("#light-catalog-recording");
      if (previous) {
        previous.querySelector("#light-catalog-step")!.textContent = recordingStep.title;
        previous.querySelector("#light-catalog-description")!.textContent = recordingStep.description;
        return;
      }
      const overlay = document.createElement("aside");
      overlay.id = "light-catalog-recording";
      overlay.innerHTML = `<div class="catalog-copy"><small id="light-catalog-test"></small><strong id="light-catalog-step"></strong><p id="light-catalog-description"></p></div><div class="catalog-state"><section><b>DESK EVENTS</b><output id="light-catalog-events">Waiting for events…</output></section><section><b>EXTERNAL OSC (TX / RX)</b><output id="light-catalog-osc">No external OSC yet</output></section><section><b>ACTUAL DMX U1</b><output id="light-catalog-dmx">Waiting for output…</output></section></div>`;
      overlay.querySelector("#light-catalog-test")!.textContent = testTitle;
      overlay.querySelector("#light-catalog-step")!.textContent = recordingStep.title;
      overlay.querySelector("#light-catalog-description")!.textContent = recordingStep.description;
      document.documentElement.append(overlay);
      const purpose = (testTitle.split("›").at(-1) ?? testTitle).trim().slice(0, 150);
      const describeControl = (target: EventTarget | null) => {
        const element = target instanceof Element
          ? target.closest<HTMLElement>('button,[role="button"],input,textarea,select,[role="slider"],[role="option"],[role="tab"]')
          : null;
        if (!element || element.closest("#light-catalog-recording")) return;
        const label = element.getAttribute("aria-label")
          || element.getAttribute("title")
          || (element instanceof HTMLInputElement ? element.labels?.[0]?.textContent : null)
          || element.textContent
          || element.getAttribute("name")
          || element.tagName.toLowerCase();
        const concise = label.replace(/\s+/g, " ").trim().slice(0, 140);
        document.querySelector("#light-catalog-step")!.textContent = "UI ACTION";
        document.querySelector("#light-catalog-description")!.textContent = concise
          ? `Using “${concise}” to verify: ${purpose}.`
          : `Operating the production UI to verify: ${purpose}.`;
      };
      document.addEventListener("pointerdown", (event) => describeControl(event.target), true);
      document.addEventListener("input", (event) => describeControl(event.target), true);
      document.addEventListener("keydown", (event) => {
        if (event.key === "Shift" || event.key === "Control" || event.key === "Alt" || event.key === "Meta") return;
        describeControl(event.target);
        const description = document.querySelector("#light-catalog-description")!;
        description.textContent = `${description.textContent} Keyboard: ${event.key}.`;
      }, true);
      const visualWindow = window as Window & { __lightVisualOscSummary?: () => Promise<string> };
      let revision = 0;
      let updating = false;
      const update = async () => {
        if (updating) return;
        updating = true;
        try {
          const oscSummary = await visualWindow.__lightVisualOscSummary?.();
          const oscOutput = document.querySelector("#light-catalog-osc");
          if (oscOutput) oscOutput.textContent = oscSummary || "No external OSC yet";
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
        } finally {
          updating = false;
        }
      };
      void update();
      window.setInterval(() => void update(), 500);
    }, { testTitle: this.testTitle, recordingStep: this.recordingStep });
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
