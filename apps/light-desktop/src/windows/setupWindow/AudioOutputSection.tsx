import { NumberField, SelectField, TextAreaField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { InternalAudioStatus } from "../../api/types";
import { useTimecodeActions } from "../../features/timecode/TimecodeActionsContext";
import type { SetupWindowController } from "./controller";

export function AudioOutputSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const draft = controller.draft;
	const timecodes = useTimecodeActions();
	const [audioOutputs, setAudioOutputs] = useState<readonly string[]>([]);
	const [audioOutputError, setAudioOutputError] = useState<string | null>(null);
	const [internalAudioStatus, setInternalAudioStatus] =
		useState<InternalAudioStatus | null>(null);
	useEffect(() => {
		let active = true;
		if (!timecodes) return;
		void timecodes.api
			.outputDevices()
			.then((result) => {
				if (active) setAudioOutputs(result.devices);
			})
			.catch((reason) => {
				if (active)
					setAudioOutputError(
						reason instanceof Error ? reason.message : String(reason),
					);
			});
		void timecodes.api
			.internalAudioStatus()
			.then((status) => {
				if (active) setInternalAudioStatus(status);
			})
			.catch(() => {
				if (active) setInternalAudioStatus(null);
			});
		return () => {
			active = false;
		};
	}, [timecodes]);
	if (!draft) return null;
	const selectedOutput =
		draft.timecode_audio_output_device ?? "$system_default";
	const outputTrim =
		draft.timecode_audio_latency_trim_micros_by_output?.[selectedOutput] ?? 0;
	return (
		<div className="setup-form-grid">
			<SelectField
				label="Timecode audio output"
				value={selectedOutput}
				options={[
					{ value: "$system_default", label: "System default" },
					...audioOutputs.map((device) => ({ value: device, label: device })),
				]}
				description={
					audioOutputError
						? `Output discovery unavailable: ${audioOutputError}`
						: "The server opens this exact device after restart; System default follows the operating system."
				}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						timecode_audio_output_device:
							value === "$system_default" ? null : value,
					})
				}
			/>
			<NumberField
				label="Audio latency trim"
				value={outputTrim}
				min={-5_000_000}
				max={5_000_000}
				unit="µs"
				description="Stored separately for the selected output and added to the backend-reported latency."
				onChange={(event) =>
					controller.editDraft({
						...draft,
						timecode_audio_latency_trim_micros_by_output: {
							...draft.timecode_audio_latency_trim_micros_by_output,
							[selectedOutput]: Number(event.target.value),
						},
					})
				}
			/>
			{internalAudioStatus && (
				<div className="setup-field-description" aria-live="polite">
					{internalAudioStatus.players.map((player) => (
						<p key={player.fixture_id} role={player.available ? undefined : "alert"}>
							Audio Player {player.fixture_id}: {player.available ? "Ready" : player.diagnostic}
						</p>
					))}
					{internalAudioStatus.libraries.flatMap((library) =>
						library.diagnostics.map((diagnostic) => (
							<p key={`${library.binding}:${diagnostic}`} role="alert">
								{library.binding}: {diagnostic}
							</p>
						)),
					)}
				</div>
			)}
			<AudioBindingMapField
				label="Audio output bindings"
				description="One portable name = exact local device per line. Use $system_default for the operating-system default."
				value={draft.internal_audio_output_devices ?? {}}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						internal_audio_output_devices: value,
					})
				}
			/>
		</div>
	);
}

function AudioBindingMapField({
	label,
	description,
	value,
	onChange,
}: {
	label: string;
	description: string;
	value: Record<string, string>;
	onChange: (value: Record<string, string>) => void;
}) {
	const formatted = Object.entries(value)
		.sort(([left], [right]) => left.localeCompare(right))
		.map(([name, target]) => `${name} = ${target}`)
		.join("\n");
	const [draft, setDraft] = useState(formatted);
	useEffect(() => setDraft(formatted), [formatted]);
	return (
		<TextAreaField
			label={label}
			description={description}
			value={draft}
			onChange={(event) => setDraft(event.target.value)}
			onBlur={() => {
				const parsed = parseAudioBindingMap(draft);
				if (parsed) onChange(parsed);
			}}
		/>
	);
}

export function parseAudioBindingMap(value: string) {
	const result: Record<string, string> = {};
	for (const line of value.split("\n")) {
		if (!line.trim()) continue;
		const separator = line.indexOf("=");
		if (separator < 1) return null;
		const name = line.slice(0, separator).trim();
		const target = line.slice(separator + 1).trim();
		if (!name || !target || name.length > 128 || name in result) return null;
		result[name] = target;
	}
	return result;
}
