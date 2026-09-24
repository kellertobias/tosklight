// The audio monitor, and the tuning beside it.
//
// The two belong on one page because they are used together: an operator turns the gain while
// watching the meter. The meter arrives over the telemetry socket; the tuning is an edit like any
// other, and the gains reach the running analysis as soon as it is stored.

import { HorizontalFaderField } from "@tosklight/ui";
import { SelectField, SwitchField, TextField } from "@tosklight/ui/controls";
import { WindowFrame } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useRef, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	AudioSettingsView,
	MicrophonePermissionView,
	UpdateAudio,
} from "../../shared/api/generated/media-wire";
import { useAudio } from "../../shared/api/queries";
import { useTelemetry } from "../../shared/api/telemetry";
import { AudioMeters } from "./AudioMeters";

export function AudioPage() {
	const audio = useAudio();
	const telemetry = useTelemetry();
	const editing = useEditing(audio.reload);

	const editingAudio = editing.editing === "audio";
	const [requestingPermission, setRequestingPermission] = useState(false);
	const [permissionError, setPermissionError] = useState<string | null>(null);
	const requestPermission = useCallback(async () => {
		setRequestingPermission(true);
		setPermissionError(null);
		try {
			await api.requestMicrophonePermission();
		} catch (error) {
			setPermissionError(
				error instanceof Error
					? error.message
					: "Could not request microphone access.",
			);
		} finally {
			await audio.reload();
			setRequestingPermission(false);
		}
	}, [audio.reload]);
	const saveAudioLive = useCallback(
		(edit: UpdateAudio) => editing.saveLive(() => api.updateAudio(edit)),
		[editing.saveLive],
	);

	return (
		<WindowFrame
			title="Audio"
			info={{
				primary: "Audio monitor",
				secondary: "Live input analysis and sound-to-light tuning",
			}}
			className="media-audio-window"
			groups={[
				{
					id: "audio-settings-actions",
					actions: editingAudio
						? [
								{
									id: "cancel-audio-settings",
									label: "Done",
									onPress: editing.cancel,
								},
							]
						: [
								{
									id: "change-audio-settings",
									label: "Change audio settings",
									onPress: () => editing.begin("audio"),
								},
							],
				},
			]}
			infoSection={
				editingAudio && audio.data ? (
					<AudioSettings
						settings={audio.data.settings}
						onChange={saveAudioLive}
					/>
				) : undefined
			}
		>
			<section className="media-page media-audio-content">
				{editing.failure && (
					<MediaErrorToast
						message={editing.failure.message}
						onDismiss={editing.dismiss}
					/>
				)}
				{permissionError && (
					<MediaErrorToast
						message={permissionError}
						onDismiss={() => setPermissionError(null)}
					/>
				)}

				<ResourceState resource={audio} subject="the audio monitor">
					{(data) => (
						<>
							<MicrophoneAccess
								permission={data.microphonePermission}
								capturing={(telemetry.frame?.audio ?? data.analysis).capturing}
								requesting={requestingPermission}
								onRequest={requestPermission}
							/>
							{/* The socket's frames when it is up, and the snapshot until it is. */}
							<AudioMeters
								audio={telemetry.frame?.audio ?? data.analysis}
								live={telemetry.connected}
							/>
							<article
								className="media-settings-section"
								aria-label="Audio settings"
							>
								<StoredSettings settings={data.settings} />
							</article>
						</>
					)}
				</ResourceState>
			</section>
		</WindowFrame>
	);
}

function MicrophoneAccess({
	permission,
	capturing,
	requesting,
	onRequest,
}: {
	permission: MicrophonePermissionView;
	capturing: boolean;
	requesting: boolean;
	onRequest: () => void;
}) {
	if (permission === "not-determined")
		return (
			<div className="media-state is-notice" role="status">
				<p>
					Pixel needs microphone access to analyse the selected audio input.
				</p>
				<button type="button" disabled={requesting} onClick={onRequest}>
					{requesting ? "Waiting for macOS…" : "Request microphone access"}
				</button>
			</div>
		);
	if (permission === "denied" || permission === "restricted")
		return (
			<div className="media-state is-warning" role="alert">
				<p>
					{permission === "denied"
						? "Microphone access is denied. Enable ToskLight Pixel in macOS System Settings → Privacy & Security → Microphone, then check access here."
						: "macOS restricts microphone access for Pixel. Check the device's privacy policy."}
				</p>
				{permission === "denied" && (
					<button type="button" disabled={requesting} onClick={onRequest}>
						Check microphone access
					</button>
				)}
			</div>
		);
	if (permission === "granted" && !capturing)
		return (
			<div className="media-state is-notice" role="status">
				<p>Microphone access is granted, but the selected input is not open.</p>
				<button type="button" disabled={requesting} onClick={onRequest}>
					{requesting ? "Opening input…" : "Start audio input"}
				</button>
			</div>
		);
	return null;
}

