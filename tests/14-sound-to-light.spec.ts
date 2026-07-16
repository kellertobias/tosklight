import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import type { BenchUiContext } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

const portableConfiguration = {
  enabled: true,
  analysis_mode: "tempo_bpm",
  frequency: { type: "custom", low_hz: 45, high_hz: 140 },
  input_gain_db: 6,
  confidence_threshold: 0.55,
  smoothing: 0,
  minimum_bpm: 60,
  maximum_bpm: 180,
  signal_hold_millis: 3_500,
  multiplier: 2,
};

const disabledConfiguration = {
  enabled: false,
  analysis_mode: "tempo_bpm",
  frequency: { type: "preset", preset: "low" },
  input_gain_db: 0,
  confidence_threshold: 0.65,
  smoothing: 0.35,
  minimum_bpm: 40,
  maximum_bpm: 240,
  signal_hold_millis: 2_000,
  multiplier: 1,
};

test.describe("docs/testing/08-sound-to-light.md", () => {
  pairedScenario<{ configuration: typeof portableConfiguration }>({
    id: "SOUND-001",
    title: "a desk-local audio input drives one authoritative Speed Group with portable response settings",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `sound-001-${surface}`, "compact-rig");
      await api.request("PUT", "/api/v1/speed-groups/A", disabledConfiguration);
      return { configuration: portableConfiguration };
    },
    api: async ({ api }, scenario) => {
      await api.request("PUT", "/api/v1/speed-groups/A", scenario.configuration);
      await api.request("POST", "/api/v1/speed-groups/A/observation", {
        captured_at_millis: 1,
        source_available: true,
        usable_signal: true,
        level: 0.72,
        selected_band_level: 0.84,
        detected_bpm: 120,
        confidence: 0.92,
      });
    },
    ui: async (context, scenario) => configureFromRecordedAudio(context, scenario.configuration),
    assert: async ({ api }, scenario) => {
      await expect.poll(async () => (await api.request<any>("GET", "/api/v1/speed-groups/A")).snapshot.source, { timeout: 8_000 }).toBe("sound");
      const state = await api.request<any>("GET", "/api/v1/speed-groups/A");
      expect(state.group).toBe("A");
      expect(state.configuration).toEqual(scenario.configuration);
      expect(state.configuration).not.toHaveProperty("device_id");
      expect(state.configuration).not.toHaveProperty("deviceId");
      expect(state.snapshot.source_available).toBe(true);
      expect(state.snapshot.usable_signal).toBe(true);
      expect(state.snapshot.sound_bpm).toBeGreaterThan(115);
      expect(state.snapshot.sound_bpm).toBeLessThan(125);
      expect(state.snapshot.effective_bpm).toBeGreaterThan(230);
      expect(state.snapshot.effective_bpm).toBeLessThan(250);
    },
  });
});

