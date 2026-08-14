import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import type {
	FrequencyPreset,
	SoundLossReason,
	SoundToLightConfig,
	SpeedGroupActionInput,
	SpeedGroupId,
	SpeedGroupSoundState,
	SpeedGroupSource,
} from "../../api/types";
import {
	Button,
	FormField,
	FormLayout,
	Input,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
} from "@tosklight/ui";
import { type SoundCaptureStatus } from "./soundToLightAnalyzer";
import { formatSpeedGroupBpm } from "./speedGroupFormatting";
import "./SoundToLightModal.css";

const frequencyOptions: Array<{
	value: FrequencyPreset | "custom";
	label: string;
}> = [
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
	if (state.source?.type === "manual") return "Manual";
	if (state.source?.type === "speed_group")
		return `Speed Group ${state.source.group}`;
	const status = state.snapshot.sound_status;
	if (status.state === "disabled") return "Manual";
	if (status.state === "active")
		return `Sound · ${formatSpeedGroupBpm(status.detected_bpm)} BPM`;
	if (status.state === "holding")
		return `Holding sound · ${Math.ceil(status.remaining_millis / 100) / 10}s`;
	return `Manual fallback · ${reasonLabel(status.reason)}`;
}

function validationError(configuration: SoundToLightConfig) {
	if (configuration.frequency.type === "custom") {
		if (
			configuration.frequency.low_hz < 20 ||
			configuration.frequency.high_hz > 20_000 ||
			configuration.frequency.low_hz >= configuration.frequency.high_hz
		)
			return "Custom frequency range must be ordered and stay within 20–20,000 Hz.";
	}
	if (
		!Number.isFinite(configuration.input_gain_db) ||
		configuration.input_gain_db < -60 ||
		configuration.input_gain_db > 60
	)
		return "Input gain must stay between −60 and +60 dB.";
	if (
		!Number.isFinite(configuration.confidence_threshold) ||
		configuration.confidence_threshold < 0 ||
		configuration.confidence_threshold > 1
	)
		return "Confidence threshold must stay between 0 and 1.";
	if (
		!Number.isFinite(configuration.smoothing) ||
		configuration.smoothing < 0 ||
		configuration.smoothing > 0.99
	)
		return "Smoothing must stay between 0 and 0.99.";
	if (
		configuration.minimum_bpm < 0.1 ||
		configuration.maximum_bpm > 999 ||
		configuration.minimum_bpm >= configuration.maximum_bpm
	)
		return "Minimum BPM must be lower than maximum BPM, within 0.1–999.";
	if (
		configuration.signal_hold_millis < 0 ||
		configuration.signal_hold_millis > 60_000
	)
		return "Signal hold must stay between 0 and 60 seconds.";
	if (configuration.multiplier < 0.125 || configuration.multiplier > 8)
		return "Multiplier must stay between 0.125× and 8×.";
	return null;
}

function Meter({ label, value }: { label: string; value: number }) {
	const normalized = Math.max(0, Math.min(1, value));
	return (
		<div className="sound-meter">
			<span>{label}</span>
			<div
				role="meter"
				aria-label={label}
				aria-valuemin={0}
				aria-valuemax={100}
				aria-valuenow={Math.round(normalized * 100)}
			>
				<i style={{ width: `${normalized * 100}%` }} />
			</div>
			<output>{Math.round(normalized * 100)}%</output>
		</div>
	);
}

function RangeField({
	label,
	value,
	minimum,
	maximum,
	step,
	unit,
	onChange,
}: {
	label: string;
	value: number;
	minimum: number;
	maximum: number;
	step: number;
	unit: string;
	onChange: (value: number) => void;
}) {
	return (
		<FormField label={label}>
			<div className="sound-range-field">
				<Input
					type="range"
					aria-label={label}
					min={minimum}
					max={maximum}
					step={step}
					value={value}
					onChange={(event) => onChange(Number(event.target.value))}
				/>
				<output>
					{value}
					{unit}
				</output>
			</div>
		</FormField>
	);
}

