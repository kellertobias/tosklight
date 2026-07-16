import type { FrequencySelection, SoundObservation, SoundToLightConfig } from "../../api/types";

export interface AudioInputDevice {
  deviceId: string;
  label: string;
}

export type MicrophonePermission = PermissionState | "unknown" | "unsupported";

export type SoundCapturePhase =
  | "inactive"
  | "requesting"
  | "capturing"
  | "permission_denied"
  | "source_missing"
  | "unsupported"
  | "error";

export interface SoundCaptureStatus {
  phase: SoundCapturePhase;
  message: string;
  observation: SoundObservation | null;
}

export const inactiveCaptureStatus: SoundCaptureStatus = {
  phase: "inactive",
  message: "This browser is not assigned as an audio source.",
  observation: null,
};

export function monotonicEpochMillis() {
  return Math.round(performance.timeOrigin + performance.now());
}

export function frequencyRange(selection: FrequencySelection): [number, number] {
  if (selection.type === "custom") return [selection.low_hz, selection.high_hz];
  if (selection.preset === "sub") return [30, 80];
  if (selection.preset === "low") return [60, 180];
  if (selection.preset === "mid") return [180, 2_000];
  if (selection.preset === "high") return [2_000, 12_000];
  return [30, 18_000];
}

function clampUnit(value: number) {
  return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0));
}

function median(values: number[]) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

function normalizeInterval(interval: number, minimumBpm: number, maximumBpm: number) {
  let bpm = 60_000 / interval;
  while (bpm < minimumBpm && bpm * 2 <= maximumBpm) bpm *= 2;
  while (bpm > maximumBpm && bpm / 2 >= minimumBpm) bpm /= 2;
  return bpm >= minimumBpm && bpm <= maximumBpm ? 60_000 / bpm : null;
}

/** Lightweight transient tracker. Source selection and BPM smoothing remain server-authoritative. */
export class SoundTempoTracker {
  private energyHistory: Array<{ at: number; value: number }> = [];
  private beats: number[] = [];
  private previousEnergy = 0;
  private estimate: { bpm: number; confidence: number; at: number } | null = null;

  observe(capturedAt: number, energy: number, usableSignal: boolean, minimumBpm: number, maximumBpm: number) {
    this.energyHistory = this.energyHistory.filter((sample) => capturedAt - sample.at <= 4_000);
    const historical = this.energyHistory.map((sample) => sample.value);
    const mean = historical.length ? historical.reduce((sum, value) => sum + value, 0) / historical.length : 0;
    const deviation = historical.length
      ? Math.sqrt(historical.reduce((sum, value) => sum + (value - mean) ** 2, 0) / historical.length)
      : 0;
    const threshold = Math.max(0.025, mean * 1.35, mean + Math.max(0.015, deviation * 1.5));
    const lastBeat = this.beats.at(-1) ?? -Infinity;
    const transient = usableSignal
      && energy >= threshold
      && energy > this.previousEnergy * 1.08
      && capturedAt - lastBeat >= 250;
    this.energyHistory.push({ at: capturedAt, value: energy });
    this.previousEnergy = energy;

    if (transient) {
      this.beats.push(capturedAt);
      this.beats = this.beats.filter((beat) => capturedAt - beat <= 8_000).slice(-12);
      const intervals = this.beats.slice(1).map((beat, index) => beat - this.beats[index]);
      const normalized = intervals
        .map((interval) => normalizeInterval(interval, minimumBpm, maximumBpm))
        .filter((interval): interval is number => interval != null);
      if (normalized.length >= 2) {
        const stableInterval = median(normalized);
        const averageError = normalized.reduce((sum, value) => sum + Math.abs(value - stableInterval), 0) / normalized.length;
        const stability = clampUnit(1 - averageError / Math.max(1, stableInterval) * 5);
        const historyConfidence = Math.min(1, 0.4 + normalized.length * 0.15);
        this.estimate = {
          bpm: 60_000 / stableInterval,
          confidence: clampUnit(stability * historyConfidence * Math.min(1, energy * 1.4)),
          at: capturedAt,
        };
      }
    }

    if (this.estimate && capturedAt - (this.beats.at(-1) ?? 0) > Math.max(1_500, 120_000 / minimumBpm)) {
      this.estimate = null;
      this.beats = [];
    }
    return this.estimate ? { bpm: this.estimate.bpm, confidence: this.estimate.confidence } : null;
  }

