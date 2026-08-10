import { Button, NumberField, SelectField, TextField } from "@tosklight/ui";
import { useEffect, useState } from "react";
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
		return () => {
			active = false;
		};
	}, [timecodes]);
	if (!draft) return null;
	const external = draft.timecode_source.type === "external";
	const externalSource =
		draft.timecode_source.type === "external"
			? draft.timecode_source.source
			: "";
	const frameRate = draft.timecode_frame_rate
		? draft.timecode_frame_rate.numerator /
			draft.timecode_frame_rate.denominator
		: 0;
	const selectedOutput =
		draft.timecode_audio_output_device ?? "$system_default";
	const outputTrim =
		draft.timecode_audio_latency_trim_micros_by_output?.[selectedOutput] ?? 0;
	return (
		<>
			<h2>Timecode</h2>
			<div className="setup-form-grid">
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
				{external && (
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
			</div>
		</>
	);
}