function SoundToLightFields({
	group,
	state,
	capture,
	draft,
	source,
	frequency,
	inputLevel,
	bandLevel,
	detectedBpm,
	confidence,
	onSourceTypeSelect,
	onSourceChange,
	onUpdate,
	onCustomFrequencyChange,
}: {
	group: SpeedGroupId;
	state: SpeedGroupSoundState;
	capture: SoundCaptureStatus;
	draft: SoundToLightConfig;
	source: SpeedGroupSource;
	frequency: FrequencyPreset | "custom";
	inputLevel: number;
	bandLevel: number;
	detectedBpm: number | null;
	confidence: number;
	onSourceTypeSelect: (value: string) => void;
	onSourceChange: (source: SpeedGroupSource) => void;
	onUpdate: <Key extends keyof SoundToLightConfig>(
		key: Key,
		value: SoundToLightConfig[Key],
	) => void;
	onCustomFrequencyChange: (change: {
		low_hz?: number;
		high_hz?: number;
	}) => void;
}) {
	const observation = capture.observation;
	return (
		<>
			<SelectField
				label="Speed Group source"
				value={source.type}
				options={[
					{ value: "manual", label: "Manual" },
					{ value: "speed_group", label: "Speed Group" },
					{ value: "sound_to_light", label: "Sound to Light" },
				]}
				onChange={onSourceTypeSelect}
			/>
			{source.type === "speed_group" && (
				<SelectField
					label="Source Speed Group"
					value={source.group}
					options={(["A", "B", "C", "D", "E"] as const)
						.filter((candidate) => candidate !== group)
						.map((candidate) => ({
							value: candidate,
							label: `Speed Group ${candidate}`,
						}))}
					onChange={(sourceGroup) =>
						onSourceChange({ type: "speed_group", group: sourceGroup })
					}
				/>
			)}
			<section className="sound-live-panel" aria-label="Live sound analysis">
				<div className="sound-meter-stack">
					<Meter label="Input level" value={inputLevel} />
					<div>
						<Meter label="Selected band" value={bandLevel} />
						{source.type === "sound_to_light" && (
							<SelectField
								label="Frequency region"
								value={frequency}
								options={frequencyOptions}
								onChange={(value) =>
									onUpdate(
										"frequency",
										value === "custom"
											? { type: "custom", low_hz: 60, high_hz: 180 }
											: { type: "preset", preset: value },
									)
								}
							/>
						)}
					</div>
				</div>
				<dl>
					<div>
						<dt>Detected tempo</dt>
						<dd>
							{detectedBpm == null
								? "—"
								: `${formatSpeedGroupBpm(detectedBpm)} BPM`}
						</dd>
					</div>
					<div>
						<dt>Confidence</dt>
						<dd>{Math.round(confidence * 100)}%</dd>
					</div>
					<div>
						<dt>Effective speed</dt>
						<dd>{formatSpeedGroupBpm(state.snapshot.effective_bpm)} BPM</dd>
					</div>
					<div>
						<dt>Authoritative source</dt>
						<dd>{sourceLabel(state)}</dd>
					</div>
				</dl>
			</section>
			{source.type === "sound_to_light" && (
				<>
					<div className="sound-status-grid" aria-label="Audio status">
						<article className={`status-${capture.phase}`}>
							<small>Audio source</small>
							<strong>{captureLabel(capture)}</strong>
						</article>
						<article
							className={
								observation?.usable_signal || state.snapshot.usable_signal
									? "status-usable"
									: "status-waiting"
							}
						>
							<small>Selected-band signal</small>
							<strong>
								{observation?.usable_signal || state.snapshot.usable_signal
									? "Usable"
									: "Waiting / quiet"}
							</strong>
						</article>
					</div>
					<p className="sound-capture-message">{capture.message}</p>
					<FormLayout
						columns={2}
						minColumnWidth={250}
						className="sound-configuration-grid"
					>
						<SelectField
							label="Analysis"
							value="tempo_bpm"
							disabled
							options={[{ value: "tempo_bpm", label: "Tempo / BPM" }]}
							onChange={() => undefined}
						/>
						{draft.frequency.type === "custom" && (
							<div className="sound-custom-frequency">
								<NumberField
									label="Low frequency"
									aria-label="Custom low frequency"
									value={draft.frequency.low_hz}
									min={20}
									max={19_999}
									unit="Hz"
									onValueChange={(value) =>
										onCustomFrequencyChange({ low_hz: Number(value) })
									}
								/>
								<NumberField
									label="High frequency"
									aria-label="Custom high frequency"
									value={draft.frequency.high_hz}
									min={21}
									max={20_000}
									unit="Hz"
									onValueChange={(value) =>
										onCustomFrequencyChange({ high_hz: Number(value) })
									}
								/>
							</div>
						)}
						<RangeField
							label="Input gain"
							value={draft.input_gain_db}
							minimum={-60}
							maximum={60}
							step={1}
							unit=" dB"
							onChange={(value) => onUpdate("input_gain_db", value)}
						/>
						<RangeField
							label="Confidence threshold"
							value={draft.confidence_threshold}
							minimum={0}
							maximum={1}
							step={0.01}
							unit=""
							onChange={(value) => onUpdate("confidence_threshold", value)}
						/>
						<RangeField
							label="Tempo smoothing"
							value={draft.smoothing}
							minimum={0}
							maximum={0.99}
							step={0.01}
							unit=""
							onChange={(value) => onUpdate("smoothing", value)}
						/>
						<NumberField
							label="Minimum accepted tempo"
							aria-label="Minimum accepted BPM"
							value={draft.minimum_bpm}
							min={0.1}
							max={998}
							step={1}
							allowDecimal
							unit="BPM"
							onChange={(event) =>
								onUpdate("minimum_bpm", Number(event.target.value))
							}
						/>
						<NumberField
							label="Maximum accepted tempo"
							aria-label="Maximum accepted BPM"
							value={draft.maximum_bpm}
							min={0.2}
							max={999}
							step={1}
							allowDecimal
							unit="BPM"
							onChange={(event) =>
								onUpdate("maximum_bpm", Number(event.target.value))
							}
						/>
						<NumberField
							label="Signal-loss hold"
							aria-label="Signal hold seconds"
							value={draft.signal_hold_millis / 1_000}
							min={0}
							max={60}
							step={0.5}
							allowDecimal
							unit="s"
							onChange={(event) =>
								onUpdate(
									"signal_hold_millis",
									Math.round(Number(event.target.value) * 1_000),
								)
							}
						/>
						<NumberField
							label="Sound speed ratio"
							aria-label="Sound multiplier"
							value={draft.multiplier}
							min={0.125}
							max={8}
							step={0.125}
							allowDecimal
							unit="×"
							onChange={(event) =>
								onUpdate("multiplier", Number(event.target.value))
							}
						/>
					</FormLayout>
				</>
			)}
		</>
	);
}