  reset() {
    this.energyHistory = [];
    this.beats = [];
    this.previousEnergy = 0;
    this.estimate = null;
  }
}

export function analyzeAudioFrame(
  timeDomain: Float32Array,
  frequencyDomain: Uint8Array,
  sampleRate: number,
  fftSize: number,
  configuration: SoundToLightConfig,
  tracker: SoundTempoTracker,
  capturedAt: number,
): SoundObservation {
  const gain = 10 ** (configuration.input_gain_db / 20);
  const timeEnergy = timeDomain.reduce((sum, value) => sum + value * value, 0) / Math.max(1, timeDomain.length);
  const level = clampUnit(Math.sqrt(timeEnergy) * gain * 3);
  const [lowHz, highHz] = frequencyRange(configuration.frequency);
  const hzPerBin = sampleRate / fftSize;
  const firstBin = Math.max(0, Math.min(frequencyDomain.length - 1, Math.floor(lowHz / hzPerBin)));
  const lastBin = Math.max(firstBin, Math.min(frequencyDomain.length - 1, Math.ceil(highHz / hzPerBin)));
  let bandEnergy = 0;
  for (let index = firstBin; index <= lastBin; index += 1) bandEnergy += (frequencyDomain[index] / 255) ** 2;
  const selectedBandLevel = clampUnit(Math.sqrt(bandEnergy / Math.max(1, lastBin - firstBin + 1)) * gain);
  const usableSignal = level >= 0.01 && selectedBandLevel >= 0.008;
  const tempo = tracker.observe(
    capturedAt,
    selectedBandLevel,
    usableSignal,
    configuration.minimum_bpm,
    configuration.maximum_bpm,
  );
  return {
    captured_at_millis: Math.max(0, Math.round(capturedAt)),
    source_available: true,
    usable_signal: usableSignal,
    level,
    selected_band_level: selectedBandLevel,
    detected_bpm: tempo?.bpm ?? null,
    confidence: tempo?.confidence ?? 0,
  };
}

export async function enumerateAudioInputs(): Promise<AudioInputDevice[]> {
  if (!navigator.mediaDevices?.enumerateDevices) return [];
  const devices = await navigator.mediaDevices.enumerateDevices();
  let anonymous = 0;
  return devices.filter((device) => device.kind === "audioinput").map((device) => ({
    deviceId: device.deviceId,
    label: device.label || `Audio input ${++anonymous}`,
  }));
}

export async function microphonePermission(): Promise<MicrophonePermission> {
  if (!navigator.mediaDevices?.getUserMedia) return "unsupported";
  if (!navigator.permissions?.query) return "unknown";
  try {
    const status = await navigator.permissions.query({ name: "microphone" as PermissionName });
    return status.state;
  } catch {
    return "unknown";
  }
}

interface AudioContextWindow extends Window {
  webkitAudioContext?: typeof AudioContext;
}

export class SoundToLightAudioAnalyzer {
  private configuration: SoundToLightConfig;
  private context: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private analyser: AnalyserNode | null = null;
  private timer: number | null = null;
  private stopped = false;
  private readonly tracker = new SoundTempoTracker();

  constructor(
    configuration: SoundToLightConfig,
    private readonly onObservation: (observation: SoundObservation) => void,
    private readonly onStatus: (status: SoundCaptureStatus) => void,
    private readonly now: () => number = monotonicEpochMillis,
  ) {
    this.configuration = configuration;
  }

  updateConfiguration(configuration: SoundToLightConfig) {
    const changedBand = JSON.stringify(this.configuration.frequency) !== JSON.stringify(configuration.frequency)
      || this.configuration.minimum_bpm !== configuration.minimum_bpm
      || this.configuration.maximum_bpm !== configuration.maximum_bpm;
    this.configuration = configuration;
    if (changedBand) this.tracker.reset();
  }

