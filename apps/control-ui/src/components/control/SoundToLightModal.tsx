import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import type {
  FrequencyPreset,
  SoundLossReason,
  SoundToLightConfig,
  SpeedGroupActionInput,
  SpeedGroupId,
  SpeedGroupSoundState,
} from "../../api/types";
import { Button, FormField, FormLayout, Input, NumberField, SelectField, SwitchField } from "../common";
import { ModalTitleBar } from "../common/ModalTitleBar";
import { monotonicEpochMillis, type AudioInputDevice, type MicrophonePermission, type SoundCaptureStatus } from "./soundToLightAnalyzer";
import "./SoundToLightModal.css";

const frequencyOptions: Array<{ value: FrequencyPreset | "custom"; label: string }> = [
  { value: "sub", label: "Sub · 30–80 Hz" },
  { value: "low", label: "Low · 60–180 Hz" },
  { value: "mid", label: "Mid · 180–2,000 Hz" },
  { value: "high", label: "High · 2,000–12,000 Hz" },
  { value: "full_range", label: "Full range · 30–18,000 Hz" },
  { value: "custom", label: "Custom range" },
];

function reasonLabel(reason: SoundLossReason) {
  if (reason === "source_unavailable") return "source unavailable";
  if (reason === "no_usable_signal") return "no usable signal";
  if (reason === "low_confidence") return "confidence below threshold";
  if (reason === "tempo_outside_range") return "tempo outside accepted range";
  return "waiting for tempo analysis";
}

function permissionLabel(permission: MicrophonePermission) {
  if (permission === "granted") return "Granted";
  if (permission === "denied") return "Denied";
  if (permission === "prompt") return "Permission required";
  if (permission === "unsupported") return "Unsupported";
  return "Not checked";
}

function captureLabel(capture: SoundCaptureStatus) {
  if (capture.phase === "capturing") return "Capturing";
  if (capture.phase === "requesting") return "Requesting access";
  if (capture.phase === "permission_denied") return "Permission denied";
  if (capture.phase === "source_missing") return "Input unavailable";
  if (capture.phase === "unsupported") return "Unsupported";
  if (capture.phase === "error") return "Capture error";
  return "Not assigned";
}

function sourceLabel(state: SpeedGroupSoundState) {
  const status = state.snapshot.sound_status;
  if (status.state === "disabled") return "Manual";
  if (status.state === "active") return `Sound · ${status.detected_bpm.toFixed(1)} BPM`;
  if (status.state === "holding") return `Holding sound · ${Math.ceil(status.remaining_millis / 100) / 10}s`;
  return `Manual fallback · ${reasonLabel(status.reason)}`;
}

function validationError(configuration: SoundToLightConfig) {
  if (configuration.frequency.type === "custom") {
    if (configuration.frequency.low_hz < 20 || configuration.frequency.high_hz > 20_000 || configuration.frequency.low_hz >= configuration.frequency.high_hz)
      return "Custom frequency range must be ordered and stay within 20–20,000 Hz.";
  }
  if (!Number.isFinite(configuration.input_gain_db) || configuration.input_gain_db < -60 || configuration.input_gain_db > 60)
    return "Input gain must stay between −60 and +60 dB.";
  if (!Number.isFinite(configuration.confidence_threshold) || configuration.confidence_threshold < 0 || configuration.confidence_threshold > 1)
    return "Confidence threshold must stay between 0 and 1.";
  if (!Number.isFinite(configuration.smoothing) || configuration.smoothing < 0 || configuration.smoothing > 0.99)
    return "Smoothing must stay between 0 and 0.99.";
  if (configuration.minimum_bpm < 0.1 || configuration.maximum_bpm > 999 || configuration.minimum_bpm >= configuration.maximum_bpm)
    return "Minimum BPM must be lower than maximum BPM, within 0.1–999.";
  if (configuration.signal_hold_millis < 0 || configuration.signal_hold_millis > 60_000)
    return "Signal hold must stay between 0 and 60 seconds.";
  if (configuration.multiplier < 0.125 || configuration.multiplier > 8)
    return "Multiplier must stay between 0.125× and 8×.";
  return null;
}

function Meter({ label, value }: { label: string; value: number }) {
  const normalized = Math.max(0, Math.min(1, value));
  return <div className="sound-meter">
    <span>{label}</span>
    <div role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(normalized * 100)}>
      <i style={{ width: `${normalized * 100}%` }}/>
    </div>
    <output>{Math.round(normalized * 100)}%</output>
  </div>;
}