function DirtyCloseConfirmation({
	visible,
	invalid,
	onDiscard,
	onSave,
	onStay,
}: {
	visible: boolean;
	invalid: boolean;
	onDiscard: () => void;
	onSave: () => void;
	onStay: () => void;
}) {
	if (!visible) return null;
	return (
		<ModalRegistration onClose={onStay}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onStay()
				}
			>
				<section
					className="nested-modal"
					role="alertdialog"
					aria-modal="true"
					aria-label="Unsaved Speed Group settings"
				>
					<ModalTitleBar title="Save Speed Group changes?" onClose={onStay} />
					<p>This Speed Group has unapplied settings.</p>
					<footer className="modal-actions">
						<Button onClick={onDiscard}>Close and discard</Button>
						<Button variant="primary" disabled={invalid} onClick={onSave}>
							Close and save
						</Button>
						<Button onClick={onStay}>Stay</Button>
					</footer>
				</section>
			</div>
		</ModalRegistration>
	);
}

function selectedSource(value: string, group: SpeedGroupId): SpeedGroupSource {
	if (value === "sound_to_light") return { type: "sound_to_light" };
	if (value === "speed_group")
		return { type: "speed_group", group: group === "A" ? "B" : "A" };
	return { type: "manual" };
}

export function SoundToLightModal({
	group,
	state,
	capture,
	controllerError,
	onPreview,
	onSave,
	onAction,
	onClose,
}: {
	group: SpeedGroupId;
	state: SpeedGroupSoundState;
	capture: SoundCaptureStatus;
	permission?: unknown;
	devices?: unknown[];
	deviceId?: string;
	onDeviceChange?: (deviceId: string) => void;
	onRefreshInputs?: () => Promise<void>;
	controllerError?: string | null;
	onPreview: (
		group: SpeedGroupId,
		configuration: SoundToLightConfig | null,
	) => void;
	onSave: (
		configuration: SoundToLightConfig,
		source: SpeedGroupSource,
	) => Promise<SpeedGroupSoundState>;
	onAction: (input: SpeedGroupActionInput) => Promise<SpeedGroupSoundState>;
	onClose: () => void;
}) {
	const [draft, setDraft] = useState<SoundToLightConfig>(() =>
		structuredClone(state.configuration),
	);
	const [source, setSource] = useState<SpeedGroupSource>(
		() =>
			state.source ??
			(state.configuration.enabled
				? { type: "sound_to_light" }
				: { type: "manual" }),
	);
	const [baseline, setBaseline] = useState(() => ({
		configuration: structuredClone(state.configuration),
		source:
			state.source ??
			(state.configuration.enabled
				? ({ type: "sound_to_light" } as const)
				: ({ type: "manual" } as const)),
	}));
	const [confirmClose, setConfirmClose] = useState(false);
	const [busy, setBusy] = useState(false);
	const [localError, setLocalError] = useState<string | null>(null);
	useEffect(() => {
		onPreview(group, draft);
	}, [draft, group, onPreview]);
	useEffect(() => () => onPreview(group, null), [group, onPreview]);
	const invalid = validationError(draft);
	const observation = capture.observation;
	const inputLevel = observation?.level ?? state.snapshot.input_level;
	const bandLevel =
		observation?.selected_band_level ?? state.snapshot.selected_band_level;
	const detectedBpm = observation?.detected_bpm ?? state.snapshot.sound_bpm;
	const confidence =
		observation?.confidence ??
		(state.snapshot.sound_status.state === "active"
			? state.snapshot.sound_status.confidence
			: 0);
	const frequency =
		draft.frequency.type === "preset" ? draft.frequency.preset : "custom";
	const dirty =
		JSON.stringify({ configuration: draft, source }) !==
		JSON.stringify(baseline);
	const update = <Key extends keyof SoundToLightConfig>(
		key: Key,
		value: SoundToLightConfig[Key],
	) => setDraft((current) => ({ ...current, [key]: value }));
	const updateCustomFrequency = (change: {
		low_hz?: number;
		high_hz?: number;
	}) =>
		setDraft((current) => {
			const frequency =
				current.frequency.type === "custom"
					? current.frequency
					: { type: "custom" as const, low_hz: 60, high_hz: 180 };
			return { ...current, frequency: { ...frequency, ...change } };
		});
	const apply = async (closeAfterSave = true) => {
		if (invalid) {
			setLocalError(invalid);
			return;
		}
		setBusy(true);
		try {
			const saved = await onSave(
				{ ...draft, enabled: source.type === "sound_to_light" },
				source,
			);
			const savedSource = saved.source ?? source;
			setDraft(structuredClone(saved.configuration));
			setSource(savedSource);
			setBaseline({
				configuration: structuredClone(saved.configuration),
				source: savedSource,
			});
			setLocalError(null);
			setConfirmClose(false);
			if (closeAfterSave) onClose();
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
			const nextSource = next.source ?? baseline.source;
			setDraft((current) => {
				const merged = structuredClone(current);
				for (const key of Object.keys(current) as Array<
					keyof SoundToLightConfig
				>) {
					if (
						JSON.stringify(current[key]) ===
						JSON.stringify(baseline.configuration[key])
					) {
						(merged as unknown as Record<string, unknown>)[key] =
							structuredClone(next.configuration[key]);
					}
				}
				return merged;
			});
			if (JSON.stringify(source) === JSON.stringify(baseline.source))
				setSource(nextSource);
			setBaseline({
				configuration: structuredClone(next.configuration),
				source: nextSource,
			});
			setLocalError(null);
		} catch (reason) {
			setLocalError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setBusy(false);
		}
	};
	const requestClose = () => (dirty ? setConfirmClose(true) : onClose());
	const selectSource = (value: string) => {
		const next = selectedSource(value, group);
		setSource(next);
		setDraft((current) => ({
			...current,
			enabled: next.type === "sound_to_light",
		}));
	};
	return createPortal(
		<ModalRegistration onClose={requestClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && requestClose()
				}
			>
				<section
					className="nested-modal sound-to-light-modal"
					role="dialog"
					aria-modal="true"
					aria-label={`Speed Group ${group} Sound to Light`}
				>
					<ModalTitleBar
						title={`Speed Group ${group}`}
						groups={[
							{
								id: "speed",
								actions: [
									{
										id: "half",
										label: "÷2",
										ariaLabel: "Half Speed Group speed",
										className: "sound-speed-scale-action",
										disabled: busy,
										onPress: () => void action({ action: "half" }),
									},
									{
										id: "double",
										label: "×2",
										ariaLabel: "Double Speed Group speed",
										className: "sound-speed-scale-action",
										disabled: busy,
										onPress: () => void action({ action: "double" }),
									},
									{
										id: "pause",
										label: state.snapshot.paused ? "Resume" : "Pause",
										ariaLabel: state.snapshot.paused
											? "Resume Speed Group"
											: "Pause Speed Group",
										active: state.snapshot.paused,
										disabled: busy,
										onPress: () => void action({ action: "pause" }),
									},
								],
							},
						]}
						accept={{
							id: "apply",
							label: "Apply",
							variant: "primary",
							loading: busy,
							disabled: Boolean(invalid) || !dirty,
							onPress: () => void apply(),
						}}
						closeLabel="Close Speed Group settings"
						onClose={requestClose}
					/>
					<SoundToLightFields
						group={group}
						state={state}
						capture={capture}
						draft={draft}
						source={source}
						frequency={frequency}
						inputLevel={inputLevel}
						bandLevel={bandLevel}
						detectedBpm={detectedBpm}
						confidence={confidence}
						onSourceTypeSelect={selectSource}
						onSourceChange={setSource}
						onUpdate={update}
						onCustomFrequencyChange={updateCustomFrequency}
					/>
					{(invalid || localError || controllerError) && (
						<p className="sound-error" role="alert">
							{localError ?? invalid ?? controllerError}
						</p>
					)}
					<DirtyCloseConfirmation
						visible={confirmClose}
						invalid={Boolean(invalid)}
						onDiscard={onClose}
						onSave={() => void apply(true)}
						onStay={() => setConfirmClose(false)}
					/>
				</section>
			</div>
		</ModalRegistration>,
		document.body,
	);
}