function StoredSettings({ settings }: { settings: AudioSettingsView }) {
	return (
		<dl className="media-facts">
			<dt>Input</dt>
			<dd>{describeDevice(settings)}</dd>
			<dt>Gain</dt>
			<dd>
				{settings.autoGain
					? `automatic, trimmed ${settings.inputGain.toFixed(2)}×`
					: `${settings.inputGain.toFixed(2)}×`}
			</dd>
			<dt>Beat sensitivity</dt>
			<dd>{settings.beatSensitivity.toFixed(2)}×</dd>
			<dt>Bass · mid · treble</dt>
			<dd>
				{settings.eqBass.toFixed(2)}× · {settings.eqMid.toFixed(2)}× ·{" "}
				{settings.eqTreble.toFixed(2)}×
			</dd>
		</dl>
	);
}

function describeDevice(settings: AudioSettingsView): string {
	if (settings.deviceBy === "system-default")
		return "this machine's default input";
	if (settings.deviceBy === "index")
		return `input number ${settings.deviceValue}`;
	return settings.deviceValue ?? "unnamed";
}

/// The gains an operator turns, and which input they turn them on.
const GAINS = [
	{
		field: "inputGain",
		label: "Gain",
		description:
			"Applied through a curve, so low settings stay precise. With automatic gain on, a trim on top of it.",
	},
	{
		field: "beatSensitivity",
		label: "Beat sensitivity",
		description:
			"Higher finds quieter kicks, snares, and hi-hats. Detection does not depend on the gain.",
	},
	{ field: "eqBass", label: "Bass", description: undefined },
	{ field: "eqMid", label: "Mid", description: undefined },
	{ field: "eqTreble", label: "Treble", description: undefined },
] as const;

type GainField = (typeof GAINS)[number]["field"];

function AudioSettings({
	settings,
	onChange,
}: {
	settings: AudioSettingsView;
	onChange: (edit: UpdateAudio) => void;
}) {
	const mounted = useRef(false);
	const [deviceBy, setDeviceBy] = useState(settings.deviceBy);
	const [deviceValue, setDeviceValue] = useState(settings.deviceValue ?? "");
	const [autoGain, setAutoGain] = useState(settings.autoGain);
	const [gains, setGains] = useState<Record<GainField, number>>({
		inputGain: settings.inputGain,
		beatSensitivity: settings.beatSensitivity,
		eqBass: settings.eqBass,
		eqMid: settings.eqMid,
		eqTreble: settings.eqTreble,
	});

	// An operator picks from the inputs this machine has; typing a name is the fallback for a
	// device that is not plugged in yet, which is a real case on a rig built in advance.
	const options = [
		{ value: "system-default", label: "This machine's default input" },
		...settings.availableDevices.map((name) => ({
			value: `name:${name}`,
			label: name,
		})),
		{ value: "name", label: "A device by name…" },
	];
	const selection =
		deviceBy === "name" && settings.availableDevices.includes(deviceValue)
			? `name:${deviceValue}`
			: deviceBy;
	useEffect(() => {
		if (!mounted.current) {
			mounted.current = true;
			return;
		}
		onChange({
			requestId: requestId(),
			deviceBy,
			deviceValue: deviceBy === "system-default" ? undefined : deviceValue,
			autoGain,
			...gains,
		});
	}, [deviceBy, deviceValue, autoGain, gains, onChange]);

	return (
		<form className="media-settings-form">
			<SelectField
				label="Input"
				value={selection}
				options={options}
				onChange={(next) => {
					if (next.startsWith("name:")) {
						setDeviceBy("name");
						setDeviceValue(next.slice("name:".length));
						return;
					}
					setDeviceBy(next);
					if (next === "system-default") setDeviceValue("");
				}}
			/>
			{deviceBy === "name" && (
				<TextField
					label="Device name"
					description="Exactly as the machine names it. A device this machine does not have is an error, never a quiet fall back to the default."
					value={deviceValue}
					onChange={(event) => setDeviceValue(event.target.value)}
				/>
			)}
			{deviceBy === "index" && (
				<TextField
					label="Device number"
					description="Carried over from the legacy application's configuration."
					value={deviceValue}
					onChange={(event) => setDeviceValue(event.target.value)}
				/>
			)}

			<SwitchField
				label="Automatic gain"
				description="Levels the meters and visualizers to whatever is playing, from a quiet room microphone to a hot desk feed."
				offLabel="Manual"
				onLabel="Auto"
				checked={autoGain}
				onChange={(event) => setAutoGain(event.target.checked)}
			/>

			{GAINS.map((gain) => (
				<HorizontalFaderField
					key={gain.field}
					label={gain.label}
					description={gain.description}
					minimum={0}
					maximum={10}
					step={0.05}
					value={gains[gain.field]}
					display={`${gains[gain.field].toFixed(2)}×`}
					onChange={(value) =>
						setGains((current) => ({
							...current,
							[gain.field]: value,
						}))
					}
				/>
			))}

			{settings.deviceTakesEffectOnRestart && (
				<p className="media-state is-notice">
					The gains take effect as you adjust them. Choosing a different input
					opens a different stream, which happens the next time this server
					starts.
				</p>
			)}
		</form>
	);
}