function RangeField({ label, value, minimum, maximum, step, unit, onChange }: {
  label: string;
  value: number;
  minimum: number;
  maximum: number;
  step: number;
  unit: string;
  onChange: (value: number) => void;
}) {
  return <FormField label={label}><div className="sound-range-field">
    <Input type="range" aria-label={label} min={minimum} max={maximum} step={step} value={value} onChange={(event) => onChange(Number(event.target.value))}/>
    <output>{value}{unit}</output>
  </div></FormField>;
}

export function SoundToLightModal({
  group,
  state,
  capture,
  permission,
  devices,
  deviceId,
  controllerError,
  onDeviceChange,
  onRefreshInputs,
  onPreview,
  onSave,
  onAction,
  onClose,
}: {
  group: SpeedGroupId;
  state: SpeedGroupSoundState;
  capture: SoundCaptureStatus;
  permission: MicrophonePermission;
  devices: AudioInputDevice[];
  deviceId: string;
  controllerError?: string | null;
  onDeviceChange: (deviceId: string) => void;
  onRefreshInputs: () => Promise<void>;
  onPreview: (group: SpeedGroupId, configuration: SoundToLightConfig | null) => void;
  onSave: (configuration: SoundToLightConfig) => Promise<SpeedGroupSoundState>;
  onAction: (input: SpeedGroupActionInput) => Promise<SpeedGroupSoundState>;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<SoundToLightConfig>(() => structuredClone(state.configuration));
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  useEffect(() => { onPreview(group, draft); }, [draft, group, onPreview]);
  useEffect(() => () => onPreview(group, null), [group, onPreview]);
  const invalid = validationError(draft);
  const observation = capture.observation;
  const inputLevel = observation?.level ?? state.snapshot.input_level;
  const bandLevel = observation?.selected_band_level ?? state.snapshot.selected_band_level;
  const detectedBpm = observation?.detected_bpm ?? state.snapshot.sound_bpm;
  const confidence = observation?.confidence ?? (state.snapshot.sound_status.state === "active" ? state.snapshot.sound_status.confidence : 0);
  const frequency = draft.frequency.type === "preset" ? draft.frequency.preset : "custom";
  const options = useMemo(() => {
    const available = devices.filter((device) => device.deviceId && device.deviceId !== "default");
    const result: Array<{ value: string; label: string }> = [
      { value: "", label: "Not assigned on this browser" },
      { value: "default", label: "System default input" },
      ...available.map((device) => ({ value: device.deviceId, label: device.label })),
    ];
    if (deviceId && !result.some((option) => option.value === deviceId)) result.push({ value: deviceId, label: "Previously selected input · unavailable" });
    return result;
  }, [deviceId, devices]);
  const update = <Key extends keyof SoundToLightConfig>(key: Key, value: SoundToLightConfig[Key]) => setDraft((current) => ({ ...current, [key]: value }));
  const updateCustomFrequency = (change: { low_hz?: number; high_hz?: number }) => setDraft((current) => {
    const frequency = current.frequency.type === "custom" ? current.frequency : { type: "custom" as const, low_hz: 60, high_hz: 180 };
    return { ...current, frequency: { ...frequency, ...change } };
  });
  const apply = async () => {
    if (invalid) {
      setLocalError(invalid);
      return;
    }
    setBusy(true);
    try {
      const saved = await onSave(draft);
      setDraft(structuredClone(saved.configuration));
      setLocalError(null);
      onClose();
    } catch (reason) {
      setLocalError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };
  const action = async (input: SpeedGroupActionInput) => {
    setBusy(true);
    try {
      const next = await onAction(input);
      setDraft(structuredClone(next.configuration));
      setLocalError(null);
    } catch (reason) {
      setLocalError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };
  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal sound-to-light-modal" role="dialog" aria-modal="true" aria-label={`Speed Group ${group} Sound to Light`}>
      <ModalTitleBar title={`Speed Group ${group} · Sound to Light`} closeLabel="Close Sound-to-Light configuration" onClose={onClose}/>
      <div className="sound-status-grid" aria-label="Audio status">
        <article className={`status-${permission}`}><small>Microphone permission</small><strong>{permissionLabel(permission)}</strong></article>
        <article className={`status-${capture.phase}`}><small>Audio source</small><strong>{captureLabel(capture)}</strong></article>
        <article className={observation?.usable_signal || state.snapshot.usable_signal ? "status-usable" : "status-waiting"}><small>Selected-band signal</small><strong>{observation?.usable_signal || state.snapshot.usable_signal ? "Usable" : "Waiting / quiet"}</strong></article>
      </div>
      <p className="sound-capture-message">{capture.message}</p>
      <div className="sound-source-row">
        <SelectField label="Audio input on this desk/browser" value={deviceId} options={options} onChange={onDeviceChange} description="The device ID stays in this browser for this desk and is never saved in the show."/>
        <Button onClick={() => void onRefreshInputs()}>Refresh inputs</Button>
      </div>
      <section className="sound-live-panel" aria-label="Live sound analysis">
        <div className="sound-meter-stack"><Meter label="Input level" value={inputLevel}/><Meter label="Selected band" value={bandLevel}/></div>
        <dl>
          <div><dt>Detected tempo</dt><dd>{detectedBpm == null ? "—" : `${detectedBpm.toFixed(1)} BPM`}</dd></div>
          <div><dt>Confidence</dt><dd>{Math.round(confidence * 100)}%</dd></div>
          <div><dt>Effective speed</dt><dd>{state.snapshot.effective_bpm.toFixed(1)} BPM</dd></div>
          <div><dt>Authoritative source</dt><dd>{sourceLabel(state)}</dd></div>
        </dl>
      </section>
      <FormLayout columns={2} minColumnWidth={250} className="sound-configuration-grid">
        <SwitchField label="Enable Sound-to-Light" checked={draft.enabled} onChange={(event) => update("enabled", event.target.checked)} description="When signal is lost, this group holds the last sound tempo, then returns to its manual BPM."/>
        <SelectField label="Analysis" value="tempo_bpm" disabled options={[{ value: "tempo_bpm", label: "Tempo / BPM" }]} onChange={() => undefined}/>
        <SelectField label="Frequency region" value={frequency} options={frequencyOptions} onChange={(value) => update("frequency", value === "custom" ? { type: "custom", low_hz: 60, high_hz: 180 } : { type: "preset", preset: value })}/>
        {draft.frequency.type === "custom" && <div className="sound-custom-frequency">
          <NumberField label="Low frequency" aria-label="Custom low frequency" value={draft.frequency.low_hz} min={20} max={19_999} unit="Hz" onValueChange={(value) => updateCustomFrequency({ low_hz: Number(value) })}/>
          <NumberField label="High frequency" aria-label="Custom high frequency" value={draft.frequency.high_hz} min={21} max={20_000} unit="Hz" onValueChange={(value) => updateCustomFrequency({ high_hz: Number(value) })}/>
        </div>}
        <RangeField label="Input gain" value={draft.input_gain_db} minimum={-60} maximum={60} step={1} unit=" dB" onChange={(value) => update("input_gain_db", value)}/>
        <RangeField label="Confidence threshold" value={draft.confidence_threshold} minimum={0} maximum={1} step={0.01} unit="" onChange={(value) => update("confidence_threshold", value)}/>
        <RangeField label="Tempo smoothing" value={draft.smoothing} minimum={0} maximum={0.99} step={0.01} unit="" onChange={(value) => update("smoothing", value)}/>
        <NumberField label="Minimum accepted tempo" aria-label="Minimum accepted BPM" value={draft.minimum_bpm} min={0.1} max={998} step={1} allowDecimal unit="BPM" onChange={(event) => update("minimum_bpm", Number(event.target.value))}/>
        <NumberField label="Maximum accepted tempo" aria-label="Maximum accepted BPM" value={draft.maximum_bpm} min={0.2} max={999} step={1} allowDecimal unit="BPM" onChange={(event) => update("maximum_bpm", Number(event.target.value))}/>
        <NumberField label="Signal-loss hold" aria-label="Signal hold seconds" value={draft.signal_hold_millis / 1_000} min={0} max={60} step={0.5} allowDecimal unit="s" onChange={(event) => update("signal_hold_millis", Math.round(Number(event.target.value) * 1_000))}/>
        <NumberField label="Sound speed ratio" aria-label="Sound multiplier" value={draft.multiplier} min={0.125} max={8} step={0.125} allowDecimal unit="×" onChange={(event) => update("multiplier", Number(event.target.value))}/>
      </FormLayout>
      <section className="sound-manual-actions" aria-label="Speed Group controls">
        <Button disabled={busy} onClick={() => void action({ action: "learn", captured_at_millis: monotonicEpochMillis() })}>Learn</Button>
        <Button disabled={busy} onClick={() => void action({ action: "half" })}>Half</Button>
        <Button disabled={busy} onClick={() => void action({ action: "double" })}>Double</Button>
        <Button active={state.snapshot.paused} disabled={busy} onClick={() => void action({ action: "pause" })}>{state.snapshot.paused ? "Resume" : "Pause"}</Button>
      </section>
      {(invalid || localError || controllerError) && <p className="sound-error" role="alert">{localError ?? invalid ?? controllerError}</p>}
      <footer className="modal-actions"><Button onClick={onClose}>Cancel</Button><Button variant="primary" loading={busy} disabled={Boolean(invalid)} onClick={() => void apply()}>Apply</Button></footer>
    </section>
  </div>, document.body);
}
