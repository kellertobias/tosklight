import {
	Button,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { InternalAudioStatus } from "../../api/generated/light-wire";
import { ShowRecoveryFileManager } from "../../components/setup/ShowRecoveryFileManager";
import { useBootstrapSnapshot } from "../../features/deskSnapshot/DeskSnapshotState";
import { useConnectionStatus } from "../../features/shellStatus/ShellStatusState";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
import { useTimecodeActions } from "../../features/timecode/TimecodeActionsContext";
import { useApp } from "../../state/AppContext";
import type { SetupWindowController } from "./controller";

export function ShowsRecoverySection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const bootstrap = useBootstrapSnapshot();
	const lifecycle = useShowLifecycle();
	const connectionStatus = useConnectionStatus();
	const { dispatch } = useApp();
	const { draft } = controller;
	const activeShow = bootstrap?.active_show;
	const autosaveActive =
		connectionStatus === "connected" && Boolean(activeShow);
	const autosaveStatus = autosaveActive
		? "Connected, autosave active"
		: connectionStatus === "connected"
			? "Connected, no active show"
			: `${connectionStatus}, autosave paused`;
	return (
		<>
			<h2>Shows & recovery</h2>
			<div className="setup-show-summary">
				<section>
					<b>Current show</b>
					<span>{activeShow?.name ?? "No show loaded"}</span>
					<small>
						{activeShow?.updated_at ?? "Choose a show from the library"}
					</small>
				</section>
				<section>
					<b>Show library</b>
					<span>{lifecycle?.shows.length ?? 0} library shows</span>
					<Button
						onClick={() =>
							dispatch({ type: "OPEN_BUILTIN", kind: "file_manager" })
						}
					>
						Open show
					</Button>
				</section>
				<section>
					{draft && (
						<NumberField
							label="Autosave interval"
							min="5"
							max="3600"
							value={draft.autosave_interval_seconds}
							onChange={(event) =>
								controller.editDraft({
									...draft,
									autosave_interval_seconds: Number(event.target.value),
								})
							}
						/>
					)}
					<small className={autosaveActive ? "is-connected" : ""}>
						{autosaveStatus}
					</small>
				</section>
			</div>
			<ShowRecoveryFileManager
				onOpenFixtureLibrary={() => controller.setFixtureLibraryOpen(true)}
			/>
		</>
	);
}

export function TimecodeSection({
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
	return (
		<>
			<h2>Timecode</h2>
			<div className="setup-form-grid">
				<TimecodeSourceFields controller={controller} />
				<TimecodeAudioFields
					controller={controller}
					audioOutputs={audioOutputs}
					audioOutputError={audioOutputError}
					internalAudioStatus={internalAudioStatus}
				/>
			</div>
		</>
	);
}

function TimecodeSourceFields({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const draft = controller.draft;
	if (!draft) return null;
	const externalSource =
		draft.timecode_source.type === "external"
			? draft.timecode_source.source
			: "";
	const frameRate = draft.timecode_frame_rate
		? draft.timecode_frame_rate.numerator /
			draft.timecode_frame_rate.denominator
		: 0;
	return (
		<>
			<TextField
				label="ArtTimeCode UDP bind"
				value={draft.art_timecode_bind ?? ""}
				description="Listen for Art-Net ArtTimeCode on this local address, for example 0.0.0.0:6454. Leave empty to disable the network source."
				onChange={(event) =>
					controller.editDraft({
						...draft,
						art_timecode_bind: event.target.value || null,
					})
				}
			/>
			<SelectField
				label="Authoritative source"
				value={draft.timecode_source.type}
				options={[
					{ value: "internal", label: "Internal generator" },
					{ value: "external", label: "Explicit external source" },
				]}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						timecode_source:
							value === "external"
								? { type: "external", source: "" }
								: { type: "internal" },
					})
				}
			/>
			{draft.timecode_source.type === "external" && (
				<TextField
					label="External source identity"
					value={externalSource}
					description="Use the exact normalized identity shown by the input adapter. No other sender can take over."
					onChange={(event) =>
						controller.editDraft({
							...draft,
							timecode_source: {
								type: "external",
								source: event.target.value,
							},
						})
					}
				/>
			)}
			<NumberField
				label="Timecode frame rate"
				value={frameRate}
				min={0}
				max={240}
				description="0 follows the desk DMX frame rate. A non-zero value converts known incoming rates with a warning."
				onChange={(event) => {
					const value = Number(event.target.value);
					controller.editDraft({
						...draft,
						timecode_frame_rate:
							value > 0
								? { numerator: value, denominator: 1, drop_frame: false }
								: null,
					});
				}}
			/>
		</>
	);
}

function TimecodeAudioFields({
	controller,
	audioOutputs,
	audioOutputError,
	internalAudioStatus,
}: {
	controller: SetupWindowController;
	audioOutputs: readonly string[];
	audioOutputError: string | null;
	internalAudioStatus: InternalAudioStatus | null;
}) {
	const draft = controller.draft;
	if (!draft) return null;
	const selectedOutput =
		draft.timecode_audio_output_device ?? "$system_default";
	const outputTrim =
		draft.timecode_audio_latency_trim_micros_by_output?.[selectedOutput] ?? 0;
	return (
		<>
			<SelectField
				label="External source loss"
				value={draft.timecode_external_loss_policy}
				options={[
					{ value: "continue_internal", label: "Continue internally" },
					{ value: "pause", label: "Pause" },
					{ value: "stop", label: "Stop" },
				]}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						timecode_external_loss_policy: value as
							| "continue_internal"
							| "pause"
							| "stop",
					})
				}
			/>
			<NumberField
				label="Loss timeout"
				value={draft.timecode_external_loss_timeout_millis}
				min={1}
				max={60_000}
				unit="ms"
				onChange={(event) =>
					controller.editDraft({
						...draft,
						timecode_external_loss_timeout_millis: Number(event.target.value),
					})
				}
			/>
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
			<AudioBindingMapField
				label="Audio library bindings"
				description="One portable name = absolute local root per line. Example: show-audio = /Volumes/Show/Audio"
				value={draft.internal_audio_library_roots ?? {}}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						internal_audio_library_roots: value,
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
		</>
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