async function configureFromRecordedAudio({ api, bench, desk, page }: BenchUiContext, configuration: typeof portableConfiguration) {
  await installRecordedKickInput(page);
  await desk.recordStep("OPEN SPEED GROUP A", "Open the existing Speed Group control; this must configure Sound-to-Light rather than perform a Learn tap.");
  await desk.open(bench.baseUrl);
  api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".playback-tools")).toBeVisible();
  await page.getByRole("button", { name: /Speed group A, .* BPM/ }).click();
  const modal = page.getByRole("dialog", { name: "Speed Group A Sound to Light" });
  await expect(modal).toBeVisible();
  await expect(modal).toContainText("The device ID stays in this browser for this desk and is never saved in the show.");

  await desk.recordStep("ASSIGN RECORDED AUDIO", "Grant the synthetic microphone and assign the deterministic 120 BPM kick track to this browser and desk.");
  await modal.getByRole("button", { name: "Not assigned on this browser" }).click();
  await page.getByRole("option", { name: "Recorded kick track" }).click();
  await expect(modal.getByText("Capturing", { exact: true })).toBeVisible();
  await expect(modal.getByText("Granted", { exact: true })).toBeVisible();
  expect((await api.request<any>("GET", "/api/v1/speed-groups/A")).configuration.enabled).toBe(false);

  await desk.recordStep("SET RESPONSE", "Enable tempo analysis, isolate 45–140 Hz, and set gain, confidence, smoothing, hold, accepted BPM, and 2× mapping.");
  await modal.getByRole("switch", { name: "Enable Sound-to-Light" }).click();
  await modal.getByRole("button", { name: "Low · 60–180 Hz" }).click();
  await page.getByRole("option", { name: "Custom range" }).click();
  await modal.getByLabel("Custom low frequency").fill(String(configuration.frequency.low_hz));
  await modal.getByLabel("Custom high frequency").fill(String(configuration.frequency.high_hz));
  await setRange(modal.getByLabel("Input gain"), configuration.input_gain_db);
  await setRange(modal.getByLabel("Confidence threshold"), configuration.confidence_threshold);
  await setRange(modal.getByLabel("Tempo smoothing"), configuration.smoothing);
  await modal.getByLabel("Minimum accepted BPM").fill(String(configuration.minimum_bpm));
  await modal.getByLabel("Maximum accepted BPM").fill(String(configuration.maximum_bpm));
  await modal.getByLabel("Signal hold seconds").fill(String(configuration.signal_hold_millis / 1_000));
  await modal.getByLabel("Sound multiplier").fill(String(configuration.multiplier));
  await modal.getByRole("button", { name: "Apply" }).click();
  await expect(modal).toBeHidden();

  await desk.recordStep("ANALYZE AND VERIFY", "The browser analyzer now publishes normalized observations; the server should select Sound at about 120 BPM and map it to about 240 BPM.");
  await expect.poll(async () => (await api.request<any>("GET", "/api/v1/speed-groups/A")).snapshot.source, { timeout: 8_000 }).toBe("sound");
  const deskId = api.session!.desk.id;
  expect(await page.evaluate((key) => localStorage.getItem(key), `light.sound-to-light.device.${deskId}.A`)).toBe("test-line");
  await page.getByRole("button", { name: /Speed group A, .* BPM/ }).click();
  const live = page.getByRole("dialog", { name: "Speed Group A Sound to Light" });
  await expect(live.getByText("Capturing", { exact: true })).toBeVisible();
  await expect(live.getByText("Usable", { exact: true })).toBeVisible();
  await expect(live.getByText(/Sound · .* BPM/)).toBeVisible();
  await live.getByRole("button", { name: "Cancel" }).click();
}

async function setRange(locator: import("../apps/control-ui/node_modules/@playwright/test/index.js").Locator, value: number) {
  await locator.evaluate((element, next) => {
    const input = element as HTMLInputElement;
    input.value = String(next);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }, value);
}

async function installRecordedKickInput(page: import("../apps/control-ui/node_modules/@playwright/test/index.js").Page) {
  await page.addInitScript(() => {
    class FakeTrack extends EventTarget { stop() {} }
    class FakeStream {
      private readonly track = new FakeTrack();
      getTracks() { return [this.track]; }
      getAudioTracks() { return [this.track]; }
    }
    class FakeSource {
      connect() {}
      disconnect() {}
    }
    class FakeAnalyser {
      fftSize = 2_048;
      smoothingTimeConstant = 0;
      calls = 0;
      get frequencyBinCount() { return this.fftSize / 2; }
      getFloatTimeDomainData(values: Float32Array) { values.fill(0.05); }
      getByteFrequencyData(values: Uint8Array) {
        values.fill(2);
        const transient = this.calls++ % 5 === 0;
        for (let index = 2; index <= 7; index += 1) values[index] = transient ? 230 : 5;
      }
    }
    class FakeAudioContext {
      state = "running";
      sampleRate = 48_000;
      createMediaStreamSource() { return new FakeSource(); }
      createAnalyser() { return new FakeAnalyser(); }
      async resume() { this.state = "running"; }
      async close() { this.state = "closed"; }
    }
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: {
        async enumerateDevices() { return [{ kind: "audioinput", deviceId: "test-line", label: "Recorded kick track", groupId: "test", toJSON() { return this; } }]; },
        async getUserMedia() { return new FakeStream(); },
        addEventListener() {},
        removeEventListener() {},
      },
    });
    Object.defineProperty(navigator, "permissions", {
      configurable: true,
      value: { async query() { return { state: "granted", addEventListener() {}, removeEventListener() {} }; } },
    });
    Object.defineProperty(window, "AudioContext", { configurable: true, value: FakeAudioContext });
  });
}
