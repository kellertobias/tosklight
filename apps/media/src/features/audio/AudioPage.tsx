// The audio monitor, and the tuning beside it.
//
// The two belong on one page because they are used together: an operator turns the gain while
// watching the meter. The meter arrives over the telemetry socket; the tuning is an edit like any
// other, and the gains reach the running analysis as soon as it is stored.

import {
	Button,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui/controls";
import { useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	AudioSettingsView,
	UpdateAudio,
} from "../../shared/api/generated/media-wire";
import { useAudio } from "../../shared/api/queries";
import { useTelemetry } from "../../shared/api/telemetry";
import { AudioMeters } from "./AudioMeters";

export function AudioPage() {
	const audio = useAudio();
	const telemetry = useTelemetry();
	const editing = useEditing(audio.reload);

	return (
		<section className="media-page">
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}

			<ResourceState resource={audio} subject="the audio monitor">
				{(data) => (
					<>
						{/* The socket's frames when it is up, and the snapshot until it is. */}
						<AudioMeters
							audio={telemetry.frame?.audio ?? data.analysis}
							live={telemetry.connected}
						/>
						{editing.editing === "audio" ? (
							<AudioSettings
								settings={data.settings}
								busy={editing.busy}
								onSave={(edit) =>
									void editing.save(() => api.updateAudio(edit))
								}
								onCancel={editing.cancel}
							/>
						) : (
							<article
								className="media-settings-section"
								aria-label="Audio settings"
							>
								<StoredSettings settings={data.settings} />
								<div className="media-settings-actions">
									<Button onClick={() => editing.begin("audio")}>
										Change audio settings
									</Button>
								</div>
							</article>
						)}
					</>
				)}
			</ResourceState>
		</section>
	);
}

function StoredSettings({ settings }: { settings: AudioSettingsView }) {
	return (
		<dl className="media-facts">
			<dt>Input</dt>
			<dd>{describeDevice(settings)}</dd>
			<dt>Gain</dt>
			<dd>{settings.inputGain.toFixed(2)}×</dd>
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
		description: "Applied through a curve, so low settings stay precise.",
	},
	{
		field: "beatSensitivity",
		label: "Beat sensitivity",
		description: "Higher triggers more easily.",
	},
	{ field: "eqBass", label: "Bass", description: undefined },
	{ field: "eqMid", label: "Mid", description: undefined },
	{ field: "eqTreble", label: "Treble", description: undefined },
] as const;

type GainField = (typeof GAINS)[number]["field"];

function AudioSettings({
	settings,
	busy,
	onSave,
	onCancel,
}: {
	settings: AudioSettingsView;
	busy: boolean;
	onSave: (edit: UpdateAudio) => void;
	onCancel: () => void;
}) {
	const [deviceBy, setDeviceBy] = useState(settings.deviceBy);
	const [deviceValue, setDeviceValue] = useState(settings.deviceValue ?? "");
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

	return (
		<form
			className="media-settings-form"
			onSubmit={(event) => {
				event.preventDefault();
				onSave({
					requestId: requestId(),
					deviceBy,
					deviceValue: deviceBy === "system-default" ? undefined : deviceValue,
					...gains,
				});
			}}
		>
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

			{GAINS.map((gain) => (
				<NumberField
					key={gain.field}
					label={gain.label}
					description={gain.description}
					min={0}
					max={10}
					step={0.05}
					value={String(gains[gain.field])}
					onChange={(event) =>
						setGains((current) => ({
							...current,
							[gain.field]: Number(event.target.value),
						}))
					}
				/>
			))}

			{settings.deviceTakesEffectOnRestart && (
				<p className="media-state is-notice">
					The gains take effect as you save them. Choosing a different input
					opens a different stream, which happens the next time this server
					starts.
				</p>
			)}

			<div className="media-settings-actions">
				<Button type="submit" variant="primary" loading={busy}>
					Save
				</Button>
				<Button onClick={onCancel}>Cancel</Button>
			</div>
		</form>
	);
}