  async start(deviceId: string) {
    this.stopped = false;
    if (!navigator.mediaDevices?.getUserMedia) {
      this.onStatus({ phase: "unsupported", message: "Audio capture is not supported by this browser.", observation: null });
      this.reportUnavailable();
      return;
    }
    const Context = window.AudioContext ?? (window as AudioContextWindow).webkitAudioContext;
    if (!Context) {
      this.onStatus({ phase: "unsupported", message: "Web Audio analysis is not supported by this browser.", observation: null });
      this.reportUnavailable();
      return;
    }
    this.onStatus({ phase: "requesting", message: "Waiting for microphone permission…", observation: null });
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          ...(deviceId && deviceId !== "default" ? { deviceId: { exact: deviceId } } : {}),
          autoGainControl: false,
          echoCancellation: false,
          noiseSuppression: false,
        },
      });
      if (this.stopped) {
        stream.getTracks().forEach((track) => track.stop());
        return;
      }
      const context = new Context();
      const source = context.createMediaStreamSource(stream);
      const analyser = context.createAnalyser();
      analyser.fftSize = 2_048;
      analyser.smoothingTimeConstant = 0.2;
      source.connect(analyser);
      this.stream = stream;
      this.context = context;
      this.source = source;
      this.analyser = analyser;
      stream.getAudioTracks().forEach((track) => track.addEventListener("ended", () => {
        if (this.stopped) return;
        this.onStatus({ phase: "source_missing", message: "The selected audio input was disconnected.", observation: null });
        this.reportUnavailable();
      }, { once: true }));
      if (context.state === "suspended") await context.resume();
      this.onStatus({ phase: "capturing", message: "Audio input is available and being analyzed.", observation: null });
      const timeDomain = new Float32Array(analyser.fftSize);
      const frequencyDomain = new Uint8Array(analyser.frequencyBinCount);
      const sample = () => {
        if (this.stopped || !this.analyser || !this.context) return;
        this.analyser.getFloatTimeDomainData(timeDomain);
        this.analyser.getByteFrequencyData(frequencyDomain);
        const observation = analyzeAudioFrame(
          timeDomain,
          frequencyDomain,
          this.context.sampleRate,
          this.analyser.fftSize,
          this.configuration,
          this.tracker,
          this.now(),
        );
        this.onObservation(observation);
        this.onStatus({
          phase: "capturing",
          message: observation.usable_signal ? "Usable signal detected." : "Input is connected, but the selected band is quiet.",
          observation,
        });
      };
      sample();
      this.timer = window.setInterval(sample, 100);
    } catch (reason) {
      if (this.stopped) return;
      const error = reason as DOMException;
      const denied = error?.name === "NotAllowedError" || error?.name === "SecurityError";
      const missing = error?.name === "NotFoundError" || error?.name === "OverconstrainedError";
      this.onStatus({
        phase: denied ? "permission_denied" : missing ? "source_missing" : "error",
        message: denied
          ? "Microphone permission was denied. Allow access in the browser or system settings."
          : missing
            ? "The selected audio input is unavailable. Choose another input."
            : `Audio capture failed: ${error?.message || String(reason)}`,
        observation: null,
      });
      this.reportUnavailable();
    }
  }

  stop() {
    this.stopped = true;
    if (this.timer != null) window.clearInterval(this.timer);
    this.timer = null;
    this.source?.disconnect();
    this.stream?.getTracks().forEach((track) => track.stop());
    void this.context?.close();
    this.source = null;
    this.stream = null;
    this.analyser = null;
    this.context = null;
    this.tracker.reset();
  }

  private reportUnavailable() {
    this.onObservation({
      captured_at_millis: this.now(),
      source_available: false,
      usable_signal: false,
      level: 0,
      selected_band_level: 0,
      detected_bpm: null,
      confidence: 0,
    });
  }
}
